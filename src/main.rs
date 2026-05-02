#![allow(dead_code)]

mod accessibility;
mod config;
mod event_tap;
mod keymap;
mod mouse;
mod state;

fn main() {
    env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("debug"),
    )
    .init();

    if !accessibility::is_trusted(true) {
        eprintln!(
            "keytogo needs Accessibility permission.\n\
             System Settings → Privacy & Security → Accessibility → enable keytogo.\n\
             Restart after granting permission."
        );
        std::process::exit(1);
    }

    log::info!("Accessibility permission granted.");

    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("--test-mouse") => mouse::smoke_test(),
        Some("--test-state") => test_state(),
        _ => {
            log::info!("Event tap starting — press any key to see keycodes. Ctrl-C to quit.");
            event_tap::run();
        }
    }
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
