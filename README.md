# keytogo

A Rust-based macOS keyboard-only mouse navigation utility. Control your cursor entirely from the keyboard — no trackpad, no mouse required. Inspired by warpd and mouseless.

https://github.com/user-attachments/assets/237cbb22-5714-4ad8-b69d-245ef4f5600e

**Status:** Active development | **macOS 13+** | **Requires Accessibility permission**

---

## Installation

### One-line script

Downloads and installs the latest pre-built binary for your architecture:

```sh
curl -fsSL https://raw.githubusercontent.com/unitdhda/keytogo/refs/heads/main/install.sh | sh
```

### Install with Cargo

Install the published crate directly from crates.io:

```sh
cargo install keytogo
```

### Download a release binary

Grab the latest archive from the [Releases page](https://github.com/unitf90/keytogo/releases):

| Architecture | File |
|---|---|
| Apple Silicon | `keytogo-aarch64-apple-darwin.tar.gz` |
| Intel | `keytogo-x86_64-apple-darwin.tar.gz` |

```sh
tar -xzf keytogo-*.tar.gz
sudo mv keytogo /usr/local/bin/
```

### Build from the latest Git revision

Requires the Rust toolchain (`rustup.rs`):

```sh
cargo install --git https://github.com/unitf90/keytogo
```

---

### Grant Accessibility permission

keytogo intercepts keyboard events system-wide via the macOS CGEventTap API, which requires explicit user consent:

1. Open **System Settings → Privacy & Security → Accessibility**
2. Click the lock and authenticate
3. Add or enable `keytogo` in the list

Without this permission the event tap cannot start. You will be prompted on first launch.

---

### Run at login (recommended)

Install a per-user LaunchAgent so keytogo starts automatically at login:

```sh
keytogo --install
```

To remove it:

```sh
keytogo --uninstall
```

Or run once in the foreground without installing as a daemon:

```sh
keytogo
```

---

## How navigation works

keytogo uses a **three-stage keyboard navigation model** to place the cursor anywhere on screen without moving your hands.

### Activate

Press the activation chord from any context:

```
Right-Cmd + Space
```

A full-screen overlay appears. Press **Escape** at any time to return to Idle.

### Stage 1 — Macro grid

The screen is divided into a grid of labeled cells, each identified by a single key. The layout is configurable (see [Configuration](#configuration)); the default covers the full screen with your keyboard home rows.

Press a key to select a macro cell and highlight it. The overlay updates to show which sub-cells are available inside it.

**Backspace** — return to Idle (dismiss overlay).

### Stage 2 — Sub-cell grid

After the macro key, press a second key to zoom into one of the sub-cells within the selected macro region. The cursor moves to the center of the selected sub-cell and the subcell precision grid appears.

**Backspace** — return to Stage 1 (clear macro selection, pick a different macro cell).  
**Escape** — return to Idle.

### Stage 3 — Subcell precision

A fine-grained grid fills the selected region. Press a key to place the cursor at that exact position and fire a click.

**Backspace** — return to Stage 2 (keep your macro key, re-pick the sub-cell).  
**Escape** — return to Stage 1.

---

### Click actions

Apply modifier keys at the final keypress to change what happens on landing:

| Modifier | Action |
|---|---|
| *(none)* | Left click |
| Shift | Right click |
| Ctrl | Middle click |
| Option | Move cursor only (no click) |
| Option + Shift | Drag from activation point to selected position |

### Multi-click

Press the same subcell key again within the click window (default 250 ms) to upgrade the click:

- 2nd press → double-click
- 3rd press → triple-click

---

## Scroll mode

Press **Tab** from the macro grid to enter Scroll mode. A HUD appears showing available keys.

| Key | Action |
|---|---|
| `j` | Scroll down |
| `k` | Scroll up |
| `h` | Scroll left |
| `l` | Scroll right |
| `d` | Half-page down |
| `u` | Half-page up |
| `Shift` + any direction | Fast scroll (3× speed) |
| `Space` | Hold left button (for drag-scrolling) |
| `Option + Space` | Hold right button |
| `Tab` | Return to grid |
| `Escape` | Exit to Idle |

---

## Keyboard reference

| Action | Key |
|---|---|
| Activate | Right-Cmd + Space |
| Back one stage | Backspace |
| Cancel to Idle | Escape |
| Enter Scroll mode | Tab (from grid overlay) |
| Exit Scroll mode | Escape or Tab |
| Double / triple click | Repeat subcell key within window |
| Drag to position | Option + Shift + subcell key |
| Move cursor only | Option + subcell key |

---

## Menu bar

The status bar icon provides quick access to:

- **Pause / Resume** — disable keytogo temporarily without stopping the daemon; all keys pass through normally while paused
- **Launch at Login** — toggle the LaunchAgent
- **Quit**

---

## Configuration

All configuration is optional — the defaults work out of the box. To generate a config file with all options pre-filled:

```sh
keytogo --init-config
```

This writes `~/.config/keytogo/config.toml`. Edit it, then restart keytogo (or run `keytogo --stop && keytogo --start`).

For the full reference see [docs/config.md](./docs/config.md).

### Layout

The three grids are fully configurable. Each is a multiline string where each line is a keyboard row; spaces are ignored and can be used for visual alignment. Row lengths must all match within a grid.

```toml
[layout]
# Stage 1 — macro grid (rows × cols inferred from the string)
macro_keys = """
qwer
asdf
zxcv
yuio
hjkl
nm,.
"""

# Stage 2 — sub-cell grid
# Omit this key entirely to let keytogo compute square sub-cells automatically
# based on your screen's aspect ratio.
sub_keys = """
ertyuio
dfghjkl
cvbnm,.
"""

# Stage 3 — precision subcell grid
subcell_keys = """
ertyui
dfghjk
xcvbnm
"""
```

### Scroll

```toml
[scroll]
line_px = 60           # pixels scrolled per j/k/h/l press
half_page_lines = 10   # lines for d/u half-page commands
```

### Scroll HUD position

```toml
[hud]
# Anchor point for the scroll mode HUD pill.
# bottom-center | bottom-left | bottom-right | top-center | top-left | top-right
position = "bottom-center"
margin_x = 0.0    # horizontal offset from the edge (ignored for *-center)
margin_y = 64.0   # vertical offset from the screen edge
```

### Multi-click timing

```toml
[subcell]
tap_window_ms = 250   # max ms between presses to count as double/triple click
```

---

## Requirements

- macOS 13 Ventura or later
- Accessibility permission (prompted on first launch)
- Rust toolchain (build from source only)

---

## Documentation

- [Configuration reference](./docs/config.md) — all config keys, types, defaults, examples
- [Testing](./docs/testing.md) — test coverage, CLI flags, running tests
