use serde::Deserialize;
use std::sync::OnceLock;

use crate::keymap::{parse_layout_string, ParsedLayout};

static CONFIG: OnceLock<Config> = OnceLock::new();
static PARSED: OnceLock<ParsedLayouts> = OnceLock::new();

/// Returns the global config, loading from disk on first call.
pub fn get() -> &'static Config {
    CONFIG.get_or_init(Config::load_from_disk)
}

/// Returns the three parsed key layouts, derived from config on first call.
/// Panics on startup if the config contains an invalid layout string.
pub fn parsed_layouts() -> &'static ParsedLayouts {
    PARSED.get_or_init(|| {
        let cfg = get();
        ParsedLayouts {
            macro_l:   parse_layout_string(&cfg.layout.macro_keys)
                           .unwrap_or_else(|e| panic!("invalid [layout].macro_keys: {e}")),
            sub_l:     parse_layout_string(&cfg.layout.sub_keys)
                           .unwrap_or_else(|e| panic!("invalid [layout].sub_keys: {e}")),
            subcell_l: parse_layout_string(&cfg.layout.subcell_keys)
                           .unwrap_or_else(|e| panic!("invalid [layout].subcell_keys: {e}")),
        }
    })
}

/// Three parsed key-layout grids, cached after first access.
pub struct ParsedLayouts {
    pub macro_l:   ParsedLayout,
    pub sub_l:     ParsedLayout,
    pub subcell_l: ParsedLayout,
}

impl Config {
    fn load_from_disk() -> Self {
        let home = std::env::var("HOME").unwrap_or_default();
        let path = format!("{}/.config/keytogo/config.toml", home);
        match std::fs::read_to_string(&path) {
            Ok(s) => toml::from_str(&s).unwrap_or_else(|e| {
                log::warn!("config parse error in {path}: {e}; using defaults");
                Config::default()
            }),
            Err(_) => Config::default(),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct Config {
    pub layout:  LayoutConfig,
    pub subcell: SubcellConfig,
    pub keybinds: KeybindsConfig,
    pub scroll:  ScrollConfig,
    pub style:   StyleConfig,
    pub hud:     HudConfig,
}

/// Three multiline key-alphabet strings. Dimensions are inferred from the strings.
/// Spaces within a line are ignored (use them for visual alignment).
#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct LayoutConfig {
    /// Stage 1 — selects which screen region. Each line = one keyboard row.
    pub macro_keys: String,
    /// Stage 2 — selects a sub-cell within the chosen region.
    pub sub_keys: String,
    /// Stage 3 — fine-positions the cursor inside the selected sub-cell (SubcellMode).
    pub subcell_keys: String,
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct SubcellConfig {
    /// Max milliseconds between taps to count as double/triple click.
    pub tap_window_ms: u64,
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct KeybindsConfig {
    /// Modifier that selects right-click: "shift" | "ctrl" | "alt".
    pub right_click_modifier: String,
    /// Modifier that selects middle-click: "shift" | "ctrl" | "alt".
    pub middle_click_modifier: String,
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct ScrollConfig {
    pub line_px: u32,
    pub half_page_lines: u32,
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct StyleConfig {
    pub overlay_bg: String,
    pub cell_border: String,
    pub label_color: String,
    pub active_cell: String,
    pub subcell_dot: String,
}

/// HUD overlay position and margin configuration.
#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct HudConfig {
    /// Where to anchor the scroll HUD pill.
    /// Values: "bottom-center" | "bottom-left" | "bottom-right"
    ///         | "top-center"  | "top-left"    | "top-right"
    pub position: String,
    /// Horizontal offset from the anchor edge (pixels). Ignored for *-center.
    pub margin_x: f64,
    /// Vertical offset from the screen edge (pixels).
    pub margin_y: f64,
}

// ── Defaults ───────────────────────────────────────────────────────────────

impl Default for Config {
    fn default() -> Self {
        Self {
            layout:  LayoutConfig::default(),
            subcell: SubcellConfig::default(),
            keybinds: KeybindsConfig::default(),
            scroll:  ScrollConfig::default(),
            style:   StyleConfig::default(),
            hud:     HudConfig::default(),
        }
    }
}

impl Default for LayoutConfig {
    fn default() -> Self {
        Self {
            macro_keys:   "qwer\nasdf\nzxcv\nyuio\nhjkl\nnm,.".into(),
            sub_keys:     "ertyuio\ndfghjkl\ncvbnm,.".into(),
            subcell_keys: "ertyui\ndfghjk\nxcvbnm".into(),
        }
    }
}

impl Default for SubcellConfig {
    fn default() -> Self {
        Self { tap_window_ms: 250 }
    }
}

impl Default for KeybindsConfig {
    fn default() -> Self {
        Self {
            right_click_modifier:  "shift".into(),
            middle_click_modifier: "ctrl".into(),
        }
    }
}

impl Default for ScrollConfig {
    fn default() -> Self {
        Self { line_px: 60, half_page_lines: 10 }
    }
}

impl Default for StyleConfig {
    fn default() -> Self {
        Self {
            overlay_bg:  "#00000088".into(),
            cell_border: "#ffffff33".into(),
            label_color: "#ffffffff".into(),
            active_cell: "#ffff0055".into(),
            subcell_dot: "#00ff88cc".into(),
        }
    }
}

impl Default for HudConfig {
    fn default() -> Self {
        Self {
            position: "bottom-center".into(),
            margin_x: 0.0,
            margin_y: 64.0,
        }
    }
}
