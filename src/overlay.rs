/// Full-screen transparent overlay with multiple visual states:
///   GridA   — macro grid with optional macro-key highlight
///   Subcell — semi-transparent background + selected cell + ortholinear subcell grid
use std::cell::Cell;
use std::sync::atomic::{AtomicBool, Ordering};

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{declare_class, msg_send, msg_send_id, mutability, ClassType, DeclaredClass};
use objc2_app_kit::{NSBackingStoreType, NSColor, NSScreen, NSView, NSWindow, NSWindowStyleMask};
use objc2_foundation::{MainThreadMarker, NSPoint, NSRect, NSSize, NSString};

use crate::config::{self, HudConfig};
use crate::keymap::layout_geom;

// ── Overlay state ──────────────────────────────────────────────────────────

#[derive(Clone, Debug, Default)]
pub enum OverlayState {
    #[default]
    Hidden,
    /// Macro grid; optionally highlights the selected macro key.
    GridA { macro_first: Option<char> },
    /// Selected cell area (screen coords, y from top) + ortho subcell grid.
    Subcell { x: f64, y: f64, w: f64, h: f64 },
    /// KeyCastr-style HUD shown during scroll mode: key name + action label.
    /// Empty key = "Scroll Mode" hint; non-empty = last key pressed and its effect.
    ScrollHud { key: String, action: String },
}

thread_local! {
    // No `const { }` — String fields make the full type non-const-constructable.
    static STATE: Cell<OverlayState> = Cell::new(OverlayState::Hidden);
}

fn set_state(s: OverlayState) {
    STATE.with(|c| c.set(s));
}

static INITIALISED: AtomicBool = AtomicBool::new(false);
static mut VIEW_PTR: *mut AnyObject = std::ptr::null_mut();

// ── OverlayView ────────────────────────────────────────────────────────────

pub struct OverlayViewIvars;

declare_class!(
    pub struct OverlayView;

    unsafe impl ClassType for OverlayView {
        type Super = NSView;
        type Mutability = mutability::MainThreadOnly;
        const NAME: &'static str = "OverlayView";
    }

    impl DeclaredClass for OverlayView {
        type Ivars = OverlayViewIvars;
    }

    unsafe impl OverlayView {
        #[method(drawRect:)]
        fn draw_rect(&self, _dirty: NSRect) {
            let state = STATE.with(|c| c.take());
            STATE.with(|c| c.set(state.clone()));

            match state {
                OverlayState::Hidden => {}
                OverlayState::GridA { macro_first } => draw_grid_a(self, macro_first),
                OverlayState::Subcell { x, y, w, h } => draw_subcell_layer(self, x, y, w, h),
                OverlayState::ScrollHud { key, action } => draw_scroll_hud(self, &key, &action),
            }
        }
    }
);

// ── Renderers ──────────────────────────────────────────────────────────────

fn draw_grid_a(view: &OverlayView, macro_first: Option<char>) {
    let bounds = view.bounds();
    let sw = bounds.size.width;
    let sh = bounds.size.height;

    let layouts = config::parsed_layouts();
    let macro_l = &layouts.macro_l;
    let sub_l   = &layouts.sub_l;

    let g  = layout_geom(sw, sh, macro_l.num_cols, macro_l.num_rows, sub_l.num_cols, sub_l.num_rows);
    let ss = g.sub_size;

    draw_dim_bg(bounds);

    for row in 0..macro_l.num_rows {
        for col in 0..macro_l.num_cols {
            let macro_key = macro_l.keys[row][col];
            // Centered grid origin; NSView y increases upward.
            let mx = g.offset_x + col as f64 * g.macro_w;
            let my = sh - g.offset_y - (row + 1) as f64 * g.macro_h;
            let macro_rect = NSRect {
                origin: NSPoint { x: mx, y: my },
                size: NSSize { width: g.macro_w, height: g.macro_h },
            };

            let is_selected = macro_first == Some(macro_key);
            if is_selected {
                fill_rect(macro_rect, 1.0, 1.0, 1.0, 0.10);
            }
            stroke_rect(macro_rect, 1.0, 1.0, 1.0, 0.30);

            let sub_alpha = if is_selected { 0.85 } else { 0.60 };
            for sr in 0..sub_l.num_rows {
                for sc in 0..sub_l.num_cols {
                    let sub_key = sub_l.keys[sr][sc];
                    let sx = mx + sc as f64 * ss;
                    // sr=0 is visual top → highest NSView y
                    let sy = my + (sub_l.num_rows - 1 - sr) as f64 * ss;
                    let sub_rect = NSRect {
                        origin: NSPoint { x: sx, y: sy },
                        size: NSSize { width: ss, height: ss },
                    };
                    stroke_rect(sub_rect, 1.0, 1.0, 1.0, sub_alpha * 0.5);
                    draw_label_alpha(
                        &format!("{}{}", macro_key, sub_key),
                        sx + ss * 0.20,
                        sy + ss * 0.28,
                        10.0,
                        sub_alpha,
                    );
                }
            }
        }
    }
}

fn draw_subcell_layer(view: &OverlayView, cell_x: f64, cell_y: f64, cell_w: f64, cell_h: f64) {
    let bounds = view.bounds();
    let sh = bounds.size.height;

    draw_dim_bg(bounds);

    // Highlight the selected cell.
    // cell_y is screen-y-from-top; convert to NSView y (from bottom).
    let nsview_y = sh - cell_y - cell_h;
    let cell_rect = NSRect {
        origin: NSPoint { x: cell_x, y: nsview_y },
        size: NSSize { width: cell_w, height: cell_h },
    };
    fill_rect(cell_rect, 1.0, 1.0, 0.0, 0.40);
    stroke_rect(cell_rect, 1.0, 1.0, 0.0, 0.80);

    let subcell_l = &config::parsed_layouts().subcell_l;
    let sc_cols = subcell_l.num_cols;
    let sc_rows = subcell_l.num_rows;

    // Square sub-cells centered within the selected cell.
    let sc_size     = (cell_w / sc_cols as f64).min(cell_h / sc_rows as f64);
    let sc_offset_x = (cell_w - sc_size * sc_cols as f64) / 2.0;
    let sc_offset_y = (cell_h - sc_size * sc_rows as f64) / 2.0;

    for row in 0..sc_rows {
        for col in 0..sc_cols {
            let key = subcell_l.keys[row][col];
            let x = cell_x + sc_offset_x + col as f64 * sc_size;
            // row=0 is visual top → highest NSView y
            let y = nsview_y + sc_offset_y + (sc_rows - 1 - row) as f64 * sc_size;
            let sub_rect = NSRect {
                origin: NSPoint { x, y },
                size: NSSize { width: sc_size, height: sc_size },
            };

            fill_rect(sub_rect, 0.0, 1.0, 0.533, 0.12);
            stroke_rect(sub_rect, 0.0, 1.0, 0.533, 0.70);
            draw_label(&key.to_string(), x + sc_size * 0.3, y + sc_size * 0.35, 10.0);
        }
    }
}

// ── Drawing primitives ─────────────────────────────────────────────────────

fn draw_dim_bg(bounds: NSRect) {
    unsafe {
        // Dark beige: warm near-black with a sandy undertone, more visible than pure black.
        let bg = NSColor::colorWithSRGBRed_green_blue_alpha(0.10, 0.08, 0.05, 0.45);
        bg.setFill();
        objc2_app_kit::NSRectFill(bounds);
    }
}

fn fill_rect(rect: NSRect, r: f64, g: f64, b: f64, a: f64) {
    unsafe {
        let color = NSColor::colorWithSRGBRed_green_blue_alpha(r, g, b, a);
        color.setFill();
        objc2_app_kit::NSRectFill(rect);
    }
}

fn stroke_rect(rect: NSRect, r: f64, g: f64, b: f64, a: f64) {
    unsafe {
        let color = NSColor::colorWithSRGBRed_green_blue_alpha(r, g, b, a);
        color.setStroke();
        let path = objc2_app_kit::NSBezierPath::bezierPathWithRect(rect);
        path.stroke();
    }
}

fn fill_rounded_rect(rect: NSRect, radius: f64, r: f64, g: f64, b: f64, a: f64) {
    unsafe {
        let color = NSColor::colorWithSRGBRed_green_blue_alpha(r, g, b, a);
        color.setFill();
        let path: Retained<AnyObject> = msg_send_id![
            objc2_app_kit::NSBezierPath::class(),
            bezierPathWithRoundedRect: rect,
            xRadius: radius,
            yRadius: radius
        ];
        let _: () = msg_send![&*path, fill];
    }
}

fn draw_label(text: &str, x: f64, y: f64, size: f64) {
    draw_label_alpha(text, x, y, size, 0.90);
}

fn draw_label_alpha(text: &str, x: f64, y: f64, size: f64, alpha: f64) {
    // Shadow pass — dark offset provides contrast against any desktop background.
    draw_label_color(text, x + 0.8, y - 0.8, size, 0.05, 0.04, 0.02, (alpha * 0.6).min(1.0));
    // Main pass — white text on top.
    draw_label_color(text, x, y, size, 1.0, 1.0, 1.0, alpha);
}

fn draw_label_color(text: &str, x: f64, y: f64, size: f64, r: f64, g: f64, b: f64, a: f64) {
    unsafe {
        use objc2_app_kit::{NSFontAttributeName, NSForegroundColorAttributeName};

        let font = objc2_app_kit::NSFont::monospacedSystemFontOfSize_weight(
            size,
            objc2_app_kit::NSFontWeightRegular,
        );
        let color = NSColor::colorWithSRGBRed_green_blue_alpha(r, g, b, a);

        let font_obj: Retained<AnyObject> = Retained::cast(font);
        let color_obj: Retained<AnyObject> = Retained::cast(color);

        let keys: &[&NSString] = &[NSFontAttributeName, NSForegroundColorAttributeName];
        let attrs = objc2_foundation::NSDictionary::<NSString, AnyObject>::from_vec(
            keys,
            vec![font_obj, color_obj],
        );

        let ns_str = NSString::from_str(text);
        let pt = NSPoint { x, y };
        let _: () = msg_send![&*ns_str, drawAtPoint: pt withAttributes: &*attrs];
    }
}

// ── Scroll HUD renderer ────────────────────────────────────────────────────

/// Approximate display-pixel width of a string at a given point size (monospace).
fn text_px_width(text: &str, pt: f64) -> f64 {
    // Monospace char is ~0.60× pt wide; unicode arrows/symbols count as one glyph.
    text.chars().count() as f64 * pt * 0.60
}

fn hud_position(cfg: &HudConfig, sw: f64, sh: f64, hud_w: f64, hud_h: f64) -> (f64, f64) {
    let mx = cfg.margin_x;
    let my = cfg.margin_y;
    let (hud_x, hud_y) = match cfg.position.as_str() {
        "bottom-left"  => (mx, my),
        "bottom-right" => (sw - hud_w - mx, my),
        "top-center"   => ((sw - hud_w) / 2.0, sh - hud_h - my),
        "top-left"     => (mx, sh - hud_h - my),
        "top-right"    => (sw - hud_w - mx, sh - hud_h - my),
        _              => ((sw - hud_w) / 2.0, my), // "bottom-center" (default)
    };
    (hud_x, hud_y)
}

fn draw_scroll_hud(view: &OverlayView, key: &str, action: &str) {
    let bounds = view.bounds();
    let sw = bounds.size.width;
    let sh = bounds.size.height;
    let cfg = &config::get().hud;

    const HUD_H:     f64 = 54.0;
    const CAP_PAD:   f64 = 10.0;
    const CAP_H:     f64 = 34.0;
    const KEY_PT:    f64 = 14.0;
    const ACT_PT:    f64 = 13.0;
    const HINT_PT:   f64 = 10.5;

    let hud_w = if key.is_empty() {
        // Wide enough to display the full hint line with padding.
        let hint = "Scroll Mode  j↓ k↑ h← l→  ⇧=fast  Space=click";
        text_px_width(hint, HINT_PT) + 32.0
    } else {
        let cap_w = (text_px_width(key, KEY_PT) + 16.0).max(34.0);
        let act_w = text_px_width(action, ACT_PT);
        // cap_pad + cap_w + gap(14) + act_w + trailing(14)
        CAP_PAD + cap_w + 14.0 + act_w + 14.0
    };
    let hud_w = hud_w.max(120.0); // minimum sane width

    let (hud_x, hud_y) = hud_position(&cfg, sw, sh, hud_w, HUD_H);

    let hud_rect = NSRect {
        origin: NSPoint { x: hud_x, y: hud_y },
        size:   NSSize  { width: hud_w, height: HUD_H },
    };
    fill_rounded_rect(hud_rect, 14.0, 0.0, 0.0, 0.0, 0.88);

    if key.is_empty() {
        let hint = "Scroll Mode  j↓ k↑ h← l→  ⇧=fast  Space=click";
        draw_label_alpha(hint, hud_x + 16.0, hud_y + (HUD_H - HINT_PT) / 2.0 - 2.0, HINT_PT, 0.80);
    } else {
        let cap_w  = (text_px_width(key, KEY_PT) + 16.0).max(34.0);
        let cap_x  = hud_x + CAP_PAD;
        let cap_y  = hud_y + (HUD_H - CAP_H) / 2.0;
        let cap_rect = NSRect {
            origin: NSPoint { x: cap_x, y: cap_y },
            size:   NSSize  { width: cap_w, height: CAP_H },
        };
        fill_rounded_rect(cap_rect, 7.0, 0.28, 0.24, 0.18, 1.0);
        stroke_rect(cap_rect, 1.0, 1.0, 0.85, 0.35);

        let key_x = cap_x + (cap_w - text_px_width(key, KEY_PT)) / 2.0;
        let key_y = cap_y + (CAP_H - KEY_PT) / 2.0 - 1.0;
        draw_label_alpha(key, key_x, key_y, KEY_PT, 0.95);

        let act_x = cap_x + cap_w + 14.0;
        let act_y = hud_y + (HUD_H - ACT_PT) / 2.0 - 1.0;
        draw_label_alpha(action, act_x, act_y, ACT_PT, 0.85);
    }
}

// ── Public API ─────────────────────────────────────────────────────────────

pub fn init(mtm: MainThreadMarker) {
    if INITIALISED.swap(true, Ordering::SeqCst) {
        return;
    }

    let screen_frame = NSScreen::mainScreen(mtm)
        .map(|s| s.frame())
        .unwrap_or(NSRect {
            origin: NSPoint { x: 0.0, y: 0.0 },
            size: NSSize {
                width: 1440.0,
                height: 900.0,
            },
        });

    let window: Retained<NSWindow> = unsafe {
        NSWindow::initWithContentRect_styleMask_backing_defer(
            msg_send_id![NSWindow::class(), alloc],
            screen_frame,
            NSWindowStyleMask::Borderless,
            NSBackingStoreType::NSBackingStoreBuffered,
            false,
        )
    };

    unsafe {
        window.setOpaque(false);
        window.setBackgroundColor(Some(&NSColor::clearColor()));
        window.setIgnoresMouseEvents(true);
        window.setLevel(objc2_app_kit::NSScreenSaverWindowLevel);
        window.setCollectionBehavior(
            objc2_app_kit::NSWindowCollectionBehavior::CanJoinAllSpaces
                | objc2_app_kit::NSWindowCollectionBehavior::Stationary,
        );
    }

    let view: Retained<OverlayView> = unsafe {
        msg_send_id![
            objc2::msg_send_id![OverlayView::class(), alloc],
            initWithFrame: screen_frame
        ]
    };

    unsafe {
        window.setContentView(Some(&view));
        window.orderFrontRegardless();
        VIEW_PTR = Retained::into_raw(view) as *mut AnyObject;
        let _ = Retained::into_raw(window);
    }

    log::info!(
        "overlay window created  {}×{}",
        screen_frame.size.width,
        screen_frame.size.height
    );
}

/// Show the macro grid. Pass `macro_first` to highlight the selected key.
pub fn show_grid_a(macro_first: Option<char>) {
    set_state(OverlayState::GridA { macro_first });
    redraw();
}

/// Show the subcell layer: dim bg + selected cell highlight + ortho subcell grid.
/// `x`, `y`, `w`, `h` are the selected cell bounds in screen coords (y from top).
pub fn show_subcell(x: f64, y: f64, w: f64, h: f64) {
    set_state(OverlayState::Subcell { x, y, w, h });
    redraw();
}

/// Show the scroll-mode entry HUD (key hint banner, no specific key pressed yet).
pub fn show_scroll_mode() {
    set_state(OverlayState::ScrollHud { key: String::new(), action: String::new() });
    redraw();
}

/// Update the scroll HUD with the last key pressed and its action label.
pub fn show_scroll_hud(key: &str, action: &str) {
    set_state(OverlayState::ScrollHud {
        key:    key.to_owned(),
        action: action.to_owned(),
    });
    redraw();
}

/// Hide the overlay (Idle).
pub fn hide() {
    set_state(OverlayState::Hidden);
    redraw();
}

fn redraw() {
    unsafe {
        if VIEW_PTR.is_null() {
            return;
        }
        let _: () = msg_send![&*VIEW_PTR, setNeedsDisplay: true];
    }
}
