use gpui::Rgba;
use wisp_core::EnvironmentTag;

use crate::theme::EnvColors;

pub fn env_edge_colour(tag: &EnvironmentTag, env: &EnvColors) -> Rgba {
    match tag {
        EnvironmentTag::Production => env.production,
        EnvironmentTag::Staging => env.staging,
        EnvironmentTag::Development => env.dev,
        EnvironmentTag::Custom(label) if label.eq_ignore_ascii_case("local") => env.local,
        EnvironmentTag::Custom(_) => env.dev,
    }
}

pub fn parse_edge_colour(hex: &str) -> Option<Rgba> {
    let hex = hex.trim_start_matches('#');
    if hex.len() != 6 {
        return None;
    }
    let value = u32::from_str_radix(hex, 16).ok()?;
    Some(Rgba {
        r: ((value >> 16) & 0xff) as f32 / 255.,
        g: ((value >> 8) & 0xff) as f32 / 255.,
        b: (value & 0xff) as f32 / 255.,
        a: 1.,
    })
}
