//! GPUI helpers for the tokio DB bridge (LUM-010).

pub mod sql_fetch;

use std::sync::Arc;

use gpui::{App, Context, Global, Task, WeakEntity};
use tokio_util::sync::CancellationToken;
use wisp_core::{
    bridge::RequestId, DbBridge, DbCommandPayload, DbEvent, DbEventPayload, WispError,
};

struct DbBridgeGlobal(Arc<DbBridge>);

impl Global for DbBridgeGlobal {}

/// Install the shared bridge and mark the GPUI thread for debug guards.
pub fn init_db_bridge(cx: &mut App, bridge: Arc<DbBridge>) {
    wisp_core::enter_ui_thread();
    cx.set_global(DbBridgeGlobal(bridge));
}

pub fn db_bridge(cx: &App) -> Arc<DbBridge> {
    cx.global::<DbBridgeGlobal>().0.clone()
}

/// Enqueue DB work and update the view when the matching event arrives (non-blocking on UI thread).
pub fn spawn_db<T: 'static>(
    cx: &Context<T>,
    bridge: &DbBridge,
    payload: DbCommandPayload,
    on_done: impl FnOnce(&mut T, &mut Context<T>, Result<DbEventPayload, WispError>) + 'static,
) -> (RequestId, CancellationToken, Task<()>) {
    let (request_id, cancel, done_rx) = bridge.submit(payload);
    let task = cx.spawn(async move |this: WeakEntity<T>, cx| {
        let event = cx
            .background_executor()
            .spawn(async move {
                done_rx.recv().unwrap_or_else(|_| {
                    DbEvent::completed(
                        request_id,
                        Err(WispError::internal(
                            "Database worker stopped",
                            "bridge channel closed",
                        )),
                    )
                })
            })
            .await;
        let result = event.result();
        this.update(cx, |view, cx| on_done(view, cx, result)).ok();
    });
    (request_id, cancel, task)
}
