//! Owns live database sessions: connect, keep-alive, reconnect, safety gates.

use std::{
    collections::HashMap,
    sync::Arc,
    time::Duration,
};

use crossbeam_channel::{Receiver, Sender, TryRecvError};
use tokio_util::sync::CancellationToken;
use wisp_drivers::{DbDriver, ExecuteStats};
use wisp_store::ConnectionId;

use super::{
    config::SessionRuntimeConfig,
    driver::{apply_read_only_session, open_main_driver, open_metadata_driver, SessionSecrets},
    error::SessionError,
    policy::check_write_allowed,
    state::{SessionEvent, SessionPhase, SessionSnapshot},
};
struct ManagedSession {
    profile: wisp_store::ConnectionProfile,
    secrets: SessionSecrets,
    phase: SessionPhase,
    main: Option<Box<dyn DbDriver>>,
    metadata: Option<Box<dyn DbDriver>>,
    version: Option<String>,
    detail: Option<String>,
    keepalive_cancel: CancellationToken,
}

pub struct SessionManager {
    config: SessionRuntimeConfig,
    sessions: HashMap<ConnectionId, ManagedSession>,
    events_tx: Sender<SessionEvent>,
    events_rx: Receiver<SessionEvent>,
    shutdown: CancellationToken,
}

impl SessionManager {
    pub fn new(config: SessionRuntimeConfig) -> Arc<tokio::sync::Mutex<Self>> {
        let (events_tx, events_rx) = crossbeam_channel::unbounded();
        Arc::new(tokio::sync::Mutex::new(Self {
            config,
            sessions: HashMap::new(),
            events_tx,
            events_rx,
            shutdown: CancellationToken::new(),
        }))
    }

    pub fn try_recv_event(manager: &Arc<tokio::sync::Mutex<Self>>) -> Option<SessionEvent> {
        let guard = manager.blocking_lock();
        match guard.events_rx.try_recv() {
            Ok(ev) => Some(ev),
            Err(TryRecvError::Empty | TryRecvError::Disconnected) => None,
        }
    }

    pub fn shutdown_token(manager: &Arc<tokio::sync::Mutex<Self>>) -> CancellationToken {
        manager.blocking_lock().shutdown.clone()
    }

    pub async fn open(
        manager: &Arc<tokio::sync::Mutex<Self>>,
        spec: SessionOpenSpec,
        runtime: tokio::runtime::Handle,
    ) -> Result<SessionSnapshot, SessionError> {
        let id = spec.profile.id;
        {
            let mut guard = manager.lock().await;
            if let Some(existing) = guard.sessions.get(&id) {
                if matches!(
                    existing.phase,
                    SessionPhase::Ready | SessionPhase::Busy | SessionPhase::Connecting
                ) {
                    return Ok(guard.snapshot_locked(id));
                }
                guard.close_locked(id);
            }
            guard.sessions.insert(
                id,
                ManagedSession {
                    profile: spec.profile.clone(),
                    secrets: spec.secrets.clone(),
                    phase: SessionPhase::Connecting,
                    main: None,
                    metadata: None,
                    version: None,
                    detail: None,
                    keepalive_cancel: CancellationToken::new(),
                },
            );
            guard.emit(id, SessionPhase::Connecting, None);
        }

        let runtime_for_connect = runtime.clone();
        let connect_result = {
            let profile = spec.profile.clone();
            let secrets = spec.secrets.clone();
            tokio::task::spawn_blocking(move || {
                let mut driver =
                    open_main_driver(&profile, &secrets, runtime_for_connect.clone())?;
                apply_read_only_session(&mut *driver, &profile)?;
                let info = driver
                    .server_info()
                    .map_err(|e| SessionError::Connect(e.to_string()))?;
                Ok::<_, SessionError>((driver, info.version))
            })
            .await
            .map_err(|e| SessionError::Connect(e.to_string()))?
        };

        let mut guard = manager.lock().await;
        match connect_result {
            Ok((driver, version)) => {
                let session = guard
                    .sessions
                    .get_mut(&id)
                    .ok_or(SessionError::NotFound(id))?;
                session.main = Some(driver);
                session.version = Some(version);
                session.phase = SessionPhase::Ready;
                session.detail = None;
            }
            Err(err) => {
                if let Some(session) = guard.sessions.get_mut(&id) {
                    session.phase = SessionPhase::Failed;
                    session.detail = Some(err.to_string());
                }
                let detail = err.to_string();
                guard.emit(id, SessionPhase::Failed, Some(detail));
                return Err(err);
            }
        }
        let snapshot = guard.snapshot_locked(id);
        let keep_alive = guard.config.keep_alive;
        let cancel = guard
            .sessions
            .get(&id)
            .expect("session inserted")
            .keepalive_cancel
            .clone();
        guard.emit(id, SessionPhase::Ready, None);
        drop(guard);
        spawn_keepalive(Arc::clone(manager), id, keep_alive, cancel, runtime);
        Ok(snapshot)
    }

    pub async fn close(manager: &Arc<tokio::sync::Mutex<Self>>, id: ConnectionId) {
        let mut guard = manager.lock().await;
        guard.close_locked(id);
    }

    pub async fn shutdown_all(manager: &Arc<tokio::sync::Mutex<Self>>) {
        let mut guard = manager.lock().await;
        guard.shutdown.cancel();
        let ids: Vec<_> = guard.sessions.keys().copied().collect();
        for id in ids {
            guard.close_locked(id);
        }
    }

    pub async fn execute(
        manager: &Arc<tokio::sync::Mutex<Self>>,
        id: ConnectionId,
        sql: &str,
        write_approved: bool,
    ) -> Result<ExecuteStats, SessionError> {
        let (read_only, safe_mode) = {
            let guard = manager.lock().await;
            let session = guard.sessions.get(&id).ok_or(SessionError::NotFound(id))?;
            if session.phase != SessionPhase::Ready {
                return Err(SessionError::NotReady {
                    phase: session.phase,
                });
            }
            (session.profile.read_only, session.profile.safe_mode)
        };
        check_write_allowed(read_only, safe_mode, write_approved, sql)?;

        let sql = sql.to_string();
        let result = {
            let mut guard = manager.lock().await;
            let session = guard.sessions.get_mut(&id).ok_or(SessionError::NotFound(id))?;
            session.phase = SessionPhase::Busy;
            let driver = session.main.as_mut().ok_or(SessionError::NotReady {
                phase: SessionPhase::Reconnecting,
            })?;
            driver.execute(&sql).map_err(|e| SessionError::Query(e.to_string()))
        };

        let mut guard = manager.lock().await;
        if let Some(session) = guard.sessions.get_mut(&id) {
            session.phase = if result.is_ok() {
                SessionPhase::Ready
            } else {
                SessionPhase::Ready
            };
        }
        result
    }

    pub async fn snapshot(
        manager: &Arc<tokio::sync::Mutex<Self>>,
        id: ConnectionId,
    ) -> Option<SessionSnapshot> {
        let guard = manager.lock().await;
        guard.sessions.get(&id).map(|_| guard.snapshot_locked(id))
    }

    fn close_locked(&mut self, id: ConnectionId) {
        if let Some(mut session) = self.sessions.remove(&id) {
            session.keepalive_cancel.cancel();
            if let Some(mut driver) = session.main.take() {
                let _ = driver.close();
            }
            if let Some(mut driver) = session.metadata.take() {
                let _ = driver.close();
            }
            self.emit(id, SessionPhase::Closed, None);
        }
    }

    fn snapshot_locked(&self, id: ConnectionId) -> SessionSnapshot {
        let session = self
            .sessions
            .get(&id)
            .expect("snapshot without session");
        SessionSnapshot {
            connection_id: id,
            name: session.profile.name.clone(),
            phase: session.phase,
            read_only: session.profile.read_only,
            safe_mode: session.profile.safe_mode,
            version: session.version.clone(),
            detail: session.detail.clone(),
        }
    }

    fn set_phase(
        &mut self,
        id: ConnectionId,
        phase: SessionPhase,
        detail: Option<String>,
        profile: &wisp_store::ConnectionProfile,
    ) {
        if let Some(session) = self.sessions.get_mut(&id) {
            session.phase = phase;
            session.detail = detail.clone();
        } else {
            self.sessions.insert(
                id,
                ManagedSession {
                    profile: profile.clone(),
                    secrets: SessionSecrets {
                        password: None,
                        ssh_password: None,
                        ssh_key_passphrase: None,
                    },
                    phase,
                    main: None,
                    metadata: None,
                    version: None,
                    detail: detail.clone(),
                    keepalive_cancel: CancellationToken::new(),
                },
            );
        }
        self.emit(id, phase, detail);
    }

    fn emit(&self, id: ConnectionId, phase: SessionPhase, detail: Option<String>) {
        let _ = self.events_tx.send(SessionEvent {
            connection_id: id,
            phase,
            detail,
        });
    }

    async fn reconnect(
        &mut self,
        id: ConnectionId,
        runtime: tokio::runtime::Handle,
    ) -> Result<(), SessionError> {
        let (profile, secrets) = {
            let session = self.sessions.get(&id).ok_or(SessionError::NotFound(id))?;
            (session.profile.clone(), session.secrets.clone())
        };
        if let Some(session) = self.sessions.get_mut(&id) {
            if let Some(mut driver) = session.main.take() {
                let _ = driver.close();
            }
            session.phase = SessionPhase::Reconnecting;
            session.detail = Some("connection lost".into());
        }
        self.emit(id, SessionPhase::Reconnecting, Some("connection lost".into()));

        let connect_result = tokio::task::spawn_blocking(move || {
            let mut driver = open_main_driver(&profile, &secrets, runtime.clone())?;
            apply_read_only_session(&mut *driver, &profile)?;
            let info = driver.server_info().map_err(|e| SessionError::Connect(e.to_string()))?;
            Ok::<_, SessionError>((driver, info.version))
        })
        .await
        .map_err(|e| SessionError::Connect(e.to_string()))?;

        let (ready, fail_detail) = {
            let session = self.sessions.get_mut(&id).ok_or(SessionError::NotFound(id))?;
            match connect_result {
                Ok((driver, version)) => {
                    session.main = Some(driver);
                    session.version = Some(version);
                    session.phase = SessionPhase::Ready;
                    session.detail = None;
                    (true, None)
                }
                Err(err) => {
                    session.phase = SessionPhase::Failed;
                    let msg = err.to_string();
                    session.detail = Some(msg.clone());
                    (false, Some(msg))
                }
            }
        };
        if ready {
            self.emit(id, SessionPhase::Ready, None);
            Ok(())
        } else {
            let detail = fail_detail.expect("failed reconnect");
            self.emit(id, SessionPhase::Failed, Some(detail.clone()));
            Err(SessionError::Connect(detail))
        }
    }

    pub async fn ensure_metadata(
        &mut self,
        id: ConnectionId,
        runtime: tokio::runtime::Handle,
    ) -> Result<(), SessionError> {
        if self
            .sessions
            .get(&id)
            .map(|s| s.metadata.is_some())
            .unwrap_or(false)
        {
            return Ok(());
        }
        let (profile, secrets) = {
            let session = self.sessions.get(&id).ok_or(SessionError::NotFound(id))?;
            (session.profile.clone(), session.secrets.clone())
        };
        let driver = tokio::task::spawn_blocking(move || {
            open_metadata_driver(&profile, &secrets, runtime)
        })
        .await
        .map_err(|e| SessionError::Connect(e.to_string()))??;
        if let Some(session) = self.sessions.get_mut(&id) {
            session.metadata = Some(driver);
        }
        Ok(())
    }
}

fn spawn_keepalive(
    manager: Arc<tokio::sync::Mutex<SessionManager>>,
    id: ConnectionId,
    interval: Duration,
    cancel: CancellationToken,
    runtime: tokio::runtime::Handle,
) {
    tokio::spawn(async move {
        let mut backoff = Duration::from_secs(1);
        loop {
            tokio::select! {
                () = cancel.cancelled() => break,
                () = tokio::time::sleep(interval) => {
                    let ping_ok = {
                        let mut guard = manager.lock().await;
                        let Some(session) = guard.sessions.get_mut(&id) else { break };
                        if session.phase != SessionPhase::Ready {
                            continue;
                        }
                        let Some(driver) = session.main.as_mut() else { continue };
                        driver.ping().is_ok()
                    };
                    if ping_ok {
                        backoff = Duration::from_secs(1);
                        continue;
                    }
                    loop {
                        if cancel.is_cancelled() {
                            return;
                        }
                        {
                            let mut guard = manager.lock().await;
                            if guard.sessions.get(&id).is_none() {
                                return;
                            }
                        }
                        let reconnected = {
                            let mut guard = manager.lock().await;
                            guard.reconnect(id, runtime.clone()).await.is_ok()
                        };
                        if reconnected {
                            backoff = Duration::from_secs(1);
                            break;
                        }
                        tokio::time::sleep(backoff).await;
                        backoff = (backoff * 2).min(Duration::from_secs(30));
                    }
                }
            }
        }
    });
}

#[derive(Debug, Clone)]
pub struct SessionOpenSpec {
    pub profile: wisp_store::ConnectionProfile,
    pub secrets: SessionSecrets,
}
