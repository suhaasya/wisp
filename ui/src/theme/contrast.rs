//! WCAG 2.x contrast helpers for token pairs.

use super::tokens::ColorTokens;

const AA_NORMAL: f64 = 4.5;
const AA_LARGE: f64 = 3.0;
pub const ENV_LABEL_DARK: u32 = 0x1D222B;
pub const ENV_LABEL_LIGHT: u32 = 0xFFFFFF;

#[derive(Debug, Clone, Copy)]
pub struct ContrastPair {
    pub name: &'static str,
    pub foreground: u32,
    pub background: u32,
    pub min_ratio: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ContrastResult {
    pub name: &'static str,
    pub ratio: f64,
    pub required: f64,
    pub passed: bool,
}

/// Foreground for labels on saturated env badge backgrounds.
pub fn env_label_foreground(background: u32, dark: u32, light: u32) -> u32 {
    let dark_ratio = contrast_ratio(dark, background);
    let light_ratio = contrast_ratio(light, background);
    if dark_ratio >= AA_NORMAL && light_ratio >= AA_NORMAL {
        if dark_ratio >= light_ratio {
            dark
        } else {
            light
        }
    } else if dark_ratio >= AA_NORMAL {
        dark
    } else if light_ratio >= AA_NORMAL {
        light
    } else if dark_ratio >= light_ratio {
        dark
    } else {
        light
    }
}

fn env_on(_tokens: &ColorTokens, background: u32) -> u32 {
    env_label_foreground(background, ENV_LABEL_DARK, ENV_LABEL_LIGHT)
}

pub fn wcag_pairs(tokens: &ColorTokens) -> Vec<ContrastPair> {
    let s = &tokens.surfaces;
    vec![
        ContrastPair {
            name: "ink1_on_window",
            foreground: s.ink1,
            background: s.window,
            min_ratio: AA_NORMAL,
        },
        ContrastPair {
            name: "ink1_on_panel",
            foreground: s.ink1,
            background: s.panel,
            min_ratio: AA_NORMAL,
        },
        ContrastPair {
            name: "ink2_on_window",
            foreground: s.ink2,
            background: s.window,
            min_ratio: AA_NORMAL,
        },
        ContrastPair {
            name: "ink3_on_panel",
            foreground: s.ink3,
            background: s.panel,
            min_ratio: AA_NORMAL,
        },
        ContrastPair {
            name: "accent_on_accent_soft",
            foreground: s.accent,
            background: s.accent_soft,
            min_ratio: AA_LARGE,
        },
        ContrastPair {
            name: "on_env_local",
            foreground: env_on(tokens, tokens.env.local),
            background: tokens.env.local,
            min_ratio: AA_NORMAL,
        },
        ContrastPair {
            name: "on_env_dev",
            foreground: env_on(tokens, tokens.env.dev),
            background: tokens.env.dev,
            min_ratio: AA_NORMAL,
        },
        ContrastPair {
            name: "on_env_staging",
            foreground: env_on(tokens, tokens.env.staging),
            background: tokens.env.staging,
            min_ratio: AA_NORMAL,
        },
        ContrastPair {
            name: "on_env_prod",
            foreground: env_on(tokens, tokens.env.production),
            background: tokens.env.production,
            min_ratio: AA_NORMAL,
        },
        ContrastPair {
            name: "syntax_kw_on_panel",
            foreground: tokens.syntax.keyword,
            background: s.panel,
            min_ratio: AA_NORMAL,
        },
        ContrastPair {
            name: "syntax_comment_on_panel",
            foreground: tokens.syntax.comment,
            background: s.panel,
            min_ratio: AA_NORMAL,
        },
    ]
}

pub fn evaluate_pair(pair: ContrastPair) -> ContrastResult {
    let ratio = contrast_ratio(pair.foreground, pair.background);
    ContrastResult {
        name: pair.name,
        ratio,
        required: pair.min_ratio,
        passed: ratio >= pair.min_ratio,
    }
}

pub fn contrast_ratio(fg: u32, bg: u32) -> f64 {
    let l1 = relative_luminance(fg);
    let l2 = relative_luminance(bg);
    let (lighter, darker) = if l1 >= l2 { (l1, l2) } else { (l2, l1) };
    (lighter + 0.05) / (darker + 0.05)
}

fn relative_luminance(hex: u32) -> f64 {
    let r = channel(hex, 16);
    let g = channel(hex, 8);
    let b = channel(hex, 0);
    let r = linearize(r);
    let g = linearize(g);
    let b = linearize(b);
    0.2126 * r + 0.7152 * g + 0.0722 * b
}

fn channel(hex: u32, shift: u32) -> f64 {
    ((hex >> shift) & 0xFF) as f64 / 255.0
}

fn linearize(c: f64) -> f64 {
    if c <= 0.03928 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn light_palette_meets_wcag_aa() {
        let failures: Vec<_> = wcag_pairs(&ColorTokens::LIGHT)
            .into_iter()
            .map(evaluate_pair)
            .filter(|r| !r.passed)
            .collect();
        assert!(failures.is_empty(), "light contrast failures: {failures:?}");
    }

    #[test]
    fn dark_palette_meets_wcag_aa() {
        let failures: Vec<_> = wcag_pairs(&ColorTokens::DARK)
            .into_iter()
            .map(evaluate_pair)
            .filter(|r| !r.passed)
            .collect();
        assert!(failures.is_empty(), "dark contrast failures: {failures:?}");
    }
}
