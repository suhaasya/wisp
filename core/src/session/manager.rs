//! Owns live database sessions: connect, keep-alive, reconnect, safety gates.

use std::{
    collections::HashMap,
    sync::Arc,
    time::Duration,
};

use crossbeam_channel::{Receiver, Sender, TryRecvError};
use tokio_util::sync::CancellationToken;
use wisp_drivers::{DbDriver, ExecuteStats, PageRequest};
use wisp_store::ConnectionId;

use super::{
    config::SessionRuntimeConfig,
    driver::{apply_read_only_session, open_main_driver, open_metadata_driver, SessionSecrets},
    error::SessionError,
    policy::check_write_allowed,
    script::{RunScriptReport, ScriptStatement, StatementRunOutcome},
    state::{SessionEvent, SessionPhase, SessionSnapshot},
};
use crate::schema::load_schema_catalog;
use crate::sql_editor::{format_explain_json, sql_is_explain, sql_returns_rows};
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

    /// Run statements in one transaction; rolls back on any failure.
    pub async fn commit_transaction(
        manager: &Arc<tokio::sync::Mutex<Self>>,
        id: ConnectionId,
        statements: Vec<String>,
        write_approved: bool,
    ) -> Result<CommitTransactionOutcome, SessionError> {
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
        for sql in &statements {
            check_write_allowed(read_only, safe_mode, write_approved, sql)?;
        }

        let mut guard = manager.lock().await;
        let session = guard.sessions.get_mut(&id).ok_or(SessionError::NotFound(id))?;
        session.phase = SessionPhase::Busy;
        let driver = session.main.as_mut().ok_or(SessionError::NotReady {
            phase: SessionPhase::Reconnecting,
        })?;
        driver
            .begin()
            .map_err(|e| SessionError::Query(e.to_string()))?;
        let mut rows_affected = 0u64;
        for (index, sql) in statements.iter().enumerate() {
            match driver.execute(sql) {
                Ok(stats) => rows_affected += stats.rows_affected,
                Err(err) => {
                    let _ = driver.rollback();
                    session.phase = SessionPhase::Ready;
                    return Err(SessionError::CommitFailed {
                        index,
                        message: err.to_string(),
                    });
                }
            }
        }
        if let Err(err) = driver.commit() {
            let _ = driver.rollback();
            session.phase = SessionPhase::Ready;
            return Err(SessionError::CommitFailed {
                index: statements.len().saturating_sub(1),
                message: err.to_string(),
            });
        }
        session.phase = SessionPhase::Ready;
        Ok(CommitTransactionOutcome {
            statements_run: statements.len(),
            rows_affected,
        })
    }

    pub async fn run_script(
        manager: &Arc<tokio::sync::Mutex<Self>>,
        id: ConnectionId,
        statements: Vec<ScriptStatement>,
        write_approved: bool,
        cancel: CancellationToken,
    ) -> Result<RunScriptReport, SessionError> {
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
        for stmt in &statements {
            check_write_allowed(read_only, safe_mode, write_approved, &stmt.sql)?;
        }

        let mut outcomes = Vec::with_capacity(statements.len());
        for (stmt_index, stmt) in statements.into_iter().enumerate() {
            if cancel.is_cancelled() {
                return Ok(RunScriptReport {
                    outcomes,
                    cancelled: true,
                });
            }
            let sql = stmt.sql.trim().to_string();
            if sql.is_empty() {
                continue;
            }
            let manager = Arc::clone(manager);
            let byte_start = stmt.byte_start;
            let outcome = tokio::task::spawn_blocking(move || {
                manager.blocking_lock().run_one_statement(
                    id,
                    &sql,
                    stmt_index,
                    byte_start,
                )
            })
            .await
            .map_err(|e| SessionError::Query(e.to_string()))?;

            let outcome = match outcome {
                Ok(o) => o,
                Err(SessionError::Query(message)) => {
                    if message.contains("cancelled") {
                        return Ok(RunScriptReport {
                            outcomes,
                            cancelled: true,
                        });
                    }
                    StatementRunOutcome::Failed {
                        stmt_index,
                        byte_start,
                        message,
                    }
                }
                Err(err) => return Err(err),
            };

            outcomes.push(outcome.clone());
            if matches!(outcome, StatementRunOutcome::Failed { .. }) {
                break;
            }
            if cancel.is_cancelled() {
                return Ok(RunScriptReport {
                    outcomes,
                    cancelled: true,
                });
            }
        }
        Ok(RunScriptReport {
            outcomes,
            cancelled: false,
        })
    }

    pub fn cancel_in_flight_query(
        manager: &Arc<tokio::sync::Mutex<Self>>,
        id: ConnectionId,
    ) {
        let mut guard = manager.blocking_lock();
        let Some(session) = guard.sessions.get_mut(&id) else {
            return;
        };
        let Some(driver) = session.main.as_mut() else {
            return;
        };
        if let Some(qid) = driver.in_flight_query() {
            let _ = driver.cancel(qid);
        }
    }

    pub fn query_page_blocking(
        manager: &Arc<tokio::sync::Mutex<Self>>,
        id: ConnectionId,
        sql: &str,
        offset: u64,
        limit: u32,
    ) -> Result<wisp_drivers::Page, SessionError> {
        manager
            .blocking_lock()
            .query_page_locked(id, sql, offset, limit)
    }

    fn query_page_locked(
        &mut self,
        id: ConnectionId,
        sql: &str,
        offset: u64,
        limit: u32,
    ) -> Result<wisp_drivers::Page, SessionError> {
        let session = self.sessions.get_mut(&id).ok_or(SessionError::NotFound(id))?;
        if !matches!(session.phase, SessionPhase::Ready | SessionPhase::Busy) {
            return Err(SessionError::NotReady {
                phase: session.phase,
            });
        }
        session.phase = SessionPhase::Busy;
        let driver = session.main.as_mut().ok_or(SessionError::NotReady {
            phase: SessionPhase::Reconnecting,
        })?;
        let result = driver
            .query_paged(
                sql,
                PageRequest {
                    limit: limit.max(1),
                    offset,
                },
            )
            .map_err(|e| SessionError::Query(e.to_string()));
        session.phase = SessionPhase::Ready;
        result
    }

    fn run_one_statement(
        &mut self,
        id: ConnectionId,
        sql: &str,
        stmt_index: usize,
        byte_start: usize,
    ) -> Result<StatementRunOutcome, SessionError> {
        let session = self.sessions.get_mut(&id).ok_or(SessionError::NotFound(id))?;
        session.phase = SessionPhase::Busy;
        let driver = session.main.as_mut().ok_or(SessionError::NotReady {
            phase: SessionPhase::Reconnecting,
        })?;

        let driver = driver.as_mut();
        let result = if sql_is_explain(sql) {
            run_explain_on_driver(driver, sql, stmt_index, byte_start)
        } else if sql_returns_rows(sql) {
            run_query_on_driver(driver, sql, stmt_index, byte_start)
        } else {
            run_execute_on_driver(driver, sql, stmt_index, byte_start)
        };

        if let Some(session) = self.sessions.get_mut(&id) {
            session.phase = SessionPhase::Ready;
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
        manager: &Arc<tokio::sync::Mutex<Self>>,
        id: ConnectionId,
        runtime: tokio::runtime::Handle,
    ) -> Result<(), SessionError> {
        {
            let guard = manager.lock().await;
            if guard
                .sessions
                .get(&id)
                .map(|s| s.metadata.is_some())
                .unwrap_or(false)
            {
                return Ok(());
            }
        }
        let (profile, secrets) = {
            let guard = manager.lock().await;
            let session = guard.sessions.get(&id).ok_or(SessionError::NotFound(id))?;
            (session.profile.clone(), session.secrets.clone())
        };
        let driver = tokio::task::spawn_blocking(move || {
            open_metadata_driver(&profile, &secrets, runtime)
        })
        .await
        .map_err(|e| SessionError::Connect(e.to_string()))??;
        if let Some(session) = manager.lock().await.sessions.get_mut(&id) {
            session.metadata = Some(driver);
        }
        Ok(())
    }

    pub fn fetch_schema_catalog_blocking(
        manager: &Arc<tokio::sync::Mutex<Self>>,
        id: ConnectionId,
    ) -> Result<crate::schema::SchemaLoadResult, SessionError> {
        manager.blocking_lock().fetch_schema_catalog_locked(id)
    }

    fn fetch_schema_catalog_locked(
        &mut self,
        id: ConnectionId,
    ) -> Result<crate::schema::SchemaLoadResult, SessionError> {
        let session = self.sessions.get_mut(&id).ok_or(SessionError::NotFound(id))?;
        if !matches!(session.phase, SessionPhase::Ready | SessionPhase::Busy) {
            return Err(SessionError::NotReady {
                phase: session.phase,
            });
        }
        let profile = session.profile.clone();
        let engine = profile.engine;
        let driver = session
            .metadata
            .as_mut()
            .map(|d| d.as_mut() as &mut dyn DbDriver)
            .or_else(|| session.main.as_mut().map(|d| d.as_mut() as &mut dyn DbDriver))
            .ok_or(SessionError::NotReady {
                phase: SessionPhase::Reconnecting,
            })?;
        load_schema_catalog(driver, engine, &profile)
            .map_err(|e| SessionError::Query(e.to_string()))
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

fn run_execute_on_driver(
    driver: &mut dyn DbDriver,
    sql: &str,
    stmt_index: usize,
    byte_start: usize,
) -> Result<StatementRunOutcome, SessionError> {
    let stats = driver
        .execute(sql)
        .map_err(|e| SessionError::Query(e.to_string()))?;
    Ok(StatementRunOutcome::Message {
        stmt_index,
        byte_start,
        rows_affected: stats.rows_affected,
        text: format!("{} row(s) affected", stats.rows_affected),
    })
}

fn run_query_on_driver(
    driver: &mut dyn DbDriver,
    sql: &str,
    stmt_index: usize,
    byte_start: usize,
) -> Result<StatementRunOutcome, SessionError> {
    let page = driver
        .query_paged(
            sql,
            PageRequest {
                limit: 200,
                offset: 0,
            },
        )
        .map_err(|e| SessionError::Query(e.to_string()))?;
    let columns = page.columns.clone();
    let row_count = infer_row_count(&page);
    Ok(StatementRunOutcome::ResultSet {
        stmt_index,
        byte_start,
        sql: sql.to_string(),
        columns,
        row_count,
    })
}

fn run_explain_on_driver(
    driver: &mut dyn DbDriver,
    sql: &str,
    stmt_index: usize,
    byte_start: usize,
) -> Result<StatementRunOutcome, SessionError> {
    let page = driver
        .query_paged(
            sql,
            PageRequest {
                limit: 1,
                offset: 0,
            },
        )
        .map_err(|e| SessionError::Query(e.to_string()))?;
    let raw = page.text_at(0, 0).unwrap_or("").to_string();
    let plan_text = format_explain_json(&raw);
    Ok(StatementRunOutcome::Explain {
        stmt_index,
        byte_start,
        plan_text,
    })
}

fn infer_row_count(page: &wisp_drivers::Page) -> crate::pager::RowCount {
    use crate::pager::RowCount;
    if page.row_count == 0 {
        return RowCount::Exact(0);
    }
    if page.row_count < 200 {
        RowCount::Exact(page.row_count as u64)
    } else {
        RowCount::Unknown
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommitTransactionOutcome {
    pub statements_run: usize,
    pub rows_affected: u64,
}

#[derive(Debug, Clone)]
pub struct SessionOpenSpec {
    pub profile: wisp_store::ConnectionProfile,
    pub secrets: SessionSecrets,
}
