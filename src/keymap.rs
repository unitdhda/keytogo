/// macOS virtual keycode → printable char (ANSI QWERTY layout).
pub fn keycode_to_char(kc: u16) -> Option<char> {
    Some(match kc {
        0x00 => 'a', 0x01 => 's', 0x02 => 'd', 0x03 => 'f',
        0x04 => 'h', 0x05 => 'g', 0x06 => 'z', 0x07 => 'x',
        0x08 => 'c', 0x09 => 'v', 0x0B => 'b', 0x0C => 'q',
        0x0D => 'w', 0x0E => 'e', 0x0F => 'r', 0x10 => 'y',
        0x11 => 't', 0x12 => '1', 0x13 => '2', 0x14 => '3',
        0x15 => '4', 0x16 => '6', 0x17 => '5', 0x18 => '=',
        0x19 => '9', 0x1A => '7', 0x1B => '-', 0x1C => '8',
        0x1D => '0', 0x1E => ']', 0x1F => 'o', 0x20 => 'u',
        0x21 => '[', 0x22 => 'i', 0x23 => 'p', 0x25 => 'l',
        0x26 => 'j', 0x27 => '\'', 0x28 => 'k', 0x29 => ';',
        0x2A => '\\', 0x2B => ',', 0x2C => '/', 0x2D => 'n',
        0x2E => 'm', 0x2F => '.', 0x31 => ' ', 0x32 => '`',
        _ => return None,
    })
}

/// Named keys for log readability.
pub fn keycode_name(kc: u16) -> &'static str {
    match kc {
        0x24 => "Return",
        0x30 => "Tab",
        0x31 => "Space",
        0x33 => "Delete",
        0x35 => "Escape",
        0x37 => "LCmd",  0x36 => "RCmd",
        0x38 => "LShift", 0x3C => "RShift",
        0x3A => "LOpt",  0x3D => "ROpt",
        0x3B => "LCtrl", 0x3E => "RCtrl",
        0x39 => "CapsLock",
        0x7B => "Left",  0x7C => "Right",
        0x7D => "Down",  0x7E => "Up",
        0x75 => "FwdDelete",
        _ => "?",
    }
}

// ── Subcell layout (ortholinear 6×3) ──────────────────────────────────────

/// Subcell keys with (key, col, row) — ortholinear grid, no stagger.
/// col 0..5 left→right, row 0..2 top→bottom.
///
///   ┌─e─┬─r─┬─t─┬─y─┬─u─┬─i─┐   row 0
///   ├─d─┼─f─┼─g─┼─h─┼─j─┼─k─┤   row 1
///   └─x─┴─c─┴─v─┴─b─┴─n─┴─m─┘   row 2
pub const SUBCELL_KEYS: &[(char, u8, u8)] = &[
    ('e', 0, 0), ('r', 1, 0), ('t', 2, 0), ('y', 3, 0), ('u', 4, 0), ('i', 5, 0),
    ('d', 0, 1), ('f', 1, 1), ('g', 2, 1), ('h', 3, 1), ('j', 4, 1), ('k', 5, 1),
    ('x', 0, 2), ('c', 1, 2), ('v', 2, 2), ('b', 3, 2), ('n', 4, 2), ('m', 5, 2),
];

pub const SUBCELL_COLS: usize = 6;
pub const SUBCELL_ROWS: usize = 3;

// ── Layout A — 4×6 macro (upper+lower) + 7×3 sub ─────────────────────────

/// Layout A upper half: 4 cols × 3 rows, maps to top 50% of screen.
///
///   screen:  q  w  e  r   (row 0)
///            a  s  d  f   (row 1)
///            z  x  c  v   (row 2)
pub const LAYOUT_A_UPPER_KEYS: [[char; 4]; 3] = [
    ['q', 'w', 'e', 'r'],
    ['a', 's', 'd', 'f'],
    ['z', 'x', 'c', 'v'],
];

/// Layout A lower half: 4 cols × 3 rows, maps to bottom 50% of screen.
///
///   screen:  y  u  i  o   (row 3)
///            h  j  k  l   (row 4)
///            n  m  ,  .   (row 5)
pub const LAYOUT_A_LOWER_KEYS: [[char; 4]; 3] = [
    ['y', 'u', 'i', 'o'],
    ['h', 'j', 'k', 'l'],
    ['n', 'm', ',', '.'],
];

/// Layout A sub-grid: 7 cols × 3 rows (spans both hands, home cluster).
///
///   screen within macro cell:  e  r  t  y  u  i  o   (row 0)
///                              d  f  g  h  j  k  l   (row 1)
///                              c  v  b  n  m  ,  .   (row 2)
pub const LAYOUT_A_SUB_KEYS: [[char; 7]; 3] = [
    ['e', 'r', 't', 'y', 'u', 'i', 'o'],
    ['d', 'f', 'g', 'h', 'j', 'k', 'l'],
    ['c', 'v', 'b', 'n', 'm', ',', '.'],
];

pub const LAYOUT_A_MACRO_COLS: usize = 4;
pub const LAYOUT_A_MACRO_ROWS: usize = 3;
pub const LAYOUT_A_TOTAL_ROWS: usize = 6;
pub const LAYOUT_A_SUB_COLS: usize = 7;
pub const LAYOUT_A_SUB_ROWS: usize = 3;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn home_row_keycodes() {
        assert_eq!(keycode_to_char(0x00), Some('a'));
        assert_eq!(keycode_to_char(0x01), Some('s'));
        assert_eq!(keycode_to_char(0x02), Some('d'));
        assert_eq!(keycode_to_char(0x03), Some('f'));
    }

    #[test]
    fn top_row_keycodes() {
        assert_eq!(keycode_to_char(0x0C), Some('q'));
        assert_eq!(keycode_to_char(0x0D), Some('w'));
        assert_eq!(keycode_to_char(0x0E), Some('e'));
        assert_eq!(keycode_to_char(0x0F), Some('r'));
    }

    #[test]
    fn space_keycode() {
        assert_eq!(keycode_to_char(0x31), Some(' '));
    }

    #[test]
    fn non_printable_keycodes_return_none() {
        assert_eq!(keycode_to_char(0x24), None);
        assert_eq!(keycode_to_char(0x35), None);
        assert_eq!(keycode_to_char(0xFF), None);
    }

    #[test]
    fn keycode_names() {
        assert_eq!(keycode_name(0x24), "Return");
        assert_eq!(keycode_name(0x35), "Escape");
        assert_eq!(keycode_name(0x36), "RCmd");
        assert_eq!(keycode_name(0xFF), "?");
    }

    // ── SUBCELL_KEYS tests ────────────────────────────────────────────────

    #[test]
    fn subcell_keys_count() {
        assert_eq!(SUBCELL_KEYS.len(), 18);
    }

    #[test]
    fn subcell_keys_top_row() {
        let e = SUBCELL_KEYS.iter().find(|&&(k, _, _)| k == 'e').unwrap();
        assert_eq!((e.1, e.2), (0, 0));
        let i = SUBCELL_KEYS.iter().find(|&&(k, _, _)| k == 'i').unwrap();
        assert_eq!((i.1, i.2), (5, 0));
    }

    #[test]
    fn subcell_keys_home_row() {
        let d = SUBCELL_KEYS.iter().find(|&&(k, _, _)| k == 'd').unwrap();
        assert_eq!((d.1, d.2), (0, 1));
        let k_key = SUBCELL_KEYS.iter().find(|&&(k, _, _)| k == 'k').unwrap();
        assert_eq!((k_key.1, k_key.2), (5, 1));
    }

    #[test]
    fn subcell_keys_bottom_row() {
        let x = SUBCELL_KEYS.iter().find(|&&(k, _, _)| k == 'x').unwrap();
        assert_eq!((x.1, x.2), (0, 2));
        let m = SUBCELL_KEYS.iter().find(|&&(k, _, _)| k == 'm').unwrap();
        assert_eq!((m.1, m.2), (5, 2));
    }

    #[test]
    fn subcell_cols_rows_consistent() {
        let max_col = SUBCELL_KEYS.iter().map(|&(_, c, _)| c as usize).max().unwrap();
        let max_row = SUBCELL_KEYS.iter().map(|&(_, _, r)| r as usize).max().unwrap();
        assert_eq!(max_col, SUBCELL_COLS - 1);
        assert_eq!(max_row, SUBCELL_ROWS - 1);
    }

    // ── LAYOUT_A tests ────────────────────────────────────────────────────

    #[test]
    fn layout_a_upper_corners() {
        assert_eq!(LAYOUT_A_UPPER_KEYS[0][0], 'q'); // top-left
        assert_eq!(LAYOUT_A_UPPER_KEYS[0][3], 'r'); // top-right
        assert_eq!(LAYOUT_A_UPPER_KEYS[2][0], 'z'); // bottom-left
        assert_eq!(LAYOUT_A_UPPER_KEYS[2][3], 'v'); // bottom-right
    }

    #[test]
    fn layout_a_lower_corners() {
        assert_eq!(LAYOUT_A_LOWER_KEYS[0][0], 'y'); // top-left
        assert_eq!(LAYOUT_A_LOWER_KEYS[0][3], 'o'); // top-right
        assert_eq!(LAYOUT_A_LOWER_KEYS[2][0], 'n'); // bottom-left
        assert_eq!(LAYOUT_A_LOWER_KEYS[2][3], '.'); // bottom-right
    }

    #[test]
    fn layout_a_sub_corners() {
        assert_eq!(LAYOUT_A_SUB_KEYS[0][0], 'e'); // top-left
        assert_eq!(LAYOUT_A_SUB_KEYS[0][6], 'o'); // top-right
        assert_eq!(LAYOUT_A_SUB_KEYS[2][0], 'c'); // bottom-left
        assert_eq!(LAYOUT_A_SUB_KEYS[2][6], '.'); // bottom-right
    }

    #[test]
    fn layout_a_sub_count() {
        let count = LAYOUT_A_SUB_ROWS * LAYOUT_A_SUB_COLS;
        assert_eq!(count, 21);
    }

    #[test]
    fn layout_a_total_rows() {
        assert_eq!(LAYOUT_A_TOTAL_ROWS, LAYOUT_A_MACRO_ROWS * 2);
    }

    // ── Keybind conflict: no duplicate keys within a layout ───────────────

    #[test]
    fn subcell_keys_no_duplicates() {
        let mut seen = std::collections::HashSet::new();
        for &(k, _, _) in SUBCELL_KEYS {
            assert!(seen.insert(k), "duplicate subcell key: '{k}'");
        }
    }

    #[test]
    fn layout_a_macro_keys_no_duplicates() {
        let mut seen = std::collections::HashSet::new();
        for row in LAYOUT_A_UPPER_KEYS.iter().chain(LAYOUT_A_LOWER_KEYS.iter()) {
            for &k in row {
                assert!(seen.insert(k), "duplicate macro key: '{k}'");
            }
        }
    }

    #[test]
    fn layout_a_sub_keys_no_duplicates() {
        let mut seen = std::collections::HashSet::new();
        for row in &LAYOUT_A_SUB_KEYS {
            for &k in row {
                assert!(seen.insert(k), "duplicate sub key: '{k}'");
            }
        }
    }

    #[test]
    fn subcell_keys_each_position_is_unique() {
        let mut positions = std::collections::HashSet::new();
        for &(_, col, row) in SUBCELL_KEYS {
            assert!(positions.insert((col, row)), "duplicate subcell position ({col},{row})");
        }
    }

    #[test]
    fn layout_a_macro_and_sub_keys_are_disambiguated_by_state() {
        // Macro keys and sub keys intentionally share chars (e.g. 'e' is both a macro and a sub
        // key). There is no within-state conflict because macro_first==None routes to the macro
        // table and macro_first==Some routes to the sub table.  This test documents the overlap
        // is expected and safe.
        let macro_keys: std::collections::HashSet<char> = LAYOUT_A_UPPER_KEYS
            .iter()
            .chain(LAYOUT_A_LOWER_KEYS.iter())
            .flat_map(|row| row.iter().copied())
            .collect();
        let sub_keys: std::collections::HashSet<char> = LAYOUT_A_SUB_KEYS
            .iter()
            .flat_map(|row| row.iter().copied())
            .collect();
        let overlap_count = macro_keys.intersection(&sub_keys).count();
        // Overlap is expected; the important invariant is that it is never zero
        // (which would suggest the layout tables diverged from the UX design).
        assert!(
            overlap_count > 0,
            "expected macro/sub key overlap for shared home-cluster keys"
        );
    }

    #[test]
    fn tab_keycode_not_a_printable_char() {
        // Tab (0x30) enters scroll mode; it has no keycode_to_char mapping, so it can never
        // accidentally match a label char or a subcell key.
        assert_eq!(keycode_to_char(0x30), None, "Tab must not map to a printable char");
    }
}
