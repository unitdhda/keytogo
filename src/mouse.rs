use std::os::raw::c_void;
use std::sync::atomic::{AtomicPtr, Ordering};

use crate::state::ClickButton;

// ── Display helpers ────────────────────────────────────────────────────────

type CGDirectDisplayID = u32;

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGMainDisplayID() -> CGDirectDisplayID;
    fn CGDisplayPixelsWide(display: CGDirectDisplayID) -> usize;
    fn CGDisplayPixelsHigh(display: CGDirectDisplayID) -> usize;
}

/// Returns (width, height) of the main display in pixels.
pub fn screen_size() -> (f64, f64) {
    unsafe {
        let id = CGMainDisplayID();
        (CGDisplayPixelsWide(id) as f64, CGDisplayPixelsHigh(id) as f64)
    }
}

/// Returns the pixel center of the main display.
pub fn screen_center() -> CGPoint {
    let (w, h) = screen_size();
    CGPoint::new(w / 2.0, h / 2.0)
}

// ── CGPoint ────────────────────────────────────────────────────────────────

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct CGPoint {
    pub x: f64,
    pub y: f64,
}

impl CGPoint {
    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }
}

// ── FFI types ──────────────────────────────────────────────────────────────

type CGEventRef       = *mut c_void;
type CGEventSourceRef = *mut c_void;

// ── CGEventType ────────────────────────────────────────────────────────────

const K_CG_EVENT_LEFT_MOUSE_DOWN:    u32 = 1;
const K_CG_EVENT_LEFT_MOUSE_UP:      u32 = 2;
const K_CG_EVENT_RIGHT_MOUSE_DOWN:   u32 = 3;
const K_CG_EVENT_RIGHT_MOUSE_UP:     u32 = 4;
const K_CG_EVENT_LEFT_MOUSE_DRAGGED: u32 = 6;
const K_CG_EVENT_OTHER_MOUSE_DOWN:   u32 = 25;
const K_CG_EVENT_OTHER_MOUSE_UP:     u32 = 26;

// ── CGMouseButton ──────────────────────────────────────────────────────────

const K_CG_MOUSE_BUTTON_LEFT:   u32 = 0;
const K_CG_MOUSE_BUTTON_RIGHT:  u32 = 1;
const K_CG_MOUSE_BUTTON_CENTER: u32 = 2;

// ── CGEventField ───────────────────────────────────────────────────────────

// kCGMouseEventClickState — tells the target app this is click N of a multi-click
const K_CG_MOUSE_EVENT_CLICK_STATE: u32 = 1;

// ── CGScrollEventUnit / CGEventTapLocation ─────────────────────────────────

const K_CG_SCROLL_EVENT_UNIT_LINE: u32 = 1;
// kCGSessionEventTap: inject at the session boundary, correct for app-targeted events.
const K_CG_SESSION_EVENT_TAP: u32 = 1;

// ── CGEventSource state ────────────────────────────────────────────────────

// kCGEventSourceStateCombinedSessionState: correct for user-session processes.
const K_CG_EVENT_SOURCE_STATE_COMBINED_SESSION: u32 = 1;

// ── Framework bindings ─────────────────────────────────────────────────────

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGEventCreate(source: CGEventSourceRef) -> CGEventRef;
    fn CGEventGetLocation(event: CGEventRef) -> CGPoint;
    fn CGEventSourceCreate(state_id: u32) -> CGEventSourceRef;
    fn CGEventSourceSetLocalEventsSuppressionInterval(source: CGEventSourceRef, seconds: f64);
}

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGEventCreateMouseEvent(
        source:                CGEventSourceRef,
        mouse_type:            u32,
        mouse_cursor_position: CGPoint,
        mouse_button:          u32,
    ) -> CGEventRef;

    // Fixed-arity variant of the variadic CGEventCreateScrollWheelEvent.
    // Available since macOS 10.13 (High Sierra).
    fn CGEventCreateScrollWheelEvent2(
        source:      CGEventSourceRef,
        units:       u32,
        wheel_count: u32,
        wheel1:      i32, // vertical:   positive = scroll up
        wheel2:      i32, // horizontal: positive = scroll left
        wheel3:      i32,
    ) -> CGEventRef;

    fn CGEventSetIntegerValueField(event: CGEventRef, field: u32, value: i64);
    // Zeroing flags before post strips any live modifier state inherited from the HID table.
    fn CGEventSetFlags(event: CGEventRef, flags: u64);
    fn CGEventPost(tap: u32, event: CGEventRef);
    fn CGWarpMouseCursorPosition(new_cursor_position: CGPoint);
    fn CFRelease(cf: *const c_void);
}

// ── Event source singleton ─────────────────────────────────────────────────

// Lazily created on first use. Using an explicit source:
//   • eliminates modifier-flag contamination (NULL source inherits live HID flags)
//   • allows zeroing the 0.25 s hardware-event suppression interval
static EVENT_SOURCE: AtomicPtr<c_void> = AtomicPtr::new(std::ptr::null_mut());

fn event_source() -> CGEventSourceRef {
    let ptr = EVENT_SOURCE.load(Ordering::Acquire);
    if !ptr.is_null() {
        return ptr;
    }
    unsafe {
        let src = CGEventSourceCreate(K_CG_EVENT_SOURCE_STATE_COMBINED_SESSION);
        if !src.is_null() {
            CGEventSourceSetLocalEventsSuppressionInterval(src, 0.0);
            EVENT_SOURCE.store(src, Ordering::Release);
        }
        src
    }
}

// ── Public API ─────────────────────────────────────────────────────────────

/// Returns the current cursor position by querying a null CGEvent.
pub fn cursor_pos() -> CGPoint {
    unsafe {
        let ev = CGEventCreate(std::ptr::null_mut());
        let pt = CGEventGetLocation(ev);
        CFRelease(ev as *const c_void);
        pt
    }
}

/// Instantly teleport the cursor without generating a move event.
pub fn move_cursor(x: f64, y: f64) {
    unsafe { CGWarpMouseCursorPosition(CGPoint::new(x, y)) }
}

/// Post a mouse-button-down event only. Pair with `mouse_up` for hold behaviour.
pub fn mouse_down(pos: CGPoint, button: ClickButton) {
    let (down_type, _, btn) = button_event_types(button);
    unsafe {
        let ev = CGEventCreateMouseEvent(event_source(), down_type, pos, btn);
        if !ev.is_null() {
            CGEventSetFlags(ev, 0);
            CGEventPost(K_CG_SESSION_EVENT_TAP, ev);
            CFRelease(ev as *const c_void);
        }
    }
}

/// Post a mouse-button-up event. Called on key release after `mouse_down`.
pub fn mouse_up(pos: CGPoint, button: ClickButton) {
    let (_, up_type, btn) = button_event_types(button);
    unsafe {
        let ev = CGEventCreateMouseEvent(event_source(), up_type, pos, btn);
        if !ev.is_null() {
            CGEventSetFlags(ev, 0);
            CGEventPost(K_CG_SESSION_EVENT_TAP, ev);
            CFRelease(ev as *const c_void);
        }
    }
}

/// Click at `pos` with the given button and click count.
///
/// For multi-click (count > 1) each iteration increments kCGMouseEventClickState
/// so the receiving app sees a proper double/triple click sequence.
pub fn click(pos: CGPoint, button: ClickButton, count: u8) {
    let (down_type, up_type, btn) = button_event_types(button);
    unsafe {
        for n in 1..=count {
            let down = CGEventCreateMouseEvent(event_source(), down_type, pos, btn);
            if !down.is_null() {
                CGEventSetIntegerValueField(down, K_CG_MOUSE_EVENT_CLICK_STATE, n as i64);
                CGEventSetFlags(down, 0);
                CGEventPost(K_CG_SESSION_EVENT_TAP, down);
                CFRelease(down as *const c_void);
            }

            let up = CGEventCreateMouseEvent(event_source(), up_type, pos, btn);
            if !up.is_null() {
                CGEventSetIntegerValueField(up, K_CG_MOUSE_EVENT_CLICK_STATE, n as i64);
                CGEventSetFlags(up, 0);
                CGEventPost(K_CG_SESSION_EVENT_TAP, up);
                CFRelease(up as *const c_void);
            }
        }
    }
}

/// Post a scroll event at the current cursor position.
///
/// `dy`: line units (positive = up, negative = down).
/// `dx`: line units (positive = left, negative = right).
///
/// macOS applies the "natural scrolling" user preference to synthetic events,
/// so visual direction matches whatever the user has configured system-wide.
pub fn scroll(dy: i32, dx: i32) {
    unsafe {
        let ev = CGEventCreateScrollWheelEvent2(
            event_source(),
            K_CG_SCROLL_EVENT_UNIT_LINE,
            2, // wheel_count: 2 enables both axes
            dy,
            dx,
            0,
        );
        if !ev.is_null() {
            CGEventSetFlags(ev, 0);
            CGEventPost(K_CG_SESSION_EVENT_TAP, ev);
            CFRelease(ev as *const c_void);
        }
    }
}

/// Press and hold at `from`, move to `to`, release.
///
/// Posts: LeftMouseDown → CGWarp → LeftMouseDragged → LeftMouseUp.
pub fn drag(from: CGPoint, to: CGPoint) {
    unsafe {
        let down = CGEventCreateMouseEvent(
            event_source(),
            K_CG_EVENT_LEFT_MOUSE_DOWN,
            from,
            K_CG_MOUSE_BUTTON_LEFT,
        );
        if !down.is_null() {
            CGEventSetFlags(down, 0);
            CGEventPost(K_CG_SESSION_EVENT_TAP, down);
            CFRelease(down as *const c_void);
        }

        CGWarpMouseCursorPosition(to);

        let dragged = CGEventCreateMouseEvent(
            event_source(),
            K_CG_EVENT_LEFT_MOUSE_DRAGGED,
            to,
            K_CG_MOUSE_BUTTON_LEFT,
        );
        if !dragged.is_null() {
            CGEventSetFlags(dragged, 0);
            CGEventPost(K_CG_SESSION_EVENT_TAP, dragged);
            CFRelease(dragged as *const c_void);
        }

        let up = CGEventCreateMouseEvent(
            event_source(),
            K_CG_EVENT_LEFT_MOUSE_UP,
            to,
            K_CG_MOUSE_BUTTON_LEFT,
        );
        if !up.is_null() {
            CGEventSetFlags(up, 0);
            CGEventPost(K_CG_SESSION_EVENT_TAP, up);
            CFRelease(up as *const c_void);
        }
    }
}

// ── Phase 1 smoke test ─────────────────────────────────────────────────────

/// Interactive smoke test for all mouse primitives.
///
/// Opens a text editor or any window before running — the test will click,
/// scroll, and drag inside whatever window is focused when the delay ends.
pub fn smoke_test() {
    use std::thread::sleep;
    use std::time::Duration;

    let (w, h) = screen_size();
    let center = CGPoint::new(w / 2.0, h / 2.0);
    let offset = CGPoint::new(w / 2.0 + 80.0, h / 2.0);

    println!("=== Phase 1 mouse smoke test ===");
    println!("Focus any window (e.g. TextEdit). Starting in 3 seconds...\n");
    sleep(Duration::from_secs(3));

    step("move_cursor → screen center", || move_cursor(center.x, center.y));
    sleep(Duration::from_millis(400));
    step("click left ×1", || click(center, ClickButton::Left, 1));
    sleep(Duration::from_millis(400));
    step("click left ×2 (double)", || click(center, ClickButton::Left, 2));
    sleep(Duration::from_millis(400));
    step("click left ×3 (triple)", || click(center, ClickButton::Left, 3));
    sleep(Duration::from_millis(400));
    step("click right ×1 (context menu)", || click(center, ClickButton::Right, 1));
    sleep(Duration::from_millis(600));
    step("dismiss context menu (Escape key event)", post_escape);
    sleep(Duration::from_millis(400));
    step("scroll down 5 lines (dy=-5)", || scroll(-5, 0));
    sleep(Duration::from_millis(400));
    step("scroll up 5 lines (dy=+5)", || scroll(5, 0));
    sleep(Duration::from_millis(400));
    step("scroll right 3 (dx=-3)", || scroll(0, -3));
    sleep(Duration::from_millis(400));
    step("scroll left 3 (dx=+3)", || scroll(0, 3));
    sleep(Duration::from_millis(400));
    step("drag center → center+80px", || drag(center, offset));
    sleep(Duration::from_millis(400));

    println!("\n=== smoke test complete ===");
    println!("Check: cursor moved, single/double/triple click registered,");
    println!("       right-click showed context menu, scroll moved content,");
    println!("       drag selected text (if in a text field).");
}

fn step(label: &str, f: impl FnOnce()) {
    println!("  » {}", label);
    f();
}

// Sends a bare Escape key down+up to dismiss menus.
fn post_escape() {
    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        fn CGEventCreateKeyboardEvent(
            source:   CGEventSourceRef,
            keycode:  u16,
            key_down: bool,
        ) -> CGEventRef;
    }

    const KEYCODE_ESCAPE: u16 = 0x35;

    unsafe {
        let down = CGEventCreateKeyboardEvent(std::ptr::null_mut(), KEYCODE_ESCAPE, true);
        CGEventPost(K_CG_SESSION_EVENT_TAP, down);
        CFRelease(down as *const c_void);
        let up = CGEventCreateKeyboardEvent(std::ptr::null_mut(), KEYCODE_ESCAPE, false);
        CGEventPost(K_CG_SESSION_EVENT_TAP, up);
        CFRelease(up as *const c_void);
    }
}

// ── Helper ─────────────────────────────────────────────────────────────────

fn button_event_types(button: ClickButton) -> (u32, u32, u32) {
    match button {
        ClickButton::Left => (
            K_CG_EVENT_LEFT_MOUSE_DOWN,
            K_CG_EVENT_LEFT_MOUSE_UP,
            K_CG_MOUSE_BUTTON_LEFT,
        ),
        ClickButton::Right => (
            K_CG_EVENT_RIGHT_MOUSE_DOWN,
            K_CG_EVENT_RIGHT_MOUSE_UP,
            K_CG_MOUSE_BUTTON_RIGHT,
        ),
        ClickButton::Middle => (
            K_CG_EVENT_OTHER_MOUSE_DOWN,
            K_CG_EVENT_OTHER_MOUSE_UP,
            K_CG_MOUSE_BUTTON_CENTER,
        ),
    }
}
