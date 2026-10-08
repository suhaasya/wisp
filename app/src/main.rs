//! Wisp application entry point.

mod bench;

use clap::Parser;
use wisp_store::settings::{
    AppearanceSettings, DensitySetting, MonoFontSetting, Settings, ThemeModeSetting, UiFontSetting,
};
use wisp_store::window::WindowState;
use wisp_ui::{
    AppearanceConfig, Density, LaunchConfig, MonoFontChoice, ThemeMode, UiFontChoice,
    WindowGeometry, WindowPersistence,
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

    let mut settings = Settings::load();
    let stored = WindowState::load();
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
        appearance: appearance_from_settings(&settings.appearance),
    };

    let outcome = wisp_ui::run(config)?;

    settings.appearance = appearance_to_settings(&outcome.appearance);
    settings.save()?;

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

    Ok(())
}

fn appearance_from_settings(value: &AppearanceSettings) -> AppearanceConfig {
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
