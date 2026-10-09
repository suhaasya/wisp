//! Queue and shared boot state for additional connection windows (LUM-026).

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use gpui::{BorrowAppContext, Global};
use wisp_core::{ConnectionHub, ConnectionId, WorkspaceSessionStore};

use crate::launch::{
    AppearanceConfig, JournalShutdownRegistry, PendingJournal, SharedSettingsInbox, ShellMetrics,
    WindowPersistence,
};

#[derive(Clone, Debug)]
pub struct SecondaryWindowRequest {
    pub connection_id: ConnectionId,
}

#[derive(Clone, Default)]
pub struct WindowOpenQueue(Rc<std::sync::Mutex<Vec<SecondaryWindowRequest>>>);

impl WindowOpenQueue {
    pub fn request(&self, connection_id: ConnectionId) {
        if let Ok(mut guard) = self.0.lock() {
            guard.push(SecondaryWindowRequest { connection_id });
        }
    }

    pub fn drain(&self) -> Vec<SecondaryWindowRequest> {
        self.0
            .lock()
            .map(|mut g| g.drain(..).collect())
            .unwrap_or_default()
    }
}

#[derive(Clone)]
pub struct ShellBoot {
    pub persistence: Rc<RefCell<WindowPersistence>>,
    pub metrics: Rc<RefCell<ShellMetrics>>,
    pub appearance: Rc<RefCell<AppearanceConfig>>,
    pub settings_inbox: Option<SharedSettingsInbox>,
    pub connections: Option<Arc<ConnectionHub>>,
    pub workspace_sessions: Arc<Mutex<WorkspaceSessionStore>>,
    pub window_open_queue: WindowOpenQueue,
    pub pending_journals: Vec<PendingJournal>,
    pub journal_shutdown: JournalShutdownRegistry,
}

#[derive(Clone)]
pub struct WindowSpawner {
    pub boot: Rc<ShellBoot>,
    pub queue: WindowOpenQueue,
}

impl Global for WindowSpawner {}

pub fn drain_pending_windows(cx: &mut gpui::App) {
    let (boot, pending) = cx.update_global::<WindowSpawner, _>(|spawner, _| {
        (spawner.boot.clone(), spawner.queue.drain())
    });
    if pending.is_empty() {
        return;
    }
    for (i, req) in pending.into_iter().enumerate() {
        let offset = 40.0 * (i as f32 + 1.0);
        let bounds = crate::app::offset_window_bounds(&boot.persistence.borrow(), offset, cx);
        crate::app::open_shell_window(cx, &boot, bounds, Some(req.connection_id), Vec::new());
    }
}
