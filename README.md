# keytogo

A Rust-based macOS keyboard-only mouse navigation utility. Control the mouse entirely from your keyboard without touching the trackpad or mouse. Inspired by warpd and mouseless.

**Status:** Active development | **macOS 13+** | **Requires Accessibility permissions**

## Installation

### From Source

Install directly from the repository:

```sh
cargo install --git https://github.com/unitf90/keytogo
```

### Grant Accessibility Permissions

The application requires accessibility permissions to control the mouse and detect keyboard input:

1. Open System Settings
2. Navigate to **Privacy & Security → Accessibility**
3. Click the lock icon and authenticate
4. Add or enable `keytogo` in the application list

### Install as Daemon (Recommended)

Install the daemon so keytogo runs automatically at login:

```sh
keytogo --install-service
```

Or run once without installing:

```sh
keytogo
```

## Usage

### Activation

Press the activation chord to show the grid overlay:

```
Right-Cmd + Left-Cmd + Space
```

This displays a full-screen grid overlay. Press Escape to hide it without selecting.

### Layout Modes

keytogo supports two layout modes (configurable in `~/.config/keytogo/config.toml`):

#### Layout A (Default)

A two-stage selection process:

1. **Grid stage**: A grid of labeled cells appears
2. **SubcellMode stage**: After selecting a cell, a 7×3 grid appears to pick the exact sub-position within that cell

Press any labeled key to select a cell, then use the SubcellMode grid to refine the position.

#### Grid Layout

A flat two-character label grid for direct single-stage selection. Each cell has a two-letter label; press both keys in sequence to select.

### SubcellMode Reference

After selecting a cell in Layout A, refine the position using this grid:

```
e  r  t  y  u  i  o
d  f  g  h  j  k  l
c  v  b  n  m  ,  .
```

Each key corresponds to a position within the selected cell.

### Click Actions

The modifier keys control how the selection is processed:

| Input | Action |
|-------|--------|
| Plain key | Left click at selected position |
| Shift | Right click at selected position |
| Ctrl | Middle click at selected position |
| Option | Move cursor only (no click) |
| Option + Shift | Drag from activation point to selected position |

### ScrollMode

From any grid overlay, press **Tab** to enter ScrollMode:

| Key | Action |
|-----|--------|
| j | Scroll down |
| k | Scroll up |
| h | Scroll left |
| l | Scroll right |
| d | Scroll half-page down |
| u | Scroll half-page up |
| Escape | Exit ScrollMode |

### Multi-click

Press the same key multiple times within a 250ms window to trigger:

- 2nd press → double click
- 3rd press → triple click

### Drag Mode

To drag from a location:

1. Navigate to the starting position
2. Press Option + Shift + the final position key

The cursor moves to your activation point, then drags to the selected position.

## Menu Bar

The application includes a menu bar icon with the following options:

- **Pause/Resume** — Temporarily disable keytogo while keeping the daemon running
- **Launch at Login** — Toggle automatic startup with your system
- **Quit** — Exit keytogo

## Configuration

All configuration is optional. Create a config file at:

```
~/.config/keytogo/config.toml
```

For detailed configuration options, defaults, and examples, see [docs/config.md](./docs/config.md).

Common configuration options include:

- Grid layout mode (`grid_a` or `grid`)
- Keybinds and chords
- Colors and UI scaling
- Scroll behavior
- Multi-click timing

## Keybind Reference

| Action | Default |
|--------|---------|
| Show grid | Right-Cmd + Left-Cmd + Space |
| Hide grid / Cancel | Escape |
| Enter ScrollMode | Tab |
| Exit ScrollMode | Escape |
| Double/Triple click | Repeat key within 250ms |
| Drag mode | Option + Shift + target |

## Requirements

- **macOS 13** or later
- **Rust toolchain** (for building from source)
- Accessibility permissions (see Installation)

## Documentation

For more detailed information, see:

- [Configuration](./docs/config.md) — All config keys and examples
- [Testing](./docs/testing.md) — Test coverage and CLI flags

## License

See LICENSE file in the repository.
