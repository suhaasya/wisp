//! Wisp application entry point.

mod bench;
mod secrets;

use std::{
    sync::{Arc, Mutex},
    thread,
};

use clap::Parser;
use wisp_core::{ConnectionHub, ConnectionHubError, DbBridge, DbRuntimeConfig, WorkspaceSessionStore};
use wisp_store::{
    list_pending_journals,
    settings::{spawn_settings_watcher, SettingsStore, SettingsWatchEvent},
    window::WindowState,
    AppearanceSettings, ConnectionsLoadError, DensitySetting, MonoFontSetting, Settings,
    ThemeModeSetting, UiFontSetting, WispPaths,
};
use wisp_ui::{
    AppearanceConfig, Density, LaunchConfig, MonoFontChoice, PendingJournal, SettingsInbox,
    SettingsToast, ThemeMode, UiFontChoice, WindowGeometry, WindowOpenQueue, WindowPersistence,
};

#[derive(Parser)]
#[command(name = "wisp", version, about = "Wisp database client")]
struct Args {
    /// Hidden harness entry point for CI budget scenarios (LUM-004).
    #[arg(long, hide = true)]
    bench_scenario: Option<String>,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    if let Some(scenario) = args.bench_scenario {
        bench::run(&scenario);
    }

    #[cfg(feature = "bench-stress")]
    {
        let _ = &*bench_stress::STRESS;
    }

    let paths = WispPaths::resolve();
    let secrets = secrets::open_shared_secret_store(&paths)?;
    let _secret_store = wisp_store::BlockingSecretStore::new(secrets.clone());
    let db_bridge = Arc::new(DbBridge::start(DbRuntimeConfig::default()));

    let store = Arc::new(Mutex::new(SettingsStore::load()));
    let stored = WindowState::load();
    let settings_inbox = Arc::new(Mutex::new(SettingsInbox::default()));
    let connections = load_connection_hub(&paths, secrets, &settings_inbox);
    start_settings_watcher(Arc::clone(&store), settings_inbox.clone());

    let (appearance, row_detail_width) = {
        let guard = store.lock().expect("settings store lock");
        let settings = guard.get();
        (
            appearance_from_settings(settings.appearance.clone()),
            settings.grid.row_detail_width,
        )
    };

    let workspace_sessions = std::sync::Arc::new(std::sync::Mutex::new(WorkspaceSessionStore::load(
        &paths,
    )));
    let workspace_sessions_for_save = std::sync::Arc::clone(&workspace_sessions);

    let pending_journals = list_pending_journals(&paths.journal_dir())
        .unwrap_or_default()
        .into_iter()
        .map(|(path, doc)| PendingJournal { path, doc })
        .collect::<Vec<_>>();
    let journal_shutdown = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));

    let config = LaunchConfig {
        window: WindowPersistence {
            maximized: stored.maximized,
            geometry: stored.geometry.map(|g| WindowGeometry {
                width: g.width,
                height: g.height,
                x: g.x,
                y: g.y,
            }),
        },
        appearance,
        row_detail_width,
        settings_inbox: Some(settings_inbox.clone()),
        db_bridge: Some(Arc::clone(&db_bridge)),
        connections,
        workspace_sessions,
        window_open_queue: WindowOpenQueue::default(),
        initial_connection: None,
        pending_journals,
        journal_shutdown: std::sync::Arc::clone(&journal_shutdown),
    };

    let outcome = wisp_ui::run(config)?;
    db_bridge.shutdown_sessions();

    {
        let mut guard = store.lock().expect("settings store lock");
        let mut merged = merge_appearance(guard.get().clone(), &outcome.appearance);
        if let Ok(inbox) = settings_inbox.lock() {
            if let Some(w) = inbox.row_detail_width {
                merged.grid.row_detail_width = w;
            }
        }
        guard.set(merged);
        guard.save()?;
    }

    let saved = WindowState {
        maximized: outcome.window.maximized,
        geometry: outcome
            .window
            .geometry
            .map(|g| wisp_store::window::WindowGeometry {
                width: g.width,
                height: g.height,
                x: g.x,
                y: g.y,
            }),
    };
    saved.save()?;

    if let Ok(store) = workspace_sessions_for_save.lock() {
        let _ = store.save();
    }

    Ok(())
}

fn load_connection_hub(
    paths: &WispPaths,
    secrets: wisp_store::SharedSecretStore,
    inbox: &Arc<Mutex<SettingsInbox>>,
) -> Option<Arc<ConnectionHub>> {
    match ConnectionHub::load(paths.clone(), secrets.clone()) {
        Ok(hub) => Some(Arc::new(hub)),
        Err(ConnectionHubError::Load(ConnectionsLoadError::Corrupt { message, .. })) => {
            if let Ok(mut guard) = inbox.lock() {
                guard.toasts.push(SettingsToast {
                    line: None,
                    message,
                });
            }
            Some(Arc::new(ConnectionHub::empty_at(
                paths.connections_toml(),
                secrets,
            )))
        }
        Err(ConnectionHubError::Load(ConnectionsLoadError::Io(err))) => {
            eprintln!("connections: {err}");
            None
        }
        Err(err) => {
            eprintln!("connections: {err}");
            None
        }
    }
}

fn start_settings_watcher(store: Arc<Mutex<SettingsStore>>, inbox: Arc<Mutex<SettingsInbox>>) {
    let (rx, _handle) = spawn_settings_watcher(store);
    thread::spawn(move || {
        while let Ok(event) = rx.recv() {
            let Ok(mut guard) = inbox.lock() else {
                continue;
            };
            match event {
                SettingsWatchEvent::Reloaded(settings) => {
                    guard.appearance = Some(appearance_from_settings(settings.appearance));
                }
                SettingsWatchEvent::ParseError(err) => {
                    guard.toasts.push(SettingsToast {
                        line: err.line(),
                        message: err.to_string(),
                    });
                }
            }
        }
    });
}

fn merge_appearance(mut settings: Settings, appearance: &AppearanceConfig) -> Settings {
    settings.appearance = appearance_to_settings(appearance);
    settings
}

fn appearance_from_settings(value: AppearanceSettings) -> AppearanceConfig {
    AppearanceConfig {
        theme_mode: match value.theme_mode {
            ThemeModeSetting::Light => ThemeMode::Light,
            ThemeModeSetting::Dark => ThemeMode::Dark,
            ThemeModeSetting::System => ThemeMode::System,
        },
        density: match value.density {
            DensitySetting::Compact => Density::Compact,
            DensitySetting::Comfortable => Density::Comfortable,
        },
        ui_font: match value.ui_font {
            UiFontSetting::System => UiFontChoice::System,
            UiFontSetting::IbmPlexSans => UiFontChoice::IbmPlexSans,
        },
        mono_font: match value.mono_font {
            MonoFontSetting::System => MonoFontChoice::System,
            MonoFontSetting::IbmPlexMono => MonoFontChoice::IbmPlexMono,
        },
    }
}

fn appearance_to_settings(value: &AppearanceConfig) -> AppearanceSettings {
    AppearanceSettings {
        theme_mode: match value.theme_mode {
            ThemeMode::Light => ThemeModeSetting::Light,
            ThemeMode::Dark => ThemeModeSetting::Dark,
            ThemeMode::System => ThemeModeSetting::System,
        },
        density: match value.density {
            Density::Compact => DensitySetting::Compact,
            Density::Comfortable => DensitySetting::Comfortable,
        },
        ui_font: match value.ui_font {
            UiFontChoice::System => UiFontSetting::System,
            UiFontChoice::IbmPlexSans => UiFontSetting::IbmPlexSans,
        },
        mono_font: match value.mono_font {
            MonoFontChoice::System => MonoFontSetting::System,
            MonoFontChoice::IbmPlexMono => MonoFontSetting::IbmPlexMono,
        },
    }
}

#[cfg(feature = "bench-stress")]
mod bench_stress {
    pub static STRESS: [u8; 30 * 1024 * 1024] = [0; 30 * 1024 * 1024];
}

#[cfg(test)]
mod tests {
    #[test]
    fn smoke() {
        assert_eq!(wisp_ui::CRATE_MARKER, "wisp-ui");
    }
}
