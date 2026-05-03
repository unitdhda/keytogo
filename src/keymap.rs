// ── Parsed layout ──────────────────────────────────────────────────────────

/// A key grid parsed from a user-supplied multiline config string.
/// `keys[row][col]` — row 0 is the top visual row.
#[derive(Debug, Clone)]
pub struct ParsedLayout {
    pub keys:     Vec<Vec<char>>,
    pub num_cols: usize,
    pub num_rows: usize,
}

impl ParsedLayout {
    /// Return `(col, row)` of `ch` in this layout, or `None`.
    pub fn key_pos(&self, ch: char) -> Option<(usize, usize)> {
        self.keys.iter().enumerate().find_map(|(row, cols)| {
            cols.iter().position(|&k| k == ch).map(|col| (col, row))
        })
    }
}

/// Parse a multiline layout string into a `ParsedLayout`.
///
/// Rules:
/// - Whitespace (spaces, tabs) within a line is skipped — use it for visual alignment.
/// - Empty lines (after stripping whitespace) are skipped.
/// - All non-empty rows must have the same number of non-whitespace characters.
/// - Duplicate keys within the same layout are rejected.
pub fn parse_layout_string(s: &str) -> Result<ParsedLayout, String> {
    let rows: Vec<Vec<char>> = s
        .lines()
        .map(|line| line.chars().filter(|c| !c.is_whitespace()).collect::<Vec<char>>())
        .filter(|row| !row.is_empty())
        .collect();

    if rows.is_empty() {
        return Err("layout string is empty".into());
    }

    let num_cols = rows[0].len();
    for (i, row) in rows.iter().enumerate() {
        if row.len() != num_cols {
            return Err(format!(
                "row {i} has {} chars but row 0 has {num_cols}",
                row.len()
            ));
        }
    }

    let mut seen = std::collections::HashSet::new();
    for row in &rows {
        for &k in row {
            if !seen.insert(k) {
                return Err(format!("duplicate key '{k}'"));
            }
        }
    }

    Ok(ParsedLayout { num_rows: rows.len(), num_cols, keys: rows })
}

// ── Grid geometry ──────────────────────────────────────────────────────────

/// Screen geometry for the two-stage macro/sub layout.
/// Sub-cells are square; macro cells and full grid are derived from them.
pub struct LayoutGeom {
    /// Square sub-cell side length (px).
    pub sub_size: f64,
    /// Macro cell width  = sub_cols × sub_size.
    pub macro_w:  f64,
    /// Macro cell height = sub_rows × sub_size.
    pub macro_h:  f64,
    /// Total grid width  = macro_cols × macro_w.
    pub grid_w:   f64,
    /// Total grid height = macro_rows × macro_h.
    pub grid_h:   f64,
    /// Left margin that centers the grid on screen.
    pub offset_x: f64,
    /// Top margin that centers the grid (screen-y-from-top).
    pub offset_y: f64,
}

/// Compute layout geometry from screen and parsed layout dimensions.
/// `sub_size` is the largest square that fits the full
/// `(macro_cols × sub_cols) × (macro_rows × sub_rows)` grid on screen.
pub fn layout_geom(
    sw: f64, sh: f64,
    macro_cols: usize, macro_rows: usize,
    sub_cols:   usize, sub_rows:   usize,
) -> LayoutGeom {
    let total_cols = (macro_cols * sub_cols) as f64;
    let total_rows = (macro_rows * sub_rows) as f64;
    let sub_size   = (sw / total_cols).min(sh / total_rows);
    let macro_w    = sub_size * sub_cols as f64;
    let macro_h    = sub_size * sub_rows as f64;
    let grid_w     = macro_w * macro_cols as f64;
    let grid_h     = macro_h * macro_rows as f64;
    LayoutGeom {
        sub_size,
        macro_w,
        macro_h,
        grid_w,
        grid_h,
        offset_x: (sw - grid_w) / 2.0,
        offset_y: (sh - grid_h) / 2.0,
    }
}

// ── Keycode table ──────────────────────────────────────────────────────────

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

#[cfg(test)]
mod tests {
    use super::*;

    // ── parse_layout_string ───────────────────────────────────────────────

    #[test]
    fn parse_basic() {
        let p = parse_layout_string("qwer\nasdf\nzxcv").unwrap();
        assert_eq!(p.num_cols, 4);
        assert_eq!(p.num_rows, 3);
        assert_eq!(p.keys[0], &['q', 'w', 'e', 'r']);
        assert_eq!(p.keys[2], &['z', 'x', 'c', 'v']);
    }

    #[test]
    fn parse_skips_spaces() {
        let spaced  = parse_layout_string("q w e r\na s d f").unwrap();
        let compact = parse_layout_string("qwer\nasdf").unwrap();
        assert_eq!(spaced.num_cols, compact.num_cols);
        assert_eq!(spaced.num_rows, compact.num_rows);
        assert_eq!(spaced.keys, compact.keys);
    }

    #[test]
    fn parse_skips_empty_lines() {
        let p = parse_layout_string("qwer\n\nasdf\n").unwrap();
        assert_eq!(p.num_rows, 2);
    }

    #[test]
    fn parse_err_mismatched_row_length() {
        assert!(parse_layout_string("qwer\nasd").is_err());
    }

    #[test]
    fn parse_err_duplicate_key() {
        assert!(parse_layout_string("qqer\nasdf").is_err());
    }

    #[test]
    fn parse_err_empty() {
        assert!(parse_layout_string("").is_err());
        assert!(parse_layout_string("   \n  ").is_err());
    }

    #[test]
    fn parse_default_macro_keys() {
        let p = parse_layout_string("qwer\nasdf\nzxcv\nyuio\nhjkl\nnm,.").unwrap();
        assert_eq!(p.num_cols, 4);
        assert_eq!(p.num_rows, 6);
    }

    #[test]
    fn parse_default_sub_keys() {
        let p = parse_layout_string("ertyuio\ndfghjkl\ncvbnm,.").unwrap();
        assert_eq!(p.num_cols, 7);
        assert_eq!(p.num_rows, 3);
    }

    #[test]
    fn parse_default_subcell_keys() {
        let p = parse_layout_string("ertyui\ndfghjk\nxcvbnm").unwrap();
        assert_eq!(p.num_cols, 6);
        assert_eq!(p.num_rows, 3);
    }

    #[test]
    fn key_pos_found() {
        let p = parse_layout_string("qwer\nasdf\nzxcv").unwrap();
        assert_eq!(p.key_pos('q'), Some((0, 0)));
        assert_eq!(p.key_pos('r'), Some((3, 0)));
        assert_eq!(p.key_pos('z'), Some((0, 2)));
        assert_eq!(p.key_pos('v'), Some((3, 2)));
    }

    #[test]
    fn key_pos_not_found() {
        let p = parse_layout_string("qwer\nasdf").unwrap();
        assert_eq!(p.key_pos('z'), None);
    }

    // ── layout_geom ───────────────────────────────────────────────────────

    #[test]
    fn layout_geom_square_subcells() {
        // 4×6 macro, 7×3 sub — default layout
        let g = layout_geom(1440.0, 900.0, 4, 6, 7, 3);
        assert!((g.macro_w - 7.0 * g.sub_size).abs() < 1e-9);
        assert!((g.macro_h - 3.0 * g.sub_size).abs() < 1e-9);
    }

    #[test]
    fn layout_geom_fits_screen() {
        for (sw, sh) in [(1440.0_f64, 900.0), (2560.0, 1440.0), (1920.0, 1080.0)] {
            let g = layout_geom(sw, sh, 4, 6, 7, 3);
            assert!(g.grid_w <= sw + 1e-9, "grid_w {:.1} > sw {sw}", g.grid_w);
            assert!(g.grid_h <= sh + 1e-9, "grid_h {:.1} > sh {sh}", g.grid_h);
            assert!(g.offset_x >= -1e-9);
            assert!(g.offset_y >= -1e-9);
        }
    }

    #[test]
    fn layout_geom_fills_one_axis() {
        let g = layout_geom(1440.0, 900.0, 4, 6, 7, 3);
        let fills_w = (g.grid_w - 1440.0).abs() < 1e-9;
        let fills_h = (g.grid_h -  900.0).abs() < 1e-9;
        assert!(fills_w || fills_h, "grid should fill at least one screen dimension");
    }

    #[test]
    fn layout_geom_dynamic_dimensions() {
        // Verify the geometry adapts to arbitrary layout sizes
        let g = layout_geom(1920.0, 1080.0, 3, 4, 5, 3);
        // total 15×12 sub-cells; sub_size = min(1920/15, 1080/12) = min(128, 90) = 90
        assert!((g.sub_size - 90.0).abs() < 1e-9);
    }

    // ── keycode table ──────────────────────────────────────────────────────

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

    #[test]
    fn tab_keycode_not_a_printable_char() {
        assert_eq!(keycode_to_char(0x30), None, "Tab must not map to a printable char");
    }
}
