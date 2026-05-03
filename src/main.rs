#![allow(dead_code)]

mod accessibility;
mod config;
mod event_tap;
mod keymap;
mod menu;
mod mouse;
mod overlay;
mod state;

use objc2_app_kit::{NSApplication, NSApplicationActivationPolicy};
use objc2_foundation::MainThreadMarker;

fn main() {
    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("debug"),
    )
    .init();

    // Service management and config init don't require Accessibility — handle before the check.
    let args: Vec<String> = std::env::args().collect();
    let has_force = args.iter().any(|a| a == "--force");
    match args.get(1).map(String::as_str) {
        Some("--install-service")   => { menu::cli_install_service();        return; }
        Some("--uninstall-service") => { menu::cli_uninstall_service();       return; }
        Some("--init-config")       => { menu::cli_init_config(has_force);    return; }
        _ => {}
    }

    if !accessibility::is_trusted(false) {
        // Open System Settings once to prompt the user, then poll silently.
        accessibility::is_trusted(true);
        eprintln!(
            "keytogo needs Accessibility permission.\n\
             Open System Settings → Privacy & Security → Accessibility,\n\
             click +, navigate to ~/.cargo/bin/, select keytogo, and toggle it ON."
        );
        while !accessibility::is_trusted(false) {
            std::thread::sleep(std::time::Duration::from_secs(2));
        }
    }

    log::info!("Accessibility permission granted.");

    match args.get(1).map(String::as_str) {
        Some("--test-mouse") => { mouse::smoke_test(); return; }
        Some("--test-state") => { test_state(); return; }
        _ => {}
    }

    // Safety: main() is the entry point and runs on the main thread.
    let mtm = unsafe { MainThreadMarker::new_unchecked() };
    let app = NSApplication::sharedApplication(mtm);
    app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);

    overlay::init(mtm);
    menu::install(mtm);
    event_tap::install();

    log::info!("event tap installed — press Ctrl+Alt+Space to activate grid");
    unsafe { app.run(); }
}

fn test_state() {
    println!("=== Phase 2 state machine test ===");
    println!();
    println!("Default grid: 13 cols × 9 rows");
    println!("Label alphabet: asdfjkl;ghqweruiopzxcvbnm");
    println!("  col index: a=0  s=1  d=2  f=3  j=4  k=5  l=6  ;=7  g=8  h=9  q=10 w=11 e=12");
    println!("  row index: a=0  s=1  d=2  f=3  j=4  k=5  l=6  ;=7  g=8");
    println!();
    println!("Activation chord:  Ctrl+Alt+Space");
    println!();
    println!("── GridMode ────────────────────────────────────────────────");
    println!("  <char1><char2>     warp cursor to cell center → SubcellMode");
    println!("  s                  → ScrollMode  (note: col 1 inaccessible)");
    println!("  d                  → DragMode    (note: col 2 inaccessible)");
    println!("  Escape             → Idle");
    println!();
    println!("── SubcellMode ─────────────────────────────────────────────");
    println!("  Space / Return     left-click at cell center → Idle");
    println!("  e r t y u i        click top-row subcell");
    println!("  d f g h j k        click home-row subcell");
    println!("  x c v b n m        click bottom-row subcell");
    println!("  Shift+key          right-click");
    println!("  Ctrl+key           middle-click");
    println!("  Escape             → GridMode (re-select)");
    println!();
    println!("── ScrollMode ──────────────────────────────────────────────");
    println!("  j/k                scroll down/up (3 lines)");
    println!("  h/l                scroll left/right (3 lines)");
    println!("  d/u                half-page down/up (10 lines)");
    println!("  Escape             → Idle");
    println!();
    println!("Running live tap — watch log output for state transitions.");
    println!("Ctrl-C to quit.\n");

    event_tap::run();
}
