use serde::Deserialize;
use std::sync::OnceLock;

static CONFIG: OnceLock<Config> = OnceLock::new();

/// Returns the global config, loading from disk on first call.
pub fn get() -> &'static Config {
    CONFIG.get_or_init(Config::load_from_disk)
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
    pub grid: GridConfig,
    pub subcell: SubcellConfig,
    pub keybinds: KeybindsConfig,
    pub scroll: ScrollConfig,
    pub style: StyleConfig,
    pub hud: HudConfig,
}

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct GridConfig {
    /// Number of columns in the macro grid.
    pub cols: usize,
    /// Number of rows in the macro grid.
    pub rows: usize,
    /// Characters used to label grid cells (home-row biased for ergonomics).
    pub label_alphabet: String,
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
    /// Modifier name that selects right-click ("shift" | "ctrl" | "alt").
    pub right_click_modifier: String,
    /// Modifier name that selects middle-click.
    pub middle_click_modifier: String,
    /// Which layout to show on activation: "grid_a" (macrogrid) or "grid" (standard).
    pub default_layout: String,
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
    /// Horizontal offset from the anchor edge (pixels).  Ignored for *-center.
    pub margin_x: f64,
    /// Vertical offset from the screen edge (pixels).
    pub margin_y: f64,
}

// ── Defaults ───────────────────────────────────────────────────────────────

impl Default for Config {
    fn default() -> Self {
        Self {
            grid: GridConfig::default(),
            subcell: SubcellConfig::default(),
            keybinds: KeybindsConfig::default(),
            scroll: ScrollConfig::default(),
            style: StyleConfig::default(),
            hud: HudConfig::default(),
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

impl Default for GridConfig {
    fn default() -> Self {
        Self {
            cols: 10,
            rows: 9,
            // Keyboard row-order: qq=top-left, pq=top-right, qo=bottom-left, po=bottom-right.
            // Cols use indices 0-9 (q-p = top keyboard row, left→right).
            // Rows use indices 0-8 (q-o = first 9 of top row, mapped top→bottom).
            label_alphabet: "qwertyuiopasdfghjklzxcvbnm".into(),
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
            right_click_modifier: "shift".into(),
            middle_click_modifier: "ctrl".into(),
            default_layout: "grid_a".into(),
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
            overlay_bg: "#00000088".into(),
            cell_border: "#ffffff33".into(),
            label_color: "#ffffffff".into(),
            active_cell: "#ffff0055".into(),
            subcell_dot: "#00ff88cc".into(),
        }
    }
}
