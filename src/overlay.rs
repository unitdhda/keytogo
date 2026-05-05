/// Full-screen transparent overlay with multiple visual states:
///   GridA   — macro grid with optional macro-key highlight
///   Subcell — semi-transparent background + selected cell + ortholinear subcell grid
use std::cell::Cell;
use std::sync::atomic::{AtomicBool, AtomicPtr, Ordering};

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{declare_class, msg_send, msg_send_id, mutability, ClassType, DeclaredClass};
use objc2_app_kit::{
    NSBackingStoreType, NSColor, NSFontWeight, NSScreen, NSView, NSWindow, NSWindowStyleMask,
};
use objc2_foundation::{MainThreadMarker, NSPoint, NSRect, NSSize, NSString};

use crate::config::{self, HudConfig, StyleConfig};
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
    static STATE: Cell<OverlayState> = const { Cell::new(OverlayState::Hidden) };
}

fn set_state(s: OverlayState) {
    STATE.with(|c| c.set(s));
}

static INITIALISED: AtomicBool = AtomicBool::new(false);
static VIEW_PTR: AtomicPtr<AnyObject> = AtomicPtr::new(std::ptr::null_mut());

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
    let style = &config::get().style;

    let layouts = config::parsed_layouts();
    let macro_l = &layouts.macro_l;
    let sub_l = &layouts.sub_l;

    let g = layout_geom(
        sw,
        sh,
        macro_l.num_cols,
        macro_l.num_rows,
        sub_l.num_cols,
        sub_l.num_rows,
    );

    draw_dim_bg(bounds, style);

    for row in 0..macro_l.num_rows {
        for col in 0..macro_l.num_cols {
            let macro_key = macro_l.keys[row][col];
            // Grid fills screen edge to edge; NSView y increases upward.
            let mx = col as f64 * g.macro_w;
            let my = sh - (row + 1) as f64 * g.macro_h;
            let macro_rect = NSRect {
                origin: NSPoint { x: mx, y: my },
                size: NSSize {
                    width: g.macro_w,
                    height: g.macro_h,
                },
            };

            let is_selected = macro_first == Some(macro_key);
            if is_selected {
                fill_rect_color(macro_rect, &with_alpha(style.active_cell.as_str(), 0.18));
            }
            stroke_rect_color(macro_rect, &with_alpha(style.cell_border.as_str(), 0.90));

            let sub_alpha = if is_selected { 0.85 } else { 0.60 };
            for sr in 0..sub_l.num_rows {
                for sc in 0..sub_l.num_cols {
                    let sub_key = sub_l.keys[sr][sc];
                    let sx = mx + sc as f64 * g.cell_w;
                    // sr=0 is visual top → highest NSView y
                    let sy = my + (sub_l.num_rows - 1 - sr) as f64 * g.cell_h;
                    let sub_rect = NSRect {
                        origin: NSPoint { x: sx, y: sy },
                        size: NSSize {
                            width: g.cell_w,
                            height: g.cell_h,
                        },
                    };
                    stroke_rect_color(sub_rect, &with_alpha(style.cell_border.as_str(), sub_alpha * 0.5));
                    draw_cell_label(
                        &format!("{}{}", macro_key, sub_key),
                        sub_rect,
                        style.label_size,
                        sub_alpha,
                        style,
                    );
                }
            }
        }
    }
}

fn draw_subcell_layer(view: &OverlayView, cell_x: f64, cell_y: f64, cell_w: f64, cell_h: f64) {
    let bounds = view.bounds();
    let sh = bounds.size.height;
    let style = &config::get().style;
    let mut subgrid_style = style.clone();
    if let Some(font) = &style.subgrid_font {
        subgrid_style.font = font.clone();
    }
    if let Some(size) = style.subgrid_label_size {
        subgrid_style.label_size = size;
    }
    if let Some(weight) = &style.subgrid_label_weight {
        subgrid_style.label_weight = weight.clone();
    }
    if let Some(gravity) = &style.subgrid_label_gravity {
        subgrid_style.label_gravity = gravity.clone();
    }

    draw_dim_bg(bounds, style);

    // Highlight the selected cell.
    // cell_y is screen-y-from-top; convert to NSView y (from bottom).
    let nsview_y = sh - cell_y - cell_h;
    let cell_rect = NSRect {
        origin: NSPoint {
            x: cell_x,
            y: nsview_y,
        },
        size: NSSize {
            width: cell_w,
            height: cell_h,
        },
    };
    fill_rect_color(cell_rect, &with_alpha(style.active_cell.as_str(), 0.75));
    stroke_rect_color(cell_rect, &with_alpha(style.active_cell.as_str(), 1.0));

    let subcell_l = &config::parsed_layouts().subcell_l;
    let sc_cols = subcell_l.num_cols;
    let sc_rows = subcell_l.num_rows;

    // Subcell grid fills the entire selected cell — cells are proportional.
    let sc_w = cell_w / sc_cols as f64;
    let sc_h = cell_h / sc_rows as f64;

    for row in 0..sc_rows {
        for col in 0..sc_cols {
            let key = subcell_l.keys[row][col];
            let x = cell_x + col as f64 * sc_w;
            // row=0 is visual top → highest NSView y
            let y = nsview_y + (sc_rows - 1 - row) as f64 * sc_h;
            let sub_rect = NSRect {
                origin: NSPoint { x, y },
                size: NSSize {
                    width: sc_w,
                    height: sc_h,
                },
            };

            fill_rect_color(sub_rect, &with_alpha(style.subcell_dot.as_str(), 0.22));
            stroke_rect_color(sub_rect, &with_alpha(style.subcell_dot.as_str(), 0.78));
            draw_cell_label(
                &key.to_string(),
                sub_rect,
                subgrid_style.label_size,
                0.90,
                &subgrid_style,
            );
        }
    }
}

// ── Drawing primitives ─────────────────────────────────────────────────────

fn draw_dim_bg(bounds: NSRect, style: &StyleConfig) {
    unsafe {
        let bg = color_from_hex(&style.overlay_bg);
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

fn fill_rect_color(rect: NSRect, color: &Rgba) {
    fill_rect(rect, color.r, color.g, color.b, color.a);
}

fn stroke_rect(rect: NSRect, r: f64, g: f64, b: f64, a: f64) {
    unsafe {
        let color = NSColor::colorWithSRGBRed_green_blue_alpha(r, g, b, a);
        color.setStroke();
        let path = objc2_app_kit::NSBezierPath::bezierPathWithRect(rect);
        path.stroke();
    }
}

fn stroke_rect_color(rect: NSRect, color: &Rgba) {
    stroke_rect(rect, color.r, color.g, color.b, color.a);
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
    draw_label_styled(text, x, y, size, alpha, &config::get().style);
}

fn draw_cell_label(text: &str, rect: NSRect, size: f64, alpha: f64, style: &StyleConfig) {
    let (x, y) = label_origin(rect, text, size, &style.label_gravity);
    draw_label_styled(text, x, y, size, alpha, style);
}

fn draw_label_styled(text: &str, x: f64, y: f64, size: f64, alpha: f64, style: &StyleConfig) {
    // Shadow pass — dark offset provides contrast against any desktop background.
    draw_label_color(
        text,
        x + 0.8,
        y - 0.8,
        size,
        0.05,
        0.04,
        0.02,
        (alpha * 0.6).min(1.0),
        style,
    );

    let color = with_alpha(style.label_color.as_str(), alpha);
    draw_label_color(
        text,
        x,
        y,
        size,
        color.r,
        color.g,
        color.b,
        color.a,
        style,
    );
}

#[allow(clippy::too_many_arguments)]
fn draw_label_color(
    text: &str,
    x: f64,
    y: f64,
    size: f64,
    r: f64,
    g: f64,
    b: f64,
    a: f64,
    style: &StyleConfig,
) {
    unsafe {
        use objc2_app_kit::{NSFontAttributeName, NSForegroundColorAttributeName};

        let font = label_font(size, style);
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

fn label_font(size: f64, style: &StyleConfig) -> Retained<objc2_app_kit::NSFont> {
    unsafe {
        let name = style.font.trim();
        if name.is_empty() || matches_ignore_ascii_case(name, "monospace") || matches_ignore_ascii_case(name, "mono") {
            return objc2_app_kit::NSFont::monospacedSystemFontOfSize_weight(
                size,
                font_weight(&style.label_weight),
            );
        }
        if matches_ignore_ascii_case(name, "user-monospace") || matches_ignore_ascii_case(name, "user-mono") {
            if let Some(font) = objc2_app_kit::NSFont::userFixedPitchFontOfSize(size) {
                return font;
            }
        }

        let ns_name = NSString::from_str(name);
        objc2_app_kit::NSFont::fontWithName_size(&ns_name, size).unwrap_or_else(|| {
            objc2_app_kit::NSFont::monospacedSystemFontOfSize_weight(
                size,
                font_weight(&style.label_weight),
            )
        })
    }
}

fn font_weight(weight: &str) -> NSFontWeight {
    unsafe {
        match weight.trim().to_ascii_lowercase().as_str() {
            "ultralight" | "ultra-light" => objc2_app_kit::NSFontWeightUltraLight,
            "thin" => objc2_app_kit::NSFontWeightThin,
            "light" => objc2_app_kit::NSFontWeightLight,
            "medium" => objc2_app_kit::NSFontWeightMedium,
            "semibold" | "semi-bold" => objc2_app_kit::NSFontWeightSemibold,
            "bold" => objc2_app_kit::NSFontWeightBold,
            "heavy" => objc2_app_kit::NSFontWeightHeavy,
            "black" => objc2_app_kit::NSFontWeightBlack,
            _ => objc2_app_kit::NSFontWeightRegular,
        }
    }
}

fn label_origin(rect: NSRect, text: &str, size: f64, gravity: &str) -> (f64, f64) {
    let text_w = text_px_width(text, size);
    let text_h = size;
    let pad_x = (rect.size.width * 0.08).max(size * 0.35);
    let pad_y = (rect.size.height * 0.08).max(size * 0.30);
    let gravity = gravity.trim().to_ascii_uppercase();

    let x = match gravity.as_str() {
        "C" | "CENTER" | "N" | "S" => rect.origin.x + (rect.size.width - text_w) / 2.0,
        "E" | "NE" | "SE" => rect.origin.x + rect.size.width - text_w - pad_x,
        _ => rect.origin.x + pad_x,
    };
    let y = match gravity.as_str() {
        "C" | "CENTER" | "E" | "W" => rect.origin.y + (rect.size.height - text_h) / 2.0,
        "S" | "SE" | "SW" => rect.origin.y + pad_y,
        _ => rect.origin.y + rect.size.height - text_h - pad_y,
    };

    (x, y)
}

#[derive(Clone, Copy)]
struct Rgba {
    r: f64,
    g: f64,
    b: f64,
    a: f64,
}

fn color_from_hex(hex: &str) -> Retained<NSColor> {
    let c = parse_hex_color(hex);
    unsafe { NSColor::colorWithSRGBRed_green_blue_alpha(c.r, c.g, c.b, c.a) }
}

fn with_alpha(hex: &str, alpha_scale: f64) -> Rgba {
    let mut c = parse_hex_color(hex);
    c.a = (c.a * alpha_scale).clamp(0.0, 1.0);
    c
}

fn parse_hex_color(hex: &str) -> Rgba {
    let s = hex.trim().strip_prefix('#').unwrap_or(hex.trim());
    if s.len() != 8 {
        return Rgba {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 1.0,
        };
    }

    let byte = |range: std::ops::Range<usize>| u8::from_str_radix(&s[range], 16).ok();
    match (byte(0..2), byte(2..4), byte(4..6), byte(6..8)) {
        (Some(r), Some(g), Some(b), Some(a)) => Rgba {
            r: r as f64 / 255.0,
            g: g as f64 / 255.0,
            b: b as f64 / 255.0,
            a: a as f64 / 255.0,
        },
        _ => Rgba {
            r: 1.0,
            g: 1.0,
            b: 1.0,
            a: 1.0,
        },
    }
}

fn matches_ignore_ascii_case(value: &str, expected: &str) -> bool {
    value.eq_ignore_ascii_case(expected)
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
        "bottom-left" => (mx, my),
        "bottom-right" => (sw - hud_w - mx, my),
        "top-center" => ((sw - hud_w) / 2.0, sh - hud_h - my),
        "top-left" => (mx, sh - hud_h - my),
        "top-right" => (sw - hud_w - mx, sh - hud_h - my),
        _ => ((sw - hud_w) / 2.0, my), // "bottom-center" (default)
    };
    (hud_x, hud_y)
}

fn draw_scroll_hud(view: &OverlayView, key: &str, action: &str) {
    let bounds = view.bounds();
    let sw = bounds.size.width;
    let sh = bounds.size.height;
    let cfg = &config::get().hud;

    const HUD_H: f64 = 54.0;
    const CAP_PAD: f64 = 10.0;
    const CAP_H: f64 = 34.0;
    const KEY_PT: f64 = 14.0;
    const ACT_PT: f64 = 13.0;
    const HINT_PT: f64 = 10.5;

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

    let (hud_x, hud_y) = hud_position(cfg, sw, sh, hud_w, HUD_H);

    let hud_rect = NSRect {
        origin: NSPoint { x: hud_x, y: hud_y },
        size: NSSize {
            width: hud_w,
            height: HUD_H,
        },
    };
    fill_rounded_rect(hud_rect, 14.0, 0.0, 0.0, 0.0, 0.88);

    if key.is_empty() {
        let hint = "Scroll Mode  j↓ k↑ h← l→  ⇧=fast  Space=click";
        draw_label_alpha(
            hint,
            hud_x + 16.0,
            hud_y + (HUD_H - HINT_PT) / 2.0 - 2.0,
            HINT_PT,
            0.80,
        );
    } else {
        let cap_w = (text_px_width(key, KEY_PT) + 16.0).max(34.0);
        let cap_x = hud_x + CAP_PAD;
        let cap_y = hud_y + (HUD_H - CAP_H) / 2.0;
        let cap_rect = NSRect {
            origin: NSPoint { x: cap_x, y: cap_y },
            size: NSSize {
                width: cap_w,
                height: CAP_H,
            },
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
        VIEW_PTR.store(
            Retained::into_raw(view) as *mut AnyObject,
            Ordering::Release,
        );
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
    set_state(OverlayState::ScrollHud {
        key: String::new(),
        action: String::new(),
    });
    redraw();
}

/// Update the scroll HUD with the last key pressed and its action label.
pub fn show_scroll_hud(key: &str, action: &str) {
    set_state(OverlayState::ScrollHud {
        key: key.to_owned(),
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
    let ptr = VIEW_PTR.load(Ordering::Acquire);
    if ptr.is_null() {
        return;
    }
    unsafe {
        let _: () = msg_send![&*ptr, setNeedsDisplay: true];
    }
}
