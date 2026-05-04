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
/// Every layer fills its parent exactly — cells are rectangular, no margins.
pub struct LayoutGeom {
    /// Stage-2 sub-cell width  = sw / (macro_cols × sub_cols).
    pub cell_w:  f64,
    /// Stage-2 sub-cell height = sh / (macro_rows × sub_rows).
    pub cell_h:  f64,
    /// Macro cell width  = cell_w × sub_cols.
    pub macro_w: f64,
    /// Macro cell height = cell_h × sub_rows.
    pub macro_h: f64,
}

/// Compute layout geometry from screen and parsed layout dimensions.
/// The full grid fills the screen exactly; each layer divides its parent
/// into equal-sized rectangular cells.
pub fn layout_geom(
    sw: f64, sh: f64,
    macro_cols: usize, macro_rows: usize,
    sub_cols:   usize, sub_rows:   usize,
) -> LayoutGeom {
    let cell_w = sw / (macro_cols * sub_cols) as f64;
    let cell_h = sh / (macro_rows * sub_rows) as f64;
    LayoutGeom {
        cell_w,
        cell_h,
        macro_w: cell_w * sub_cols as f64,
        macro_h: cell_h * sub_rows as f64,
    }
}

// ── Dynamic sub layout ─────────────────────────────────────────────────────

/// Pool for generating sub_l key rows (8 keys per row, 3 rows).
/// Row ordering matches keyboard home-adjacent rows: top → mid → bottom.
const SUB_KEY_POOL: [&str; 3] = ["ertyuiop", "dfghjkl;", "cvbnm,./"];

/// Maximum sub_cols supported by the pool.
const MAX_SUB_COLS: usize = 8;

/// Compute `sub_cols` (with `sub_rows` fixed at 3) so that stage-3 subcell
/// cells are as square as possible while the grid fills the screen exactly.
///
/// Derivation (bottom-up):
///   square subcell ⟹ cell_w/sc_cols = cell_h/sc_rows
///   ⟹ sub_cols/sub_rows = sw·macro_rows·sc_rows / (sh·macro_cols·sc_cols)
///   With sub_rows=3: sub_cols = round(target × 3), capped at MAX_SUB_COLS.
pub fn square_sub_cols(
    sw: f64, sh: f64,
    macro_cols: usize, macro_rows: usize,
    sc_cols:    usize, sc_rows:    usize,
) -> usize {
    let target = sw * macro_rows as f64 * sc_rows as f64
               / (sh * macro_cols as f64 * sc_cols as f64);
    ((target * 3.0).round() as usize).clamp(1, MAX_SUB_COLS)
}

/// Build a sub_l layout string (3 rows, `sub_cols` keys each) from the pool.
/// Always produces a valid, duplicate-free string accepted by `parse_layout_string`.
pub fn generate_sub_layout(sub_cols: usize) -> String {
    let cols = sub_cols.clamp(1, MAX_SUB_COLS);
    SUB_KEY_POOL.iter()
        .map(|row| row.chars().take(cols).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n")
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
    fn layout_geom_fills_screen_exactly() {
        for (sw, sh) in [(1440.0_f64, 900.0), (2560.0, 1440.0), (1920.0, 1080.0)] {
            let g = layout_geom(sw, sh, 4, 6, 7, 3);
            let grid_w = g.macro_w * 4.0;
            let grid_h = g.macro_h * 6.0;
            assert!((grid_w - sw).abs() < 1e-9, "grid_w {grid_w:.1} != sw {sw}");
            assert!((grid_h - sh).abs() < 1e-9, "grid_h {grid_h:.1} != sh {sh}");
        }
    }

    #[test]
    fn layout_geom_macro_derived_from_cells() {
        let g = layout_geom(1440.0, 900.0, 4, 6, 7, 3);
        assert!((g.macro_w - g.cell_w * 7.0).abs() < 1e-9);
        assert!((g.macro_h - g.cell_h * 3.0).abs() < 1e-9);
    }

    #[test]
    fn layout_geom_dynamic_dimensions() {
        // 3×4 macro, 5×3 sub on 1920×1080: cell_w=1920/15=128, cell_h=1080/12=90
        let g = layout_geom(1920.0, 1080.0, 3, 4, 5, 3);
        assert!((g.cell_w - 128.0).abs() < 1e-9);
        assert!((g.cell_h -  90.0).abs() < 1e-9);
    }

    // ── square_sub_cols ───────────────────────────────────────────────────────

    #[test]
    fn square_sub_cols_16x9() {
        // 4×6 macro, 6×3 subcell on 16:9 → sub_cols=4 gives exact squares
        let cols = square_sub_cols(2560.0, 1440.0, 4, 6, 6, 3);
        assert_eq!(cols, 4);
        // Verify resulting cell_w/sc_cols == cell_h/sc_rows
        let g = layout_geom(2560.0, 1440.0, 4, 6, cols, 3);
        let sc_w = g.cell_w / 6.0;
        let sc_h = g.cell_h / 3.0;
        assert!((sc_w - sc_h).abs() < 1e-9, "not square: {sc_w:.4} ≠ {sc_h:.4}");
    }

    #[test]
    fn square_sub_cols_1080p_16x9() {
        let cols = square_sub_cols(1920.0, 1080.0, 4, 6, 6, 3);
        assert_eq!(cols, 4);
        let g = layout_geom(1920.0, 1080.0, 4, 6, cols, 3);
        let sc_w = g.cell_w / 6.0;
        let sc_h = g.cell_h / 3.0;
        assert!((sc_w - sc_h).abs() < 1e-9);
    }

    #[test]
    fn square_sub_cols_never_zero() {
        let cols = square_sub_cols(800.0, 2400.0, 4, 6, 6, 3);
        assert!(cols >= 1);
    }

    #[test]
    fn square_sub_cols_capped_at_pool_max() {
        let cols = square_sub_cols(10000.0, 1080.0, 4, 6, 6, 3);
        assert!(cols <= MAX_SUB_COLS);
    }

    // ── generate_sub_layout ───────────────────────────────────────────────────

    #[test]
    fn generate_sub_layout_valid_parse() {
        for cols in 1..=MAX_SUB_COLS {
            let s = generate_sub_layout(cols);
            let p = parse_layout_string(&s)
                .unwrap_or_else(|e| panic!("generate_sub_layout({cols}) is invalid: {e}"));
            assert_eq!(p.num_cols, cols, "wrong num_cols for sub_cols={cols}");
            assert_eq!(p.num_rows, 3);
        }
    }

    #[test]
    fn generate_sub_layout_no_duplicates() {
        let s = generate_sub_layout(8);
        assert!(parse_layout_string(&s).is_ok());
    }

    #[test]
    fn generate_sub_layout_4_matches_expected_keys() {
        let p = parse_layout_string(&generate_sub_layout(4)).unwrap();
        assert_eq!(p.keys[0], &['e', 'r', 't', 'y']);
        assert_eq!(p.keys[1], &['d', 'f', 'g', 'h']);
        assert_eq!(p.keys[2], &['c', 'v', 'b', 'n']);
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
