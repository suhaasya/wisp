//! Multi-thread tokio runtime (2 workers by default) behind a channel bridge.

use std::{
    fmt,
    thread::{self, JoinHandle},
    time::Duration,
};

use crossbeam_channel::{Receiver, Sender};
use tokio_util::sync::CancellationToken;

use super::{
    assert_no_block_on_ui, enter_runtime_thread, payload::DbCommandPayload, payload::DbEvent,
    payload::DbEventPayload, RequestId,
};
use std::sync::Arc;

use crate::{
    connections::run_connection_test,
    error::WispError,
    session::{SessionError, SessionManager, SessionRuntimeConfig},
};

/// Tokio pool sizing (each worker uses ~2 MB stack; see PRD budget note).
#[derive(Debug, Clone)]
pub struct DbRuntimeConfig {
    pub worker_threads: usize,
}

impl Default for DbRuntimeConfig {
    fn default() -> Self {
        Self {
            worker_threads: 2,
        }
    }
}

struct Job {
    id: RequestId,
    payload: DbCommandPayload,
    cancel: CancellationToken,
    done: Sender<DbEvent>,
    sessions: Arc<tokio::sync::Mutex<SessionManager>>,
}

/// Handle to the background DB/network runtime. [`submit`](Self::submit) is non-blocking.
pub struct DbBridge {
    submit_tx: Sender<Job>,
    sessions: Arc<tokio::sync::Mutex<SessionManager>>,
    _runtime_thread: JoinHandle<()>,
}

impl fmt::Debug for DbBridge {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DbBridge").finish_non_exhaustive()
    }
}

impl DbBridge {
    pub fn start(config: DbRuntimeConfig) -> Self {
        assert_no_block_on_ui();
        let (submit_tx, submit_rx) = crossbeam_channel::unbounded::<Job>();
        let sessions = SessionManager::new(SessionRuntimeConfig::default());

        let sessions_for_thread = Arc::clone(&sessions);
        let runtime_thread = thread::Builder::new()
            .name("wisp-tokio".into())
            .spawn(move || {
                enter_runtime_thread();
                let rt = tokio::runtime::Builder::new_multi_thread()
                    .worker_threads(config.worker_threads)
                    .enable_io()
                    .enable_time()
                    .thread_name("wisp-db-worker")
                    .build()
                    .expect("tokio runtime");
                assert_no_block_on_ui();
                let (tokio_tx, mut tokio_rx) = tokio::sync::mpsc::unbounded_channel();
                thread::spawn(move || {
                    while let Ok(job) = submit_rx.recv() {
                        let _ = tokio_tx.send(job);
                    }
                });
                rt.block_on(async move {
                    let _sessions = sessions_for_thread;
                    while let Some(job) = tokio_rx.recv().await {
                        tokio::spawn(run_job(job));
                    }
                });
            })
            .expect("spawn tokio thread");

        Self {
            submit_tx,
            sessions,
            _runtime_thread: runtime_thread,
        }
    }

    pub fn sessions(&self) -> Arc<tokio::sync::Mutex<SessionManager>> {
        Arc::clone(&self.sessions)
    }

    pub fn poll_session_event(&self) -> Option<crate::session::SessionEvent> {
        SessionManager::try_recv_event(&self.sessions)
    }

    /// Block until all live sessions are closed (call before process exit).
    pub fn shutdown_sessions(&self) {
        let (done_tx, done_rx) = crossbeam_channel::bounded(1);
        let job = Job {
            id: RequestId::next(),
            payload: DbCommandPayload::ShutdownAllSessions,
            cancel: CancellationToken::new(),
            done: done_tx,
            sessions: Arc::clone(&self.sessions),
        };
        let _ = self.submit_tx.send(job);
        let _ = done_rx.recv();
    }

    /// Enqueue work on the tokio pool. Returns immediately without waiting for completion.
    pub fn submit(
        &self,
        payload: DbCommandPayload,
    ) -> (RequestId, CancellationToken, Receiver<DbEvent>) {
        let id = RequestId::next();
        let cancel = CancellationToken::new();
        let (done_tx, done_rx) = crossbeam_channel::bounded(1);
        let job = Job {
            id,
            payload,
            cancel: cancel.clone(),
            done: done_tx,
            sessions: Arc::clone(&self.sessions),
        };
        self.submit_tx
            .send(job)
            .expect("tokio bridge thread stopped");
        (id, cancel, done_rx)
    }
}

async fn run_job(job: Job) {
    if job.cancel.is_cancelled() {
        let _ = job.done.send(DbEvent::completed(
            job.id,
            Err(WispError::cancelled()),
        ));
        return;
    }

    let result = execute(job.payload, job.cancel, job.sessions).await;
    let _ = job.done.send(DbEvent::completed(job.id, result));
}

async fn execute(
    payload: DbCommandPayload,
    cancel: CancellationToken,
    sessions: Arc<tokio::sync::Mutex<SessionManager>>,
) -> Result<DbEventPayload, WispError> {
    let runtime = tokio::runtime::Handle::current();
    match payload {
        DbCommandPayload::Seq(n) => Ok(DbEventPayload::Seq(n)),
        DbCommandPayload::SleepMs(ms) => {
            tokio::select! {
                () = cancel.cancelled() => Err(WispError::cancelled()),
                () = tokio::time::sleep(Duration::from_millis(ms)) => Ok(DbEventPayload::Unit),
            }
        }
        DbCommandPayload::TestConnection(spec) => {
            let result = run_connection_test(spec, cancel).await;
            Ok(DbEventPayload::ConnectionTest(result))
        }
        DbCommandPayload::SessionOpen(spec) => {
            let result = SessionManager::open(&sessions, spec, runtime).await;
            Ok(DbEventPayload::SessionOpen(result))
        }
        DbCommandPayload::SessionClose(id) => {
            SessionManager::close(&sessions, id).await;
            Ok(DbEventPayload::SessionClose)
        }
        DbCommandPayload::SessionExecute {
            id,
            sql,
            write_approved,
        } => {
            let result = SessionManager::execute(&sessions, id, &sql, write_approved).await;
            Ok(DbEventPayload::SessionExecute(result))
        }
        DbCommandPayload::SessionCommitTransaction {
            id,
            statements,
            write_approved,
        } => {
            let result =
                SessionManager::commit_transaction(&sessions, id, statements, write_approved)
                    .await;
            Ok(DbEventPayload::SessionCommitTransaction(result))
        }
        DbCommandPayload::SessionSnapshot(id) => {
            let snap = SessionManager::snapshot(&sessions, id).await;
            Ok(DbEventPayload::SessionSnapshot(snap))
        }
        DbCommandPayload::SessionRunScript {
            id,
            statements,
            write_approved,
        } => {
            let sessions_for_cancel = Arc::clone(&sessions);
            tokio::select! {
                () = cancel.cancelled() => {
                    SessionManager::cancel_in_flight_query(&sessions_for_cancel, id);
                    Err(WispError::cancelled())
                }
                result = SessionManager::run_script(&sessions, id, statements, write_approved, cancel.clone()) => {
                    Ok(DbEventPayload::SessionRunScript(result))
                }
            }
        }
        DbCommandPayload::SessionQueryPage {
            id,
            sql,
            offset,
            limit,
        } => {
            let sessions = Arc::clone(&sessions);
            let page = tokio::task::spawn_blocking(move || {
                SessionManager::query_page_blocking(&sessions, id, &sql, offset, limit)
            })
            .await
            .map_err(|e| WispError::internal("query page task failed", &e.to_string()))?;
            let snapshot = page.map(|p| crate::session::QueryPageSnapshot::from_page(&p));
            Ok(DbEventPayload::SessionQueryPage(snapshot))
        }
        DbCommandPayload::SessionCancelQuery(id) => {
            SessionManager::cancel_in_flight_query(&sessions, id);
            Ok(DbEventPayload::SessionCancelQuery)
        }
        DbCommandPayload::SessionFetchSchemaCatalog(id) => {
            let _ = SessionManager::ensure_metadata(&sessions, id, runtime.clone()).await;
            let sessions = Arc::clone(&sessions);
            let result = match tokio::task::spawn_blocking(move || {
                SessionManager::fetch_schema_catalog_blocking(&sessions, id)
            })
            .await
            {
                Ok(inner) => inner,
                Err(e) => Err(SessionError::Connect(e.to_string())),
            };
            Ok(DbEventPayload::SessionSchemaCatalog(result))
        }
        DbCommandPayload::ShutdownAllSessions => {
            SessionManager::shutdown_all(&sessions).await;
            Ok(DbEventPayload::ShutdownAllSessions)
        }
    }
}
