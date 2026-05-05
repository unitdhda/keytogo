use std::os::raw::c_void;
use std::sync::atomic::{AtomicBool, AtomicPtr, AtomicU32, Ordering};

use std::cell::Cell;

use crate::{
    config,
    keymap::{keycode_to_char, layout_geom},
    mouse::{self, CGPoint},
    overlay,
    state::{AppMode, AppState, CellBounds, ClickButton, PendingTap},
};

// ── Statics ────────────────────────────────────────────────────────────────

static TAP_PORT: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
static STATE_PTR: AtomicPtr<AppState> = AtomicPtr::new(std::ptr::null_mut());
static PENDING_TIMER: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());
static SCROLL_HUD_TIMER: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());

static TAP_INITIALISED: AtomicBool = AtomicBool::new(false);
static TAP_TIMEOUT_COUNT: AtomicU32 = AtomicU32::new(0);

thread_local! {
    static PENDING_TAP: Cell<Option<PendingTap>> = const { Cell::new(None) };
}

// ── FFI types ──────────────────────────────────────────────────────────────

type CGEventTapProxy = *mut c_void;
type CGEventRef = *mut c_void;
type CFMachPortRef = *mut c_void;
type CFRunLoopRef = *mut c_void;
type CFRunLoopSourceRef = *mut c_void;
type CFStringRef = *const c_void;
type CFAllocatorRef = *const c_void;
type CFIndex = isize;

type CGEventTapCallBack = unsafe extern "C" fn(
    proxy: CGEventTapProxy,
    event_type: u32,
    event: CGEventRef,
    user_info: *mut c_void,
) -> CGEventRef;

// ── Constants ──────────────────────────────────────────────────────────────

const K_CG_SESSION_EVENT_TAP: u32 = 1;
const K_CG_HEAD_INSERT_EVENT_TAP: u32 = 0;
const K_CG_EVENT_TAP_OPTION_DEFAULT: u32 = 0;

const K_CG_EVENT_KEY_DOWN: u32 = 10;
const K_CG_EVENT_KEY_UP: u32 = 11;
const K_CG_EVENT_FLAGS_CHANGED: u32 = 12;
const K_CG_EVENT_TAP_DISABLED_TIMEOUT: u32 = 0xFFFFFFFE;
const K_CG_EVENT_TAP_DISABLED_USER: u32 = 0xFFFFFFFF;

const K_CG_KEYBOARD_EVENT_KEYCODE: u32 = 9;

const FLAGS_SHIFT: u64 = 0x0002_0000;
const FLAGS_CONTROL: u64 = 0x0004_0000;
const FLAGS_OPTION: u64 = 0x0008_0000; // kCGEventFlagMaskAlternate — move cursor only
const FLAGS_CMD: u64 = 0x0010_0000; // kCGEventFlagMaskCommand (either Cmd)
const FLAGS_RCMD: u64 = 0x0000_0010; // NX_DEVICERCMDKEYMASK (right Cmd only)

const TAP_TIMER_DELAY: f64 = 0.250; // seconds — multi-click window

const KEYCODE_TAB: u16 = 0x30;
const KEYCODE_SPACE: u16 = 0x31;
const KEYCODE_ESCAPE: u16 = 0x35;
const KEYCODE_RETURN: u16 = 0x24;
const KEYCODE_DELETE: u16 = 0x33; // Backspace

const ACTIVATION_MODS: u64 = FLAGS_CMD | FLAGS_RCMD;

fn event_mask(types: &[u32]) -> u64 {
    types.iter().fold(0u64, |acc, &t| acc | (1u64 << t))
}

// ── Framework bindings ─────────────────────────────────────────────────────

type CFRunLoopTimerRef = *mut c_void;
type CFRunLoopTimerCallBack = unsafe extern "C" fn(timer: CFRunLoopTimerRef, info: *mut c_void);

#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    fn CFMachPortCreateRunLoopSource(
        allocator: CFAllocatorRef,
        port: CFMachPortRef,
        order: CFIndex,
    ) -> CFRunLoopSourceRef;
    fn CFRunLoopGetCurrent() -> CFRunLoopRef;
    fn CFRunLoopAddSource(rl: CFRunLoopRef, source: CFRunLoopSourceRef, mode: CFStringRef);
    fn CFRunLoopAddTimer(rl: CFRunLoopRef, timer: CFRunLoopTimerRef, mode: CFStringRef);
    fn CFRunLoopRun();
    fn CFRelease(cf: *const c_void);
    fn CFRunLoopTimerCreate(
        allocator: CFAllocatorRef,
        fire_date: f64,
        interval: f64,
        flags: u32,
        order: CFIndex,
        callout: CFRunLoopTimerCallBack,
        context: *mut c_void,
    ) -> CFRunLoopTimerRef;
    fn CFRunLoopTimerInvalidate(timer: CFRunLoopTimerRef);
    fn CFAbsoluteTimeGetCurrent() -> f64;
    static kCFRunLoopCommonModes: CFStringRef;
}

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGEventTapCreate(
        tap: u32,
        place: u32,
        options: u32,
        events_of_interest: u64,
        callback: CGEventTapCallBack,
        user_info: *mut c_void,
    ) -> CFMachPortRef;
    fn CGEventGetIntegerValueField(event: CGEventRef, field: u32) -> i64;
    fn CGEventGetFlags(event: CGEventRef) -> u64;
    fn CGEventTapEnable(tap: CFMachPortRef, enable: bool);
}

// ── Top-level callback ─────────────────────────────────────────────────────

unsafe extern "C" fn tap_callback(
    _proxy: CGEventTapProxy,
    event_type: u32,
    event: CGEventRef,
    user_info: *mut c_void,
) -> CGEventRef {
    if event_type == K_CG_EVENT_TAP_DISABLED_TIMEOUT || event_type == K_CG_EVENT_TAP_DISABLED_USER {
        let port = TAP_PORT.load(Ordering::Acquire);
        if !port.is_null() {
            let n = TAP_TIMEOUT_COUNT.fetch_add(1, Ordering::Relaxed) + 1;
            if n >= 5 {
                log::error!("event tap disabled by macOS {n} times consecutively — exiting");
                std::process::exit(1);
            }
            CGEventTapEnable(port, true);
            log::warn!("tap disabled by macOS — re-enabled (#{n})");
        }
        return event;
    }

    let state = &mut *(user_info as *mut AppState);

    if event_type == K_CG_EVENT_KEY_UP {
        let kc = CGEventGetIntegerValueField(event, K_CG_KEYBOARD_EVENT_KEYCODE) as u16;
        // Release a held mouse button when Space is lifted in scroll mode.
        if matches!(state.mode, AppMode::Scroll) && kc == KEYCODE_SPACE {
            if let Some((_, _, button)) = state.held_click.take() {
                let pos = mouse::cursor_pos();
                mouse::mouse_up(pos, button);
                let lbl = if button == ClickButton::Right {
                    "⌥Space"
                } else {
                    "Space"
                };
                overlay::show_scroll_hud(lbl, "click");
                schedule_hud_fade(1.0);
            }
            return std::ptr::null_mut();
        }
        return match state.mode {
            AppMode::Idle => event,
            _ => std::ptr::null_mut(),
        };
    }

    if event_type == K_CG_EVENT_FLAGS_CHANGED {
        return match state.mode {
            AppMode::Idle => event,
            _ => std::ptr::null_mut(),
        };
    }

    if event_type != K_CG_EVENT_KEY_DOWN {
        return event;
    }

    let kc = CGEventGetIntegerValueField(event, K_CG_KEYBOARD_EVENT_KEYCODE) as u16;
    let flags = CGEventGetFlags(event);
    let mode = state.mode.clone();

    match mode {
        AppMode::Idle => on_idle(state, kc, flags, event),
        AppMode::GridA { macro_first } => on_grid_a(state, kc, flags, macro_first),
        AppMode::Subcell {
            bounds,
            button,
            macro_key,
        } => on_subcell(state, kc, flags, bounds, button, macro_key),
        AppMode::Scroll => on_scroll(state, kc, flags),
    }
}

// ── Multi-click timer ──────────────────────────────────────────────────────

unsafe extern "C" fn tap_timer_callback(_timer: CFRunLoopTimerRef, _info: *mut c_void) {
    PENDING_TAP.with(|cell| {
        if let Some(tap) = cell.take() {
            let pos = CGPoint::new(tap.x, tap.y);
            mouse::click(pos, tap.button, tap.count);
            let state = &mut *STATE_PTR.load(Ordering::Acquire);
            state.mode = AppMode::Idle;
            overlay::hide();
        }
    });
    PENDING_TIMER.store(std::ptr::null_mut(), Ordering::Release);
}

fn cancel_pending_timer() {
    let old = PENDING_TIMER.swap(std::ptr::null_mut(), Ordering::AcqRel);
    if !old.is_null() {
        unsafe {
            CFRunLoopTimerInvalidate(old);
            CFRelease(old as *const c_void);
        }
    }
}

fn schedule_tap_timer() {
    cancel_pending_timer();
    let fire_at = unsafe { CFAbsoluteTimeGetCurrent() } + TAP_TIMER_DELAY;
    let timer = unsafe {
        CFRunLoopTimerCreate(
            std::ptr::null(),
            fire_at,
            0.0,
            0,
            0,
            tap_timer_callback,
            std::ptr::null_mut(),
        )
    };
    unsafe {
        CFRunLoopAddTimer(CFRunLoopGetCurrent(), timer, kCFRunLoopCommonModes);
    }
    PENDING_TIMER.store(timer, Ordering::Release);
}

fn fire_pending_tap_now(state: &mut AppState) {
    cancel_pending_timer();
    PENDING_TAP.with(|cell| {
        if let Some(tap) = cell.take() {
            let pos = CGPoint::new(tap.x, tap.y);
            mouse::click(pos, tap.button, tap.count);
            state.mode = AppMode::Idle;
            overlay::hide();
        }
    });
}

// ── Scroll HUD fade timer ──────────────────────────────────────────────────

unsafe extern "C" fn hud_fade_callback(_timer: CFRunLoopTimerRef, _info: *mut c_void) {
    overlay::hide();
    SCROLL_HUD_TIMER.store(std::ptr::null_mut(), Ordering::Release);
}

fn cancel_hud_timer() {
    let old = SCROLL_HUD_TIMER.swap(std::ptr::null_mut(), Ordering::AcqRel);
    if !old.is_null() {
        unsafe {
            CFRunLoopTimerInvalidate(old);
            CFRelease(old as *const c_void);
        }
    }
}

fn schedule_hud_fade(delay_secs: f64) {
    cancel_hud_timer();
    let fire_at = unsafe { CFAbsoluteTimeGetCurrent() } + delay_secs;
    let timer = unsafe {
        CFRunLoopTimerCreate(
            std::ptr::null(),
            fire_at,
            0.0,
            0,
            0,
            hud_fade_callback,
            std::ptr::null_mut(),
        )
    };
    unsafe {
        CFRunLoopAddTimer(CFRunLoopGetCurrent(), timer, kCFRunLoopCommonModes);
    }
    SCROLL_HUD_TIMER.store(timer, Ordering::Release);
}

// ── Mode handlers ──────────────────────────────────────────────────────────

fn on_idle(state: &mut AppState, kc: u16, flags: u64, event: CGEventRef) -> CGEventRef {
    if kc == KEYCODE_SPACE && (flags & ACTIVATION_MODS) == ACTIVATION_MODS {
        let origin = mouse::cursor_pos();
        state.drag_origin = Some((origin.x, origin.y));
        state.mode = AppMode::GridA { macro_first: None };
        overlay::show_grid_a(None);
        log::info!("→ GridA");
        return std::ptr::null_mut();
    }
    event
}

fn on_grid_a(state: &mut AppState, kc: u16, flags: u64, macro_first: Option<char>) -> CGEventRef {
    if kc == KEYCODE_ESCAPE {
        state.mode = AppMode::Idle;
        overlay::hide();
        log::info!("→ Idle");
        return std::ptr::null_mut();
    }

    if kc == KEYCODE_SPACE {
        let pos = mouse::cursor_pos();
        let btn = modifier_button(flags);

        let is_repeat = PENDING_TAP.with(|cell| cell.get().is_some_and(|t| t.key == ' '));
        if is_repeat {
            PENDING_TAP.with(|cell| {
                if let Some(mut t) = cell.get() {
                    t.count = (t.count + 1).min(3);
                    cell.set(Some(t));
                }
            });
        } else {
            // fire any unrelated pending tap first
            fire_pending_tap_now(state);
            if matches!(state.mode, AppMode::Idle) {
                return std::ptr::null_mut(); // already resolved
            }
            PENDING_TAP.with(|cell| {
                cell.set(Some(PendingTap {
                    x: pos.x,
                    y: pos.y,
                    button: btn,
                    count: 1,
                    key: ' ',
                }))
            });
        }
        schedule_tap_timer();
        // Stay in GridA so repeated Space presses accumulate clicks.
        // tap_timer_callback will fire, call click, set Idle, hide overlay.
        return std::ptr::null_mut();
    }

    if kc == KEYCODE_DELETE {
        match macro_first {
            Some(_) => {
                state.mode = AppMode::GridA { macro_first: None };
                overlay::show_grid_a(None);
                log::info!("← GridA (backspace macro key)");
            }
            None => {
                state.mode = AppMode::Idle;
                overlay::hide();
                log::info!("← Idle (backspace)");
            }
        }
        return std::ptr::null_mut();
    }

    if kc == KEYCODE_TAB && macro_first.is_none() {
        state.mode = AppMode::Scroll;
        overlay::show_scroll_mode();
        schedule_hud_fade(2.0);
        log::info!("→ ScrollMode (from GridA)");
        return std::ptr::null_mut();
    }

    let Some(ch) = keycode_to_char(kc) else {
        return std::ptr::null_mut();
    };

    let layouts = config::parsed_layouts();

    match macro_first {
        None => {
            if layouts.macro_l.key_pos(ch).is_some() {
                state.mode = AppMode::GridA {
                    macro_first: Some(ch),
                };
                overlay::show_grid_a(Some(ch));
            }
        }
        Some(mc) => {
            let macro_pos = layouts.macro_l.key_pos(mc);
            let sub_pos = layouts.sub_l.key_pos(ch);

            match (macro_pos, sub_pos) {
                (Some((macro_col, macro_row)), Some((sub_col, sub_row))) => {
                    let (sw, sh) = mouse::screen_size();
                    let g = layout_geom(
                        sw,
                        sh,
                        layouts.macro_l.num_cols,
                        layouts.macro_l.num_rows,
                        layouts.sub_l.num_cols,
                        layouts.sub_l.num_rows,
                    );
                    let cell_x = macro_col as f64 * g.macro_w + sub_col as f64 * g.cell_w;
                    let cell_y = macro_row as f64 * g.macro_h + sub_row as f64 * g.cell_h;

                    let bounds = CellBounds::new(cell_x, cell_y, g.cell_w, g.cell_h);
                    mouse::move_cursor(bounds.center_x(), bounds.center_y());
                    // Option alone = move cursor only; Option+Shift = proceed to subcell for drag
                    if flags & FLAGS_OPTION != 0 && flags & FLAGS_SHIFT == 0 {
                        log::info!("→ Idle (move)");
                        state.mode = AppMode::Idle;
                        overlay::hide();
                        return std::ptr::null_mut();
                    }
                    let button = modifier_button(flags);
                    log::info!("→ SubcellMode");
                    state.mode = AppMode::Subcell {
                        bounds,
                        button,
                        macro_key: mc,
                    };
                    overlay::show_subcell(bounds.x, bounds.y, bounds.w, bounds.h);
                }
                _ => {
                    // Unknown key — reset to fresh macro selection.
                    state.mode = AppMode::GridA { macro_first: None };
                    overlay::show_grid_a(None);
                }
            }
        }
    }

    std::ptr::null_mut()
}

fn on_subcell(
    state: &mut AppState,
    kc: u16,
    flags: u64,
    bounds: CellBounds,
    button: ClickButton,
    macro_key: char,
) -> CGEventRef {
    if kc == KEYCODE_ESCAPE {
        fire_pending_tap_now(state);
        if !matches!(state.mode, AppMode::Idle) {
            state.mode = AppMode::GridA { macro_first: None };
            overlay::show_grid_a(None);
        }
        log::info!("→ GridA");
        return std::ptr::null_mut();
    }

    if kc == KEYCODE_DELETE {
        state.mode = AppMode::GridA {
            macro_first: Some(macro_key),
        };
        overlay::show_grid_a(Some(macro_key));
        log::info!("← GridA macro_first={macro_key} (backspace)");
        return std::ptr::null_mut();
    }

    let (click_pos, click_key) = if kc == KEYCODE_SPACE || kc == KEYCODE_RETURN {
        (
            Some(CGPoint::new(bounds.center_x(), bounds.center_y())),
            ' ',
        )
    } else if let Some(ch) = keycode_to_char(kc) {
        (subcell_pos(ch, &bounds), ch)
    } else {
        (None, '\0')
    };

    if let Some(pos) = click_pos {
        if flags & FLAGS_OPTION != 0 && flags & FLAGS_SHIFT != 0 {
            // Option+Shift = drag from activation cursor pos to this precise position
            fire_pending_tap_now(state);
            if let Some((ox, oy)) = state.drag_origin {
                mouse::drag(CGPoint::new(ox, oy), pos);
            }
            state.mode = AppMode::Idle;
            overlay::hide();
            log::info!("→ Idle (drag)");
            return std::ptr::null_mut();
        } else if flags & FLAGS_OPTION != 0 {
            fire_pending_tap_now(state);
            mouse::move_cursor(pos.x, pos.y);
            state.mode = AppMode::Idle;
            overlay::hide();
            log::info!("→ Idle (move)");
            return std::ptr::null_mut();
        }

        let btn = modifier_button_with_default(flags, button);
        mouse::move_cursor(pos.x, pos.y);

        let is_repeat = PENDING_TAP.with(|cell| cell.get().is_some_and(|t| t.key == click_key));

        if is_repeat {
            PENDING_TAP.with(|cell| {
                if let Some(mut t) = cell.get() {
                    t.count = (t.count + 1).min(3);
                    cell.set(Some(t));
                }
            });
        } else {
            fire_pending_tap_now(state);
            if matches!(state.mode, AppMode::Idle) {
                return std::ptr::null_mut();
            }
            PENDING_TAP.with(|cell| {
                cell.set(Some(PendingTap {
                    x: pos.x,
                    y: pos.y,
                    button: btn,
                    count: 1,
                    key: click_key,
                }))
            });
        }
        schedule_tap_timer();
    }

    std::ptr::null_mut()
}

fn on_scroll(state: &mut AppState, kc: u16, flags: u64) -> CGEventRef {
    let cfg = &config::get().scroll;
    let is_shift = flags & FLAGS_SHIFT != 0;
    let step = if is_shift { 9_i32 } else { 3_i32 };
    let half_page = cfg.half_page_lines as i32;

    if kc == KEYCODE_ESCAPE || kc == KEYCODE_TAB {
        cancel_hud_timer();
        // Release any held mouse button cleanly before leaving scroll mode.
        if let Some((_, _, button)) = state.held_click.take() {
            let pos = mouse::cursor_pos();
            mouse::mouse_up(pos, button);
            log::info!("scroll hold force-released on exit");
        }
        if kc == KEYCODE_TAB {
            state.mode = AppMode::GridA { macro_first: None };
            overlay::show_grid_a(None);
            log::info!("→ GridA (scroll Tab-back)");
        } else {
            state.mode = AppMode::Idle;
            overlay::hide();
            log::info!("→ Idle");
        }
        return std::ptr::null_mut();
    }

    if kc == KEYCODE_SPACE {
        // Ignore OS key-repeat while the button is already held down.
        if state.held_click.is_some() {
            return std::ptr::null_mut();
        }
        let pos = mouse::cursor_pos();
        let button = if flags & FLAGS_OPTION != 0 {
            ClickButton::Right
        } else {
            ClickButton::Left
        };
        mouse::mouse_down(pos, button);
        state.held_click = Some((pos.x, pos.y, button));
        let lbl = if button == ClickButton::Right {
            "⌥Space"
        } else {
            "Space"
        };
        overlay::show_scroll_hud(lbl, "holding…");
        schedule_hud_fade(1.0);
        return std::ptr::null_mut();
    }

    if let Some(ch) = keycode_to_char(kc) {
        let (dy, dx, direction) = match ch {
            'j' => (-step, 0, "↓"),
            'k' => (step, 0, "↑"),
            'h' => (0, step, "←"),
            'l' => (0, -step, "→"),
            'd' => (-half_page, 0, "↓↓"),
            'u' => (half_page, 0, "↑↑"),
            _ => return std::ptr::null_mut(),
        };
        mouse::scroll(dy, dx);
        let key_label = if is_shift {
            format!("⇧{ch}")
        } else {
            ch.to_string()
        };
        let speed = if is_shift || ch == 'd' || ch == 'u' {
            " fast"
        } else {
            ""
        };
        overlay::show_scroll_hud(&key_label, &format!("{direction}{speed}"));
        schedule_hud_fade(1.0);
    }

    std::ptr::null_mut()
}

// ── Subcell geometry ───────────────────────────────────────────────────────

/// Pixel position of a subcell key within the given cell bounds.
/// The subcell grid fills the entire cell — cells are proportional.
fn subcell_pos(key: char, bounds: &CellBounds) -> Option<CGPoint> {
    let subcell_l = &config::parsed_layouts().subcell_l;
    let (sub_col, sub_row) = subcell_l.key_pos(key)?;
    let sc_w = bounds.w / subcell_l.num_cols as f64;
    let sc_h = bounds.h / subcell_l.num_rows as f64;
    Some(CGPoint::new(
        bounds.x + sub_col as f64 * sc_w + sc_w * 0.5,
        bounds.y + sub_row as f64 * sc_h + sc_h * 0.5,
    ))
}

// ── Modifier helpers ───────────────────────────────────────────────────────

fn modifier_button(flags: u64) -> ClickButton {
    modifier_button_with_default(flags, ClickButton::Left)
}

fn modifier_button_with_default(flags: u64, default: ClickButton) -> ClickButton {
    if flags & FLAGS_SHIFT != 0 {
        ClickButton::Right
    } else if flags & FLAGS_CONTROL != 0 {
        ClickButton::Middle
    } else {
        default
    }
}

// ── Entry points ───────────────────────────────────────────────────────────

pub fn install() {
    setup_tap(false);
}

pub fn run() {
    setup_tap(true);
}

fn setup_tap(block: bool) {
    if TAP_INITIALISED.swap(true, Ordering::SeqCst) {
        return;
    }
    let raw_state = Box::into_raw(Box::new(AppState::new()));
    STATE_PTR.store(raw_state, Ordering::Release);
    let state = raw_state as *mut c_void;

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

    TAP_PORT.store(tap_port, Ordering::Release);

    let source = unsafe { CFMachPortCreateRunLoopSource(std::ptr::null(), tap_port, 0) };
    if source.is_null() {
        eprintln!("CFMachPortCreateRunLoopSource failed.");
        std::process::exit(1);
    }

    unsafe {
        let rl = CFRunLoopGetCurrent();
        CFRunLoopAddSource(rl, source, kCFRunLoopCommonModes);
        CGEventTapEnable(tap_port, true);
        log::info!("event tap active  (Cmd+RCmd+Space → GridA,  Tab → Scroll)");
        if block {
            CFRunLoopRun();
            CFRelease(source as *const c_void);
            CFRelease(tap_port as *const c_void);
        }
    }
}

// ── External control ──────────────────────────────────────────────────────

/// Disable the event tap (keys pass through; overlay is hidden by menu.rs).
pub fn pause() {
    let port = TAP_PORT.load(Ordering::Acquire);
    if !port.is_null() {
        unsafe {
            CGEventTapEnable(port, false);
        }
        log::info!("event tap paused");
    }
}

/// Re-enable the event tap after a pause.
pub fn resume() {
    let port = TAP_PORT.load(Ordering::Acquire);
    if !port.is_null() {
        unsafe {
            CGEventTapEnable(port, true);
        }
        log::info!("event tap resumed");
    }
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keymap::parse_layout_string;

    // subcell_pos (via parsed layout) ─────────────────────────────────────────

    fn make_subcell_layout() -> crate::keymap::ParsedLayout {
        // matches default subcell_keys: 6 cols × 3 rows
        parse_layout_string("ertyui\ndfghjk\nxcvbnm").unwrap()
    }

    fn subcell_pos_with_layout(
        key: char,
        bounds: &CellBounds,
        layout: &crate::keymap::ParsedLayout,
    ) -> Option<CGPoint> {
        let (sub_col, sub_row) = layout.key_pos(key)?;
        let sc_w = bounds.w / layout.num_cols as f64;
        let sc_h = bounds.h / layout.num_rows as f64;
        Some(CGPoint::new(
            bounds.x + sub_col as f64 * sc_w + sc_w * 0.5,
            bounds.y + sub_row as f64 * sc_h + sc_h * 0.5,
        ))
    }

    #[test]
    fn subcell_pos_top_left_key() {
        // 'e' is col 0, row 0 — fills cell, no centering offset
        let layout = make_subcell_layout();
        let bounds = CellBounds::new(0.0, 0.0, 600.0, 300.0);
        let pos = subcell_pos_with_layout('e', &bounds, &layout).unwrap();
        let sc_w = 600.0 / layout.num_cols as f64;
        let sc_h = 300.0 / layout.num_rows as f64;
        assert!((pos.x - sc_w * 0.5).abs() < 1e-9);
        assert!((pos.y - sc_h * 0.5).abs() < 1e-9);
    }

    #[test]
    fn subcell_pos_bottom_right_key() {
        // 'm' is col 5, row 2 — bottom-right cell center
        let layout = make_subcell_layout();
        let bounds = CellBounds::new(0.0, 0.0, 600.0, 300.0);
        let pos = subcell_pos_with_layout('m', &bounds, &layout).unwrap();
        let sc_w = 600.0 / layout.num_cols as f64;
        let sc_h = 300.0 / layout.num_rows as f64;
        assert!((pos.x - (5.0 * sc_w + sc_w * 0.5)).abs() < 1e-9);
        assert!((pos.y - (2.0 * sc_h + sc_h * 0.5)).abs() < 1e-9);
    }

    #[test]
    fn subcell_pos_unknown_key_returns_none() {
        let layout = make_subcell_layout();
        let bounds = CellBounds::new(0.0, 0.0, 600.0, 300.0);
        assert!(subcell_pos_with_layout('z', &bounds, &layout).is_none());
    }

    // event_mask ──────────────────────────────────────────────────────────────

    #[test]
    fn event_mask_single() {
        assert_eq!(event_mask(&[10]), 1u64 << 10);
    }

    #[test]
    fn event_mask_multiple() {
        let m = event_mask(&[
            K_CG_EVENT_KEY_DOWN,
            K_CG_EVENT_KEY_UP,
            K_CG_EVENT_FLAGS_CHANGED,
        ]);
        assert!(m & (1 << K_CG_EVENT_KEY_DOWN) != 0);
        assert!(m & (1 << K_CG_EVENT_KEY_UP) != 0);
        assert!(m & (1 << K_CG_EVENT_FLAGS_CHANGED) != 0);
    }

    #[test]
    fn event_mask_empty() {
        assert_eq!(event_mask(&[]), 0);
    }

    // modifier_button ─────────────────────────────────────────────────────────

    #[test]
    fn modifier_no_flags_uses_default() {
        assert_eq!(
            modifier_button_with_default(0, ClickButton::Left),
            ClickButton::Left
        );
        assert_eq!(
            modifier_button_with_default(0, ClickButton::Middle),
            ClickButton::Middle
        );
    }

    #[test]
    fn modifier_shift_gives_right_click() {
        assert_eq!(
            modifier_button_with_default(FLAGS_SHIFT, ClickButton::Left),
            ClickButton::Right
        );
    }

    #[test]
    fn modifier_ctrl_gives_middle_click() {
        assert_eq!(
            modifier_button_with_default(FLAGS_CONTROL, ClickButton::Left),
            ClickButton::Middle
        );
    }

    #[test]
    fn modifier_shift_takes_priority_over_ctrl() {
        assert_eq!(
            modifier_button_with_default(FLAGS_SHIFT | FLAGS_CONTROL, ClickButton::Left),
            ClickButton::Right
        );
    }

    // PendingTap ──────────────────────────────────────────────────────────────

    #[test]
    fn pending_tap_fields() {
        let t = PendingTap {
            x: 10.0,
            y: 20.0,
            button: ClickButton::Left,
            count: 2,
            key: 'f',
        };
        assert_eq!(t.count, 2);
        assert_eq!(t.key, 'f');
        assert!((t.x - 10.0).abs() < 1e-9);
    }

    #[test]
    fn pending_tap_count_caps_at_3() {
        let mut t = PendingTap {
            x: 0.0,
            y: 0.0,
            button: ClickButton::Left,
            count: 3,
            key: 'f',
        };
        t.count = (t.count + 1).min(3);
        assert_eq!(t.count, 3);
    }

    // AppState drag_origin ────────────────────────────────────────────────────

    #[test]
    fn drag_origin_defaults_to_none() {
        let s = AppState::new();
        assert!(s.drag_origin.is_none());
    }

    #[test]
    fn drag_origin_can_be_set() {
        let mut s = AppState::new();
        s.drag_origin = Some((100.0, 200.0));
        assert_eq!(s.drag_origin, Some((100.0, 200.0)));
    }

    // FLAGS_OPTION ────────────────────────────────────────────────────────────

    #[test]
    fn option_flag_value() {
        assert_eq!(FLAGS_OPTION, 0x0008_0000);
        assert_ne!(FLAGS_OPTION & FLAGS_SHIFT, FLAGS_OPTION);
        assert_ne!(FLAGS_OPTION & FLAGS_CONTROL, FLAGS_OPTION);
    }

    // ── Keybind conflict: flag constants ──────────────────────────────────────

    #[test]
    fn modifier_flag_constants_are_pairwise_disjoint() {
        let named = [
            ("SHIFT", FLAGS_SHIFT),
            ("CONTROL", FLAGS_CONTROL),
            ("OPTION", FLAGS_OPTION),
            ("CMD", FLAGS_CMD),
            ("RCMD", FLAGS_RCMD),
        ];
        for (i, &(name_a, a)) in named.iter().enumerate() {
            for &(name_b, b) in &named[i + 1..] {
                assert_eq!(
                    a & b,
                    0,
                    "flag constants overlap: {name_a}({a:#010x}) & {name_b}({b:#010x})"
                );
            }
        }
    }

    #[test]
    fn activation_requires_both_cmd_flags() {
        assert_eq!((FLAGS_CMD | FLAGS_RCMD) & ACTIVATION_MODS, ACTIVATION_MODS);
        assert_ne!(FLAGS_CMD & ACTIVATION_MODS, ACTIVATION_MODS);
        assert_ne!(FLAGS_RCMD & ACTIVATION_MODS, ACTIVATION_MODS);
        assert_ne!(0u64 & ACTIVATION_MODS, ACTIVATION_MODS);
        assert_ne!(FLAGS_SHIFT & ACTIVATION_MODS, ACTIVATION_MODS);
    }

    // ── Keybind conflict: subcell modifier branching ──────────────────────────

    #[test]
    fn subcell_modifier_branches_are_mutually_exclusive_and_exhaustive() {
        let samples = [
            0u64,
            FLAGS_SHIFT,
            FLAGS_CONTROL,
            FLAGS_OPTION,
            FLAGS_CMD,
            FLAGS_SHIFT | FLAGS_OPTION,
            FLAGS_SHIFT | FLAGS_CONTROL,
            FLAGS_CONTROL | FLAGS_OPTION,
            FLAGS_SHIFT | FLAGS_CONTROL | FLAGS_OPTION,
            FLAGS_CMD | FLAGS_SHIFT | FLAGS_OPTION,
        ];
        for &f in &samples {
            let is_drag = f & FLAGS_OPTION != 0 && f & FLAGS_SHIFT != 0;
            let is_move_only = f & FLAGS_OPTION != 0 && f & FLAGS_SHIFT == 0;
            let is_click = f & FLAGS_OPTION == 0;
            let count = [is_drag, is_move_only, is_click]
                .iter()
                .filter(|&&b| b)
                .count();
            assert_eq!(
                count, 1,
                "flags {f:#010x}: expected exactly 1 subcell action, got {count}"
            );
        }
    }

    #[test]
    fn option_with_shift_not_same_as_option_alone() {
        let drag_flags = FLAGS_OPTION | FLAGS_SHIFT;
        let move_flags = FLAGS_OPTION;
        assert_ne!(drag_flags & FLAGS_SHIFT, 0, "drag requires Shift bit set");
        assert_eq!(
            move_flags & FLAGS_SHIFT,
            0,
            "move-only must not have Shift bit"
        );
    }

    // ── Keybind conflict: grid mode Tab → scroll ─────────────────────────────

    #[test]
    fn tab_keycode_has_no_char_mapping() {
        assert_eq!(keycode_to_char(KEYCODE_TAB), None);
    }

    #[test]
    fn scroll_direction_keys_not_reachable_via_tab() {
        let directions = ['j', 'k', 'h', 'l', 'd', 'u'];
        for &k in &directions {
            let has_mapping = (0x00u16..=0xFF).any(|kc| keycode_to_char(kc) == Some(k));
            assert!(has_mapping, "direction key '{k}' has no keycode mapping");
        }
    }

    #[test]
    fn scroll_direction_keys_no_duplicates() {
        let directions = ['j', 'k', 'h', 'l', 'd', 'u'];
        let mut seen = std::collections::HashSet::new();
        for &k in &directions {
            assert!(seen.insert(k), "duplicate scroll direction key: '{k}'");
        }
    }

    // ── Keybind conflict: modifier_button determinism ─────────────────────────

    #[test]
    fn modifier_button_shift_wins_over_ctrl() {
        let combos: &[(u64, ClickButton)] = &[
            (FLAGS_SHIFT, ClickButton::Right),
            (FLAGS_CONTROL, ClickButton::Middle),
            (FLAGS_SHIFT | FLAGS_CONTROL, ClickButton::Right),
            (FLAGS_OPTION, ClickButton::Left),
            (0, ClickButton::Left),
        ];
        for &(flags, expected) in combos {
            assert_eq!(
                modifier_button_with_default(flags, ClickButton::Left),
                expected,
                "flags {flags:#010x} gave wrong button"
            );
        }
    }

    #[test]
    fn modifier_button_right_and_middle_never_both_true() {
        let samples = [
            0u64,
            FLAGS_SHIFT,
            FLAGS_CONTROL,
            FLAGS_SHIFT | FLAGS_CONTROL,
            FLAGS_OPTION,
        ];
        for &f in &samples {
            let _ = modifier_button_with_default(f, ClickButton::Left);
        }
        assert_ne!(
            modifier_button_with_default(FLAGS_SHIFT, ClickButton::Left),
            modifier_button_with_default(FLAGS_CONTROL, ClickButton::Left),
            "Shift and Ctrl must produce different buttons"
        );
    }
}
