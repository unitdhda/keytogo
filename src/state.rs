#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClickButton {
    Left,
    Right,
    Middle,
}

/// Pixel bounding box of the selected cell, in screen coordinates (y from top).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CellBounds {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl CellBounds {
    pub fn new(x: f64, y: f64, w: f64, h: f64) -> Self {
        Self { x, y, w, h }
    }

    pub fn center_x(&self) -> f64 { self.x + self.w / 2.0 }
    pub fn center_y(&self) -> f64 { self.y + self.h / 2.0 }
}

#[derive(Debug, Clone)]
pub enum AppMode {
    Idle,
    /// Macro grid: waiting for macro key then sub key.
    GridA { macro_first: Option<char> },
    /// A sub-cell is chosen; waiting for subcell key or Space/Return.
    Subcell { bounds: CellBounds, button: ClickButton, macro_key: char },
    Scroll,
}

impl Default for AppMode {
    fn default() -> Self {
        AppMode::Idle
    }
}

/// Accumulated tap waiting for the 250 ms multi-click window.
#[derive(Clone, Copy, Debug)]
pub struct PendingTap {
    pub x:      f64,
    pub y:      f64,
    pub button: ClickButton,
    pub count:  u8,
    pub key:    char,
}

pub struct AppState {
    pub mode: AppMode,
    /// Cursor position recorded at activation time, used as drag source.
    pub drag_origin: Option<(f64, f64)>,
    /// Active mouse-button hold started by Space in scroll mode (x, y, button).
    /// Set on key-down, cleared and released on key-up.
    pub held_click: Option<(f64, f64, ClickButton)>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            mode: AppMode::default(),
            drag_origin: None,
            held_click: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_mode_is_idle() {
        assert!(matches!(AppMode::default(), AppMode::Idle));
    }

    #[test]
    fn new_state_is_idle() {
        let s = AppState::new();
        assert!(matches!(s.mode, AppMode::Idle));
    }

    #[test]
    fn grid_a_mode_carries_macro_first() {
        let m = AppMode::GridA { macro_first: Some('q') };
        match m {
            AppMode::GridA { macro_first: Some(c) } => assert_eq!(c, 'q'),
            _ => panic!("unexpected variant"),
        }
    }

    #[test]
    fn click_button_equality() {
        assert_eq!(ClickButton::Left, ClickButton::Left);
        assert_ne!(ClickButton::Left, ClickButton::Right);
        assert_ne!(ClickButton::Right, ClickButton::Middle);
    }

    #[test]
    fn cell_bounds_center() {
        let b = CellBounds::new(100.0, 200.0, 50.0, 40.0);
        assert!((b.center_x() - 125.0).abs() < 1e-9);
        assert!((b.center_y() - 220.0).abs() < 1e-9);
    }

    #[test]
    fn held_click_defaults_to_none() {
        let s = AppState::new();
        assert!(s.held_click.is_none());
    }

    #[test]
    fn held_click_can_be_set_and_cleared() {
        let mut s = AppState::new();
        s.held_click = Some((100.0, 200.0, ClickButton::Left));
        assert!(s.held_click.is_some());
        let (x, y, btn) = s.held_click.take().unwrap();
        assert!((x - 100.0).abs() < 1e-9);
        assert!((y - 200.0).abs() < 1e-9);
        assert_eq!(btn, ClickButton::Left);
        assert!(s.held_click.is_none());
    }

    #[test]
    fn subcell_mode_carries_button() {
        let b = CellBounds::new(0.0, 0.0, 100.0, 100.0);
        let m = AppMode::Subcell { bounds: b, button: ClickButton::Right, macro_key: 'a' };
        match m {
            AppMode::Subcell { button: ClickButton::Right, .. } => {}
            _ => panic!("unexpected variant"),
        }
    }
}
