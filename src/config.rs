use serde::Deserialize;

#[derive(Debug, Deserialize)]
#[serde(default)]
pub struct Config {
    pub grid: GridConfig,
    pub subcell: SubcellConfig,
    pub keybinds: KeybindsConfig,
    pub scroll: ScrollConfig,
    pub style: StyleConfig,
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
    /// Key to enter scroll mode from grid mode.
    pub scroll_mode_key: char,
    /// Key to enter drag mode from grid mode.
    pub drag_mode_key: char,
    /// Modifier name that selects right-click ("shift" | "ctrl" | "alt").
    pub right_click_modifier: String,
    /// Modifier name that selects middle-click.
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

// ── Defaults ───────────────────────────────────────────────────────────────

impl Default for Config {
    fn default() -> Self {
        Self {
            grid: GridConfig::default(),
            subcell: SubcellConfig::default(),
            keybinds: KeybindsConfig::default(),
            scroll: ScrollConfig::default(),
            style: StyleConfig::default(),
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
            scroll_mode_key: 's',
            drag_mode_key: 'd',
            right_click_modifier: "shift".into(),
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
            overlay_bg: "#00000088".into(),
            cell_border: "#ffffff33".into(),
            label_color: "#ffffffff".into(),
            active_cell: "#ffff0055".into(),
            subcell_dot: "#00ff88cc".into(),
        }
    }
}
