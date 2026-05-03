# keytogo Configuration

Configuration lives at `~/.config/keytogo/config.toml`. All sections and keys are optional — missing keys fall back to their defaults, so a partial config is fine.

## Setup

```sh
mkdir -p ~/.config/keytogo
touch ~/.config/keytogo/config.toml
```

---

## [grid]

Controls the flat **Grid** layout (active when `keybinds.default_layout = "grid"`). Sets the number of columns and rows the screen is divided into, and which characters label each cell.

| Key | Type | Default | Notes |
|---|---|---|---|
| `cols` | integer | `10` | any positive integer |
| `rows` | integer | `9` | any positive integer |
| `label_alphabet` | string | `"qwertyuiopasdfghjklzxcvbnm"` | length must be ≥ max(cols, rows) |

**Example:** smaller 8×6 grid with a home-row-first alphabet.

```toml
[grid]
cols = 8
rows = 6
label_alphabet = "asdfjkl;qweruiop"
```

---

## [subcell]

Controls timing for multi-tap gestures (double-click, triple-click) in SubcellMode.

| Key | Type | Default | Notes |
|---|---|---|---|
| `tap_window_ms` | integer | `250` | milliseconds between taps to count as multi-click |

**Example:** more forgiving double-click window.

```toml
[subcell]
tap_window_ms = 350
```

---

## [keybinds]

Controls modifier-to-button mapping in SubcellMode and which layout activates on startup.

| Key | Type | Default | Allowed values |
|---|---|---|---|
| `right_click_modifier` | string | `"shift"` | `"shift"`, `"ctrl"`, `"alt"` |
| `middle_click_modifier` | string | `"ctrl"` | `"shift"`, `"ctrl"`, `"alt"` |
| `default_layout` | string | `"grid_a"` | `"grid_a"`, `"grid"` |

`"grid_a"` — two-stage Layout A (macro key → sub key); recommended for speed.  
`"grid"` — flat grid addressed by a two-character label code.

**Example:** Option for right-click, start in flat grid mode.

```toml
[keybinds]
right_click_modifier = "alt"
default_layout = "grid"
```

---

## [scroll]

Controls scroll distances in **ScrollMode** (entered from any grid via `Tab`).

| Key | Type | Default | Notes |
|---|---|---|---|
| `line_px` | integer | `60` | pixels per line unit |
| `half_page_lines` | integer | `10` | half-page = this × line_px |

`j`/`k`/`h`/`l` scroll 3 lines. `Shift`+direction scrolls 9 lines. `d`/`u` scroll `half_page_lines` lines.

**Example:** coarser steps, larger half-page.

```toml
[scroll]
line_px = 80
half_page_lines = 15
```

`j` → 240 px per press; `d` → 1200 px.

---

## [style]

Overlay colors. All values are hex with alpha: `#RRGGBBAA`.

| Key | Default | What it colors |
|---|---|---|
| `overlay_bg` | `"#00000088"` | full-screen dim background |
| `cell_border` | `"#ffffff33"` | grid cell border lines |
| `label_color` | `"#ffffffff"` | cell label text |
| `active_cell` | `"#ffff0055"` | selected/highlighted cell fill |
| `subcell_dot` | `"#00ff88cc"` | sub-cell position indicator |

**Example:** lighter overlay, blue active-cell highlight.

```toml
[style]
overlay_bg  = "#00000055"
active_cell = "#0088ff66"
```

---

## [hud]

Position of the ScrollMode HUD pill (the compact indicator shown instead of the full grid).

| Key | Type | Default | Notes |
|---|---|---|---|
| `position` | string | `"bottom-center"` | see values below |
| `margin_x` | float | `0.0` | px from the left/right edge; ignored for `*-center` |
| `margin_y` | float | `64.0` | px from the top/bottom screen edge |

`position` values: `"bottom-center"`, `"bottom-left"`, `"bottom-right"`, `"top-center"`, `"top-left"`, `"top-right"`.

**Example:** bottom-right corner with margins.

```toml
[hud]
position = "bottom-right"
margin_x = 32.0
margin_y = 40.0
```

---

## Full annotated example

```toml
[grid]
cols = 10
rows = 9
label_alphabet = "qwertyuiopasdfghjklzxcvbnm"

[subcell]
tap_window_ms = 250

[keybinds]
right_click_modifier  = "shift"   # "shift" | "ctrl" | "alt"
middle_click_modifier = "ctrl"    # "shift" | "ctrl" | "alt"
default_layout        = "grid_a"  # "grid_a" | "grid"

[scroll]
line_px         = 60
half_page_lines = 10

[style]
overlay_bg  = "#00000088"
cell_border = "#ffffff33"
label_color = "#ffffffff"
active_cell = "#ffff0055"
subcell_dot = "#00ff88cc"

[hud]
position = "bottom-center"   # bottom-center | bottom-left | bottom-right
                             # top-center    | top-left    | top-right
margin_x = 0.0               # px from left/right edge (ignored for *-center)
margin_y = 64.0              # px from top/bottom edge
```
