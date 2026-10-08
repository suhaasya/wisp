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
use crate::error::WispError;

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
}

/// Handle to the background DB/network runtime. [`submit`](Self::submit) is non-blocking.
pub struct DbBridge {
    submit_tx: Sender<Job>,
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

        let runtime_thread = thread::Builder::new()
            .name("wisp-tokio".into())
            .spawn(move || {
                enter_runtime_thread();
                let rt = tokio::runtime::Builder::new_multi_thread()
                    .worker_threads(config.worker_threads)
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
                    while let Some(job) = tokio_rx.recv().await {
                        tokio::spawn(run_job(job));
                    }
                });
            })
            .expect("spawn tokio thread");

        Self {
            submit_tx,
            _runtime_thread: runtime_thread,
        }
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

    let result = execute(job.payload, job.cancel).await;
    let _ = job.done.send(DbEvent::completed(job.id, result));
}

async fn execute(
    payload: DbCommandPayload,
    cancel: CancellationToken,
) -> Result<DbEventPayload, WispError> {
    match payload {
        DbCommandPayload::Seq(n) => Ok(DbEventPayload::Seq(n)),
        DbCommandPayload::SleepMs(ms) => {
            tokio::select! {
                () = cancel.cancelled() => Err(WispError::cancelled()),
                () = tokio::time::sleep(Duration::from_millis(ms)) => Ok(DbEventPayload::Unit),
            }
        }
    }
}
