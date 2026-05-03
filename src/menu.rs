use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicPtr, Ordering};

use objc2::rc::Retained;
use objc2::runtime::AnyObject;
use objc2::{declare_class, msg_send_id, mutability, sel, ClassType, DeclaredClass};
use objc2_app_kit::{
    NSApplication, NSControlStateValueOff, NSControlStateValueOn, NSImage, NSMenu, NSMenuItem,
    NSStatusBar, NSVariableStatusItemLength,
};
use objc2_foundation::{ns_string, MainThreadMarker, NSData, NSSize, NSString};

use crate::{event_tap, overlay};

static DINGIR_SVG: &[u8] = include_bytes!("../assets/dingir.svg");

// ── Shared state ───────────────────────────────────────────────────────────

/// True when the event tap is paused via the menu.
static PAUSED: AtomicBool = AtomicBool::new(false);

/// Raw pointer to the Pause/Resume menu item — updated in install(), written
/// only on the main thread from the ObjC action callback.
static PAUSE_ITEM: AtomicPtr<NSMenuItem> = AtomicPtr::new(std::ptr::null_mut());

/// Raw pointer to the Launch at Login menu item.
static LAUNCH_ITEM: AtomicPtr<NSMenuItem> = AtomicPtr::new(std::ptr::null_mut());

// ── Launch-agent helpers ───────────────────────────────────────────────────

fn launch_agents_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_default();
    PathBuf::from(home).join("Library").join("LaunchAgents")
}

fn plist_path() -> PathBuf {
    launch_agents_dir().join("com.keytogo.plist")
}

pub fn is_launch_agent_installed() -> bool {
    plist_path().exists()
}

pub fn install_launch_agent() -> std::io::Result<()> {
    let exe = std::env::current_exe()?;
    let content = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN"
  "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.keytogo</string>
    <key>ProgramArguments</key>
    <array>
        <string>{}</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>StandardOutPath</key>
    <string>/tmp/keytogo.log</string>
    <key>StandardErrorPath</key>
    <string>/tmp/keytogo.log</string>
</dict>
</plist>
"#,
        exe.display()
    );
    std::fs::create_dir_all(launch_agents_dir())?;
    std::fs::write(plist_path(), &content)?;
    let _ = std::process::Command::new("launchctl")
        .args(["load", plist_path().to_str().unwrap_or("")])
        .status();
    log::info!("launch agent installed at {}", plist_path().display());
    Ok(())
}

pub fn uninstall_launch_agent() -> std::io::Result<()> {
    let path = plist_path();
    if path.exists() {
        let _ = std::process::Command::new("launchctl")
            .args(["unload", path.to_str().unwrap_or("")])
            .status();
        std::fs::remove_file(&path)?;
        log::info!("launch agent removed");
    }
    Ok(())
}

// ── ObjC MenuController ────────────────────────────────────────────────────

declare_class!(
    pub struct MenuController;

    unsafe impl ClassType for MenuController {
        type Super = objc2_foundation::NSObject;
        type Mutability = mutability::MainThreadOnly;
        const NAME: &'static str = "KeytogoMenuController";
    }

    impl DeclaredClass for MenuController {
        type Ivars = ();
    }

    unsafe impl MenuController {
        #[method(togglePause:)]
        fn toggle_pause(&self, _sender: &AnyObject) {
            let now_paused = !PAUSED.load(Ordering::Relaxed);
            PAUSED.store(now_paused, Ordering::Relaxed);
            if now_paused {
                event_tap::pause();
                overlay::hide();
                log::info!("keytogo paused via menu");
            } else {
                event_tap::resume();
                log::info!("keytogo resumed via menu");
            }
            let ptr = PAUSE_ITEM.load(Ordering::Relaxed);
            if !ptr.is_null() {
                unsafe {
                    (&*ptr).setTitle(if now_paused {
                        ns_string!("Resume")
                    } else {
                        ns_string!("Pause")
                    });
                }
            }
        }

        #[method(toggleLaunchAtLogin:)]
        fn toggle_launch_at_login(&self, _sender: &AnyObject) {
            if is_launch_agent_installed() {
                match uninstall_launch_agent() {
                    Ok(()) => set_launch_item_state(false),
                    Err(e) => log::error!("uninstall launch agent failed: {e}"),
                }
            } else {
                match install_launch_agent() {
                    Ok(()) => set_launch_item_state(true),
                    Err(e) => log::error!("install launch agent failed: {e}"),
                }
            }
        }

        #[method(quit:)]
        fn quit(&self, _sender: &AnyObject) {
            unsafe {
                let mtm = MainThreadMarker::new_unchecked();
                NSApplication::sharedApplication(mtm).terminate(None);
            }
        }
    }
);

impl MenuController {
    fn new(_mtm: MainThreadMarker) -> Retained<Self> {
        // No ivars — alloc + init directly via msg_send_id (avoids IsAllocableAnyThread constraint).
        unsafe { msg_send_id![msg_send_id![Self::class(), alloc], init] }
    }
}

fn set_launch_item_state(installed: bool) {
    let ptr = LAUNCH_ITEM.load(Ordering::Relaxed);
    if !ptr.is_null() {
        unsafe {
            (&*ptr).setState(if installed {
                NSControlStateValueOn
            } else {
                NSControlStateValueOff
            });
        }
    }
}

fn make_item(
    title: &NSString,
    action: objc2::runtime::Sel,
    key_eq: &NSString,
    target: &AnyObject,
) -> Retained<NSMenuItem> {
    unsafe {
        // MainThreadOnly: alloc via msg_send_id rather than NSMenuItem::alloc(mtm).
        let alloc: objc2::rc::Allocated<NSMenuItem> =
            msg_send_id![NSMenuItem::class(), alloc];
        let item = NSMenuItem::initWithTitle_action_keyEquivalent(
            alloc,
            title,
            Some(action),
            key_eq,
        );
        item.setTarget(Some(target));
        item
    }
}

// ── Public API ─────────────────────────────────────────────────────────────

/// Creates the menu bar status item with Pause/Resume, Launch at Login, Quit.
/// Must be called on the main thread after `overlay::init`.
pub fn install(mtm: MainThreadMarker) {
    let controller = MenuController::new(mtm);
    // Leak the controller — it must live for the entire app lifetime.
    let controller_raw = Retained::into_raw(controller);
    let controller_any = unsafe { &*(controller_raw as *const AnyObject) };

    let status_bar = unsafe { NSStatusBar::systemStatusBar() };
    let status_item = unsafe { status_bar.statusItemWithLength(NSVariableStatusItemLength) };

    if let Some(btn) = unsafe { status_item.button(mtm) } {
        unsafe {
            let data = NSData::dataWithBytes_length(
                DINGIR_SVG.as_ptr() as *mut std::ffi::c_void,
                DINGIR_SVG.len(),
            );
            if let Some(img) = NSImage::initWithData(NSImage::alloc(), &data) {
                img.setSize(NSSize { width: 18.0, height: 18.0 });
                img.setTemplate(true);
                btn.setImage(Some(&img));
            } else {
                btn.setTitle(ns_string!("𒀭"));
            }
        }
    }

    let menu = NSMenu::new(mtm);

    // — Pause / Resume —
    let pause_item = make_item(
        ns_string!("Pause"),
        sel!(togglePause:),
        ns_string!(""),
        controller_any,
    );
    menu.addItem(&pause_item);
    PAUSE_ITEM.store(Retained::into_raw(pause_item) as *mut NSMenuItem, Ordering::Relaxed);

    menu.addItem(&NSMenuItem::separatorItem(mtm));

    // — Launch at Login —
    let launch_item = make_item(
        ns_string!("Launch at Login"),
        sel!(toggleLaunchAtLogin:),
        ns_string!(""),
        controller_any,
    );
    if is_launch_agent_installed() {
        unsafe { launch_item.setState(NSControlStateValueOn) };
    }
    menu.addItem(&launch_item);
    LAUNCH_ITEM.store(
        Retained::into_raw(launch_item) as *mut NSMenuItem,
        Ordering::Relaxed,
    );

    menu.addItem(&NSMenuItem::separatorItem(mtm));

    // — Quit —
    let quit_item = make_item(
        ns_string!("Quit keytogo"),
        sel!(quit:),
        ns_string!("q"),
        controller_any,
    );
    menu.addItem(&quit_item);

    unsafe { status_item.setMenu(Some(&menu)) };

    // Leak to keep alive for the lifetime of the app.
    let _ = Retained::into_raw(status_item);
    let _ = Retained::into_raw(menu);
}

/// Install or uninstall the launch agent (called from CLI flags in main.rs).
pub fn cli_install_service() {
    match install_launch_agent() {
        Ok(()) => println!("keytogo launch agent installed. It will start on next login."),
        Err(e) => eprintln!("failed to install launch agent: {e}"),
    }
}

pub fn cli_uninstall_service() {
    match uninstall_launch_agent() {
        Ok(()) => println!("keytogo launch agent removed."),
        Err(e) => eprintln!("failed to remove launch agent: {e}"),
    }
}

pub fn cli_init_config(force: bool) {
    let home = std::env::var("HOME").unwrap_or_default();
    let dir  = format!("{home}/.config/keytogo");
    let path = format!("{dir}/config.toml");

    if !force && std::path::Path::new(&path).exists() {
        eprintln!("config already exists at {path}  (use --force to overwrite)");
        std::process::exit(1);
    }

    let content = r##"# keytogo configuration
# Run `keytogo --init-config --force` to regenerate this file with defaults.

[layout]
# Stage 1 — selects which screen region. Each line = one keyboard row.
# Spaces within a line are ignored (use them for visual alignment).
macro_keys = """
qwer
asdf
zxcv
yuio
hjkl
nm,.
"""

# Stage 2 — selects a sub-cell within the chosen region.
sub_keys = """
ertyuio
dfghjkl
cvbnm,.
"""

# Stage 3 — fine-positions the cursor inside the selected sub-cell.
subcell_keys = """
ertyui
dfghjk
xcvbnm
"""

[subcell]
# Max milliseconds between taps to count as double/triple click.
tap_window_ms = 250

[keybinds]
# Modifier that selects right-click: "shift" | "ctrl" | "alt"
right_click_modifier = "shift"
# Modifier that selects middle-click: "shift" | "ctrl" | "alt"
middle_click_modifier = "ctrl"

[scroll]
line_px = 60
half_page_lines = 10

[style]
overlay_bg  = "#00000088"
cell_border = "#ffffff33"
label_color = "#ffffffff"
active_cell = "#ffff0055"
subcell_dot = "#00ff88cc"

[hud]
# Where to anchor the scroll HUD pill.
# Values: "bottom-center" | "bottom-left" | "bottom-right"
#         | "top-center"  | "top-left"    | "top-right"
position = "bottom-center"
margin_x = 0.0
margin_y = 64.0
"##;

    if let Err(e) = std::fs::create_dir_all(&dir) {
        eprintln!("failed to create config dir {dir}: {e}");
        std::process::exit(1);
    }
    if let Err(e) = std::fs::write(&path, content) {
        eprintln!("failed to write config to {path}: {e}");
        std::process::exit(1);
    }
    println!("config written to {path}");
}
