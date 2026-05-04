# Testing

## Unit Tests

62 tests across three modules. Run with:

```bash
cargo test
```

### src/state.rs (7 tests)
- `AppMode` default is `Idle`
- `AppState::new()` initialises with `mode=Idle`, `drag_origin=None`
- `Grid`, `GridA`, `Subcell` mode variants carry their payload fields correctly
- `ClickButton` equality / inequality
- `CellBounds::center_x` / `center_y` arithmetic
- `drag_origin` can be set and read

### src/keymap.rs (20 tests)
- `keycode_to_char`: home row, top row, Space (0x31), non-printable keys return `None`
- `keycode_name`: Return, Escape, RCmd, unknown
- `SUBCELL_KEYS`: count=18, top/home/bottom rows, all positions unique, no duplicate chars
- `LAYOUT_A_UPPER_KEYS` / `LAYOUT_A_LOWER_KEYS`: corner values, no duplicate macro keys
- `LAYOUT_A_SUB_KEYS`: corner values, count=21, no duplicate sub keys
- `LAYOUT_A_TOTAL_ROWS` = `LAYOUT_A_MACRO_ROWS * 2`
- Macro/sub key overlap is intentional and disambiguated by state
- Tab keycode (0x30) has no char mapping — confirms no alphabet conflict

### src/event_tap.rs (35 tests)
**Grid geometry**
- `label_to_cell`: first cell, last valid cell, col/row out of bounds, char not in alphabet
- `cell_bounds`: top-left and last-cell centre coordinates at 1920×1080

**Subcell**
- `subcell_pos`: top-left key, bottom-right key, unknown key → None

**Layout A helpers**
- `layout_a_macro_pos`: all four corners, unknown key → None
- `layout_a_sub_pos`: all four corners

**Event mask**
- Single type, multiple types, empty slice

**Modifier routing**
- No flags → default button
- Shift → Right, Ctrl → Middle, Shift+Ctrl → Right (Shift wins)
- `modifier_button_shift_wins_over_ctrl`
- `modifier_button_right_and_middle_never_both_true`

**Flag constants**
- `modifier_flag_constants_are_pairwise_disjoint`
- `activation_requires_both_cmd_flags` (partial combos don't activate)
- `FLAGS_OPTION` value and non-overlap with Shift/Ctrl
- `option_with_shift_not_same_as_option_alone`

**Subcell modifier branching**
- `subcell_modifier_branches_are_mutually_exclusive_and_exhaustive` — for every sample flags value, exactly one of {drag, move-only, click} fires

**Multi-click**
- `PendingTap` field access
- Count caps at 3

**Drag origin**
- Defaults to None, can be set

**Keybind conflicts**
- Tab keycode has no char mapping (no alphabet conflict)
- Scroll direction keys are all reachable via `keycode_to_char` (distinct from Tab entry mechanism)
- Scroll direction keys have no duplicates

## Manual Smoke Tests

### Activation
1. Launch app; grant Accessibility if prompted.
2. Press **RCmd + Space** → Layout A macrogrid overlay appears.
3. Press **Escape** → overlay hides, returns to Idle.

### Layout A click
1. Activate → macrogrid shown.
2. Press a macro key (e.g. `q`) → macro cell highlights.
3. Press a sub key (e.g. `e`) → SubcellMode shown for top-left area.
4. Press a subcell key (e.g. `f`) → left-click fires at that position → Idle.

### Multi-click
1. Activate → grid → subcell.
2. Press the same subcell key twice quickly (<250 ms) → double-click fires after timer.
3. Press three times → triple-click.

### Modifiers in SubcellMode
- Hold **Shift** while pressing subcell key → right-click.
- Hold **Ctrl** → middle-click.
- Hold **Option** (no Shift) → cursor moves to position, no click → Idle.
- Hold **Option+Shift** → drag from activation cursor pos to subcell pos.

### Scroll mode
1. Activate → macrogrid → press **Tab** → scroll HUD appears with hint banner.
2. Press `j` → scrolls down, HUD shows `[j] ↓`.
3. Hold **Shift** + `j` → faster scroll, HUD shows `[⇧j] ↓ fast`.
4. Press `Space` → left-click at cursor position.
5. Press **Escape** or **Tab** → returns to Idle, HUD hides.

### Standard Grid
1. Set `default_layout = "grid"` in config (or test by changing code temporarily).
2. Activate → 10×9 flat grid overlay appears.
3. Press a label char → matching column highlights.
4. Press second char → SubcellMode entered for that cell.
