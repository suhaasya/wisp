//! Debounced background journal writer (1 s, off UI thread).

use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use uuid::Uuid;

use super::io::{remove_journal, write_journal_atomic};
use super::schema::WindowJournal;

const DEBOUNCE: Duration = Duration::from_secs(1);

enum WriterMsg {
    Snapshot(WindowJournal),
    Clear,
}

struct JournalWriterInner {
    tx: Mutex<Option<SyncSender<WriterMsg>>>,
    path: PathBuf,
    window_id: Uuid,
    join: Mutex<Option<JoinHandle<()>>>,
}

#[derive(Clone)]
pub struct JournalWriter {
    inner: Arc<JournalWriterInner>,
}

impl JournalWriter {
    pub fn spawn_for_window(paths: &crate::WispPaths) -> Self {
        let window_id = Uuid::new_v4();
        Self::spawn(paths.journal_file(window_id), window_id)
    }

    pub fn spawn(path: PathBuf, window_id: Uuid) -> Self {
        let (tx, rx) = mpsc::sync_channel(64);
        let thread_path = path.clone();
        let join = thread::Builder::new()
            .name("wisp-journal".into())
            .spawn(move || writer_loop(rx, thread_path))
            .expect("journal writer thread");
        Self {
            inner: Arc::new(JournalWriterInner {
                tx: Mutex::new(Some(tx)),
                path,
                window_id,
                join: Mutex::new(Some(join)),
            }),
        }
    }

    pub fn window_id(&self) -> Uuid {
        self.inner.window_id
    }

    pub fn path(&self) -> &PathBuf {
        &self.inner.path
    }

    pub fn schedule(&self, mut snapshot: WindowJournal) {
        snapshot.window_id = self.inner.window_id;
        if let Ok(guard) = self.inner.tx.lock() {
            if let Some(tx) = guard.as_ref() {
                let _ = tx.send(WriterMsg::Snapshot(snapshot));
            }
        }
    }

    /// Remove the journal file on clean shutdown (no recovery prompt next launch).
    pub fn clear_on_exit(&self) {
        if let Ok(guard) = self.inner.tx.lock() {
            if let Some(tx) = guard.as_ref() {
                let _ = tx.send(WriterMsg::Clear);
            }
        }
        self.shutdown_thread();
    }

    fn shutdown_thread(&self) {
        if let Ok(mut tx_guard) = self.inner.tx.lock() {
            tx_guard.take();
        }
        if let Ok(mut join_guard) = self.inner.join.lock() {
            if let Some(join) = join_guard.take() {
                let _ = join.join();
            }
        }
    }
}

impl Drop for JournalWriter {
    fn drop(&mut self) {
        if Arc::strong_count(&self.inner) == 1 {
            self.shutdown_thread();
        }
    }
}

fn writer_loop(rx: Receiver<WriterMsg>, path: PathBuf) {
    let mut pending: Option<WindowJournal> = None;
    let mut debounce_deadline: Option<Instant> = None;

    loop {
        let timeout = debounce_deadline
            .map(|d| d.saturating_duration_since(Instant::now()))
            .unwrap_or(Duration::from_secs(3600));

        match rx.recv_timeout(timeout) {
            Ok(WriterMsg::Snapshot(doc)) => {
                pending = Some(doc);
                debounce_deadline = Some(Instant::now() + DEBOUNCE);
            }
            Ok(WriterMsg::Clear) => {
                pending = None;
                debounce_deadline = None;
                let _ = remove_journal(&path);
                continue;
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                if let Some(doc) = pending.take() {
                    let _ = write_journal_atomic(&path, &doc);
                }
                debounce_deadline = None;
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                if let Some(doc) = pending.take() {
                    let _ = write_journal_atomic(&path, &doc);
                }
                break;
            }
        }
    }
}
