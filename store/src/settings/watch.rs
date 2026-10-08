//! Watch `settings.toml` for external edits.

use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
    sync::mpsc::{self, Receiver},
    thread,
    time::Duration,
};

use notify::{EventKind, RecursiveMode, Watcher};

use super::parse::SettingsParseError;
use super::schema::Settings;
use super::SettingsStore;

#[derive(Debug, Clone)]
pub enum SettingsWatchEvent {
    Reloaded(Settings),
    ParseError(SettingsParseError),
}

/// Background thread: debounced reload on file change.
pub fn spawn_settings_watcher(
    store: Arc<Mutex<SettingsStore>>,
) -> (Receiver<SettingsWatchEvent>, thread::JoinHandle<()>) {
    let (event_tx, event_rx) = mpsc::channel();
    let path: PathBuf = store
        .lock()
        .expect("settings store lock")
        .path()
        .to_path_buf();

    let handle = thread::spawn(move || {
        let (signal_tx, signal_rx) = mpsc::channel();
        let watch_path = path.clone();
        let mut watcher = notify::recommended_watcher(
            move |res: Result<notify::Event, notify::Error>| {
            if let Ok(event) = res {
                if matches!(
                    event.kind,
                    EventKind::Modify(_) | EventKind::Create(_) | EventKind::Remove(_)
                ) && event.paths.iter().any(|p| p == &watch_path)
                {
                    let _ = signal_tx.send(());
                }
            }
            },
        )
        .expect("settings watcher");

        if let Some(dir) = path.parent() {
            let _ = watcher.watch(dir, RecursiveMode::NonRecursive);
        }

        while signal_rx.recv().is_ok() {
            thread::sleep(Duration::from_millis(150));
            let Ok(mut locked) = store.lock() else {
                continue;
            };
            if !path.is_file() {
                let defaults = Settings::default();
                locked.replace(defaults.clone());
                let _ = event_tx.send(SettingsWatchEvent::Reloaded(defaults));
                continue;
            }
            match locked.try_reload() {
                Ok(settings) => {
                    let _ = event_tx.send(SettingsWatchEvent::Reloaded(settings));
                }
                Err(err) => {
                    let _ = event_tx.send(SettingsWatchEvent::ParseError(err));
                }
            }
        }
    });

    (event_rx, handle)
}
