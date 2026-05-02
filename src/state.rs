#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClickButton {
    Left,
    Right,
    Middle,
}

#[derive(Debug, Clone)]
pub enum AppMode {
    Idle,
    /// Waiting for 2-char grid code. `first` holds the first char once typed.
    Grid { first: Option<char> },
    /// A macro cell is chosen; waiting for subcell key or space.
    Subcell {
        cell_col: usize,
        cell_row: usize,
        button: ClickButton,
    },
    Scroll,
    Drag { start: Option<(f64, f64)> },
}

impl Default for AppMode {
    fn default() -> Self {
        AppMode::Idle
    }
}

pub struct AppState {
    pub mode: AppMode,
}

impl AppState {
    pub fn new() -> Self {
        Self { mode: AppMode::default() }
    }
}
