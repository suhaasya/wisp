use gpui::{actions, App, KeyBinding};

actions!(wisp_connections, [NewConnection, PasteConnectionUrl]);

pub fn bind_connection_keys(cx: &mut App) {
    cx.bind_keys([
        KeyBinding::new("cmd-n", NewConnection, None),
        KeyBinding::new("cmd-v", PasteConnectionUrl, Some("WispConnections")),
    ]);
}
