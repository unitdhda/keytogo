use std::os::raw::c_void;
use std::sync::atomic::{AtomicPtr, Ordering};

use crate::{
    config::{GridConfig, KeybindsConfig, ScrollConfig},
    keymap::{keycode_to_char, SUBCELL_KEYS, SUBCELL_X_SPAN},
    mouse::{self, CGPoint},
    state::{AppMode, AppState, ClickButton},
};

// ── Static tap port — needed to re-enable on timeout ──────────────────────

static TAP_PORT: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());

// ── FFI types ──────────────────────────────────────────────────────────────

type CGEventTapProxy    = *mut c_void;
type CGEventRef         = *mut c_void;
type CFMachPortRef      = *mut c_void;
type CFRunLoopRef       = *mut c_void;
type CFRunLoopSourceRef = *mut c_void;
type CFStringRef        = *const c_void;
type CFAllocatorRef     = *const c_void;
type CFIndex            = isize;

type CGEventTapCallBack = unsafe extern "C" fn(
    proxy:      CGEventTapProxy,
    event_type: u32,
    event:      CGEventRef,
    user_info:  *mut c_void,
) -> CGEventRef;

// ── Constants ──────────────────────────────────────────────────────────────

const K_CG_SESSION_EVENT_TAP:            u32 = 1;
const K_CG_HEAD_INSERT_EVENT_TAP:        u32 = 0;
const K_CG_EVENT_TAP_OPTION_DEFAULT:     u32 = 0;

const K_CG_EVENT_KEY_DOWN:               u32 = 10;
const K_CG_EVENT_KEY_UP:                 u32 = 11;
const K_CG_EVENT_FLAGS_CHANGED:          u32 = 12;
const K_CG_EVENT_TAP_DISABLED_TIMEOUT:   u32 = 0xFFFFFFFE;
const K_CG_EVENT_TAP_DISABLED_USER:      u32 = 0xFFFFFFFF;

const K_CG_KEYBOARD_EVENT_KEYCODE: u32 = 9;

const FLAGS_SHIFT:   u64 = 0x0002_0000;
const FLAGS_CONTROL: u64 = 0x0004_0000;
const FLAGS_ALT:     u64 = 0x0008_0000;

const KEYCODE_SPACE:  u16 = 0x31;
const KEYCODE_ESCAPE: u16 = 0x35;
const KEYCODE_RETURN: u16 = 0x24;

// Ctrl+Alt held simultaneously
const ACTIVATION_MODS: u64 = FLAGS_CONTROL | FLAGS_ALT;

fn event_mask(types: &[u32]) -> u64 {
    types.iter().fold(0u64, |acc, &t| acc | (1u64 << t))
}

// ── Framework bindings ─────────────────────────────────────────────────────

#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    fn CFMachPortCreateRunLoopSource(
        allocator: CFAllocatorRef,
        port:      CFMachPortRef,
        order:     CFIndex,
    ) -> CFRunLoopSourceRef;
    fn CFRunLoopGetCurrent() -> CFRunLoopRef;
    fn CFRunLoopAddSource(rl: CFRunLoopRef, source: CFRunLoopSourceRef, mode: CFStringRef);
    fn CFRunLoopRun();
    fn CFRelease(cf: *const c_void);
    static kCFRunLoopCommonModes: CFStringRef;
}

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGEventTapCreate(
        tap:                u32,
        place:              u32,
        options:            u32,
        events_of_interest: u64,
        callback:           CGEventTapCallBack,
        user_info:          *mut c_void,
    ) -> CFMachPortRef;
    fn CGEventGetIntegerValueField(event: CGEventRef, field: u32) -> i64;
    fn CGEventGetFlags(event: CGEventRef) -> u64;
    fn CGEventTapEnable(tap: CFMachPortRef, enable: bool);
}

// ── Top-level callback ─────────────────────────────────────────────────────

unsafe extern "C" fn tap_callback(
    _proxy:     CGEventTapProxy,
    event_type: u32,
    event:      CGEventRef,
    user_info:  *mut c_void,
) -> CGEventRef {
    // macOS disables the tap if the callback takes too long; re-enable it.
    if event_type == K_CG_EVENT_TAP_DISABLED_TIMEOUT
        || event_type == K_CG_EVENT_TAP_DISABLED_USER
    {
        let port = TAP_PORT.load(Ordering::Relaxed);
        if !port.is_null() {
            CGEventTapEnable(port, true);
            log::warn!("tap disabled by macOS — re-enabled");
        }
        return event;
    }

    let state = &mut *(user_info as *mut AppState);

    // Swallow key-up and flags-changed in non-idle modes.
    if event_type == K_CG_EVENT_KEY_UP || event_type == K_CG_EVENT_FLAGS_CHANGED {
        return match state.mode {
            AppMode::Idle => event,
            _ => std::ptr::null_mut(),
        };
    }

    if event_type != K_CG_EVENT_KEY_DOWN {
        return event;
    }

    let kc    = CGEventGetIntegerValueField(event, K_CG_KEYBOARD_EVENT_KEYCODE) as u16;
    let flags = CGEventGetFlags(event);
    let mode  = state.mode.clone();

    match mode {
        AppMode::Idle => on_idle(state, kc, flags, event),
        AppMode::Grid { first } => on_grid(state, kc, flags, first),
        AppMode::Subcell { cell_col, cell_row, button } => {
            on_subcell(state, kc, flags, cell_col, cell_row, button)
        }
        AppMode::Scroll => on_scroll(state, kc),
        AppMode::Drag { start } => on_drag(state, kc, start),
    }
}

// ── Mode handlers ──────────────────────────────────────────────────────────

fn on_idle(state: &mut AppState, kc: u16, flags: u64, event: CGEventRef) -> CGEventRef {
    // Ctrl+Alt+Space → GridMode
    if kc == KEYCODE_SPACE && (flags & ACTIVATION_MODS) == ACTIVATION_MODS {
        state.mode = AppMode::Grid { first: None };
        log::info!("→ GridMode");
        return std::ptr::null_mut();
    }
    event
}

fn on_grid(state: &mut AppState, kc: u16, flags: u64, first: Option<char>) -> CGEventRef {
    let grid = GridConfig::default();
    let binds = KeybindsConfig::default();

    if kc == KEYCODE_ESCAPE {
        state.mode = AppMode::Idle;
        log::info!("→ Idle");
        return std::ptr::null_mut();
    }

    let Some(ch) = keycode_to_char(kc) else {
        return std::ptr::null_mut();
    };

    match first {
        None => {
            // Mode-switch keys take priority over grid labels at first position.
            // Note: if scroll_mode_key / drag_mode_key are in label_alphabet,
            // those cells become inaccessible — configure them to non-alpha chars
            // to avoid the conflict.
            if ch == binds.scroll_mode_key {
                state.mode = AppMode::Scroll;
                log::info!("→ ScrollMode");
            } else if ch == binds.drag_mode_key {
                state.mode = AppMode::Drag { start: None };
                log::info!("→ DragMode");
            } else if grid.label_alphabet.contains(ch) {
                state.mode = AppMode::Grid { first: Some(ch) };
                log::debug!("grid first='{ch}'");
            }
        }
        Some(f) => {
            if grid.label_alphabet.contains(ch) {
                match label_to_cell(f, ch, &grid) {
                    Some((col, row)) => {
                        let pos = cell_center(col, row, &grid);
                        mouse::move_cursor(pos.x, pos.y);
                        let button = modifier_button(flags);
                        log::info!(
                            "→ SubcellMode  cell=({col},{row})  cursor=({:.0},{:.0})",
                            pos.x, pos.y
                        );
                        state.mode = AppMode::Subcell { cell_col: col, cell_row: row, button };
                    }
                    None => {
                        log::debug!("'{f}{ch}' out of grid bounds — reset");
                        state.mode = AppMode::Grid { first: None };
                    }
                }
            }
        }
    }

    std::ptr::null_mut()
}

fn on_subcell(
    state:    &mut AppState,
    kc:       u16,
    flags:    u64,
    cell_col: usize,
    cell_row: usize,
    button:   ClickButton,
) -> CGEventRef {
    let grid = GridConfig::default();

    if kc == KEYCODE_ESCAPE {
        // Escape goes back to grid so the user can re-select without re-activating.
        state.mode = AppMode::Grid { first: None };
        log::info!("→ GridMode");
        return std::ptr::null_mut();
    }

    // Modifier at click time overrides the button chosen at grid selection.
    let btn = modifier_button_with_default(flags, button);

    let click_pos = if kc == KEYCODE_SPACE || kc == KEYCODE_RETURN {
        Some(cell_center(cell_col, cell_row, &grid))
    } else if let Some(ch) = keycode_to_char(kc) {
        subcell_pos(ch, cell_col, cell_row, &grid)
    } else {
        None
    };

    if let Some(pos) = click_pos {
        mouse::move_cursor(pos.x, pos.y);
        mouse::click(pos, btn, 1); // Phase 4 will add double/triple tap counting
        log::info!("click {btn:?} ×1 at ({:.0},{:.0}) → Idle", pos.x, pos.y);
        state.mode = AppMode::Idle;
    }

    std::ptr::null_mut()
}

fn on_scroll(state: &mut AppState, kc: u16) -> CGEventRef {
    let cfg = ScrollConfig::default();
    let step      = 3_i32;
    let half_page = cfg.half_page_lines as i32;

    if kc == KEYCODE_ESCAPE {
        state.mode = AppMode::Idle;
        log::info!("→ Idle");
        return std::ptr::null_mut();
    }

    if let Some(ch) = keycode_to_char(kc) {
        match ch {
            'j' => mouse::scroll(-step, 0),
            'k' => mouse::scroll(step, 0),
            'h' => mouse::scroll(0, step),
            'l' => mouse::scroll(0, -step),
            'd' => mouse::scroll(-half_page, 0),
            'u' => mouse::scroll(half_page, 0),
            _ => {}
        }
        log::debug!("scroll '{ch}'");
    }

    std::ptr::null_mut()
}

fn on_drag(state: &mut AppState, kc: u16, start: Option<(f64, f64)>) -> CGEventRef {
    // Full drag implementation is Phase 5.
    // For now: Escape cancels, any cell selection is stubbed.
    if kc == KEYCODE_ESCAPE {
        if let Some((x, y)) = start {
            // Release the held button if we bailed mid-drag.
            mouse::click(CGPoint::new(x, y), ClickButton::Left, 1);
        }
        state.mode = AppMode::Idle;
        log::info!("→ Idle (drag cancelled)");
    }
    std::ptr::null_mut()
}

// ── Grid geometry ──────────────────────────────────────────────────────────

/// Map (first_char, second_char) → (col, row) using the label alphabet.
fn label_to_cell(first: char, second: char, cfg: &GridConfig) -> Option<(usize, usize)> {
    let alpha: Vec<char> = cfg.label_alphabet.chars().collect();
    let col = alpha.iter().position(|&c| c == first)?;
    let row = alpha.iter().position(|&c| c == second)?;
    if col < cfg.cols && row < cfg.rows {
        Some((col, row))
    } else {
        None
    }
}

/// Pixel center of a grid cell.
fn cell_center(col: usize, row: usize, cfg: &GridConfig) -> CGPoint {
    let (sw, sh) = mouse::screen_size();
    let cw = sw / cfg.cols as f64;
    let ch = sh / cfg.rows as f64;
    CGPoint::new(col as f64 * cw + cw * 0.5, row as f64 * ch + ch * 0.5)
}

/// Pixel position of a subcell key within a grid cell.
/// Uses the stagger-accurate SUBCELL_KEYS offsets, mapped to cell interior
/// with 10 % horizontal and 15 % vertical padding from the cell edges.
fn subcell_pos(key: char, cell_col: usize, cell_row: usize, cfg: &GridConfig) -> Option<CGPoint> {
    let (sw, sh) = mouse::screen_size();
    let cw = sw / cfg.cols as f64;
    let ch = sh / cfg.rows as f64;
    let cx = cell_col as f64 * cw;
    let cy = cell_row as f64 * ch;

    let &(_, rel_x, rel_y) = SUBCELL_KEYS.iter().find(|&&(k, _, _)| k == key)?;

    let norm_x = (rel_x / SUBCELL_X_SPAN) as f64; // 0.0 – 1.0
    let norm_y = (rel_y / 2.0) as f64;            // 0.0 – 1.0  (rows 0/1/2 → thirds)

    Some(CGPoint::new(
        cx + cw * (0.10 + norm_x * 0.80),
        cy + ch * (0.15 + norm_y * 0.70),
    ))
}

// ── Modifier helpers ───────────────────────────────────────────────────────

fn modifier_button(flags: u64) -> ClickButton {
    modifier_button_with_default(flags, ClickButton::Left)
}

fn modifier_button_with_default(flags: u64, default: ClickButton) -> ClickButton {
    // These match the defaults in KeybindsConfig:
    //   right_click_modifier = "shift"
    //   middle_click_modifier = "ctrl"
    if flags & FLAGS_SHIFT != 0 {
        ClickButton::Right
    } else if flags & FLAGS_CONTROL != 0 {
        ClickButton::Middle
    } else {
        default
    }
}

// ── Entry point ────────────────────────────────────────────────────────────

pub fn run() {
    let state = Box::into_raw(Box::new(AppState::new())) as *mut c_void;

    let mask = event_mask(&[
        K_CG_EVENT_KEY_DOWN,
        K_CG_EVENT_KEY_UP,
        K_CG_EVENT_FLAGS_CHANGED,
    ]);

    let tap_port = unsafe {
        CGEventTapCreate(
            K_CG_SESSION_EVENT_TAP,
            K_CG_HEAD_INSERT_EVENT_TAP,
            K_CG_EVENT_TAP_OPTION_DEFAULT,
            mask,
            tap_callback,
            state,
        )
    };

    if tap_port.is_null() {
        eprintln!("CGEventTapCreate failed — ensure Accessibility permission is granted.");
        std::process::exit(1);
    }

    TAP_PORT.store(tap_port, Ordering::Relaxed);

    let source = unsafe { CFMachPortCreateRunLoopSource(std::ptr::null(), tap_port, 0) };
    if source.is_null() {
        eprintln!("CFMachPortCreateRunLoopSource failed.");
        std::process::exit(1);
    }

    unsafe {
        let rl = CFRunLoopGetCurrent();
        CFRunLoopAddSource(rl, source, kCFRunLoopCommonModes);
        CGEventTapEnable(tap_port, true);
        log::info!("event tap active  (Ctrl+Alt+Space to activate grid)");
        CFRunLoopRun();
        CFRelease(source as *const c_void);
        CFRelease(tap_port as *const c_void);
    }
}
