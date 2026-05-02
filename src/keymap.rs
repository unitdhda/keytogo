/// macOS virtual keycode → printable char (ANSI QWERTY layout).
///
/// Keycodes are physical-key positions, not Unicode values.
/// Based on Carbon HIToolbox/Events.h kVK_ANSI_* constants.
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

/// The 18 subcell keys with their physical stagger offsets (x in key-width units, y = row 0/1/2).
/// x is relative to 'd' at 0.0. Row spacing is uniform (1.0).
pub const SUBCELL_KEYS: &[(char, f32, f32)] = &[
    // top row — QWERTY row stagger +0.5 relative to home row
    ('e', 0.5, 0.0), ('r', 1.5, 0.0), ('t', 2.5, 0.0),
    ('y', 3.5, 0.0), ('u', 4.5, 0.0), ('i', 5.5, 0.0),
    // home row — baseline
    ('d', 0.0, 1.0), ('f', 1.0, 1.0), ('g', 2.0, 1.0),
    ('h', 3.0, 1.0), ('j', 4.0, 1.0), ('k', 5.0, 1.0),
    // bottom row — ZXCV row stagger +0.25 relative to home row
    ('x', 0.25, 2.0), ('c', 1.25, 2.0), ('v', 2.25, 2.0),
    ('b', 3.25, 2.0), ('n', 4.25, 2.0), ('m', 5.25, 2.0),
];

/// X span of the subcell grid in key-width units (from 'd'=0.0 to 'i'=5.5).
pub const SUBCELL_X_SPAN: f32 = 5.5;
