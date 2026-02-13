//! Keyboard input encoding: GPUI keystroke -> terminal escape sequences.
//!
//! Converts GPUI KeyDownEvents into byte sequences suitable for writing to the PTY.
//! Supports: ASCII, UTF-8, Ctrl+key, special keys, cursor keys (normal/application mode),
//! function keys F1-F12, editing keys, modifier encoding, and Alt/ESC prefix.

use std::io::Write;

use crux_terminal::TermMode;
use gpui::Keystroke;

/// Whether the macOS Option key should be treated as Alt.
/// When true, Option+key sends ESC prefix (terminal Alt behavior).
/// When false, Option+key sends the macOS special character (e.g., å, ∫, ç).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)] // TODO: Left/Right variants will be used when per-modifier option-as-alt config is wired
pub enum OptionAsAlt {
    /// Neither Option key acts as Alt (macOS default behavior).
    None,
    /// Left Option acts as Alt, right Option for composition — needs platform API.
    Left,
    /// Right Option acts as Alt, left Option for composition — needs platform API.
    Right,
    /// Both Option keys act as Alt (typical terminal behavior).
    Both,
}

/// Convert a GPUI Keystroke into a byte sequence for the PTY.
///
/// `mode` contains the current terminal mode flags, used to determine
/// whether cursor keys should use application mode (SS3) encoding.
pub fn keystroke_to_bytes(
    keystroke: &Keystroke,
    mode: TermMode,
    option_as_alt: OptionAsAlt,
) -> Option<Vec<u8>> {
    let mods = modifier_param(keystroke);
    let has_shift = keystroke.modifiers.shift;
    let has_alt = keystroke.modifiers.alt
        && match option_as_alt {
            OptionAsAlt::None => false,
            // Left/Right behave as Both until GPUI exposes left/right Alt distinction.
            OptionAsAlt::Left | OptionAsAlt::Right | OptionAsAlt::Both => true,
        };
    let has_ctrl = keystroke.modifiers.control;
    let app_cursor = mode.contains(TermMode::APP_CURSOR);

    match keystroke.key.as_str() {
        // Special keys that produce fixed sequences.
        "enter" => Some(b"\r".to_vec()),
        "tab" => {
            if has_shift {
                Some(b"\x1b[Z".to_vec())
            } else {
                Some(b"\t".to_vec())
            }
        }
        "backspace" => {
            if has_alt {
                Some(b"\x1b\x7f".to_vec())
            } else if has_ctrl {
                Some(b"\x08".to_vec())
            } else {
                Some(b"\x7f".to_vec())
            }
        }
        "escape" => Some(b"\x1b".to_vec()),
        "space" => {
            if has_ctrl {
                Some(vec![0x00])
            } else if has_alt {
                Some(b"\x1b ".to_vec())
            } else {
                Some(b" ".to_vec())
            }
        }

        // Cursor keys: respect DECCKM (application cursor mode).
        "up" => Some(cursor_key(b'A', mods, app_cursor)),
        "down" => Some(cursor_key(b'B', mods, app_cursor)),
        "right" => Some(cursor_key(b'C', mods, app_cursor)),
        "left" => Some(cursor_key(b'D', mods, app_cursor)),

        // Home/End: xterm style (CSI H/F).
        "home" => Some(cursor_key(b'H', mods, app_cursor)),
        "end" => Some(cursor_key(b'F', mods, app_cursor)),

        // Editing keys: CSI number ~ format.
        "insert" => Some(csi_tilde(2, mods)),
        "delete" => Some(csi_tilde(3, mods)),
        "pageup" => Some(csi_tilde(5, mods)),
        "pagedown" => Some(csi_tilde(6, mods)),

        // Function keys F1-F4: SS3 letter (no modifiers) or CSI 1;mod letter.
        "f1" => Some(f1_f4(b'P', mods)),
        "f2" => Some(f1_f4(b'Q', mods)),
        "f3" => Some(f1_f4(b'R', mods)),
        "f4" => Some(f1_f4(b'S', mods)),

        // Function keys F5-F12: CSI number ~ format (note: non-contiguous numbers).
        "f5" => Some(csi_tilde(15, mods)),
        "f6" => Some(csi_tilde(17, mods)),
        "f7" => Some(csi_tilde(18, mods)),
        "f8" => Some(csi_tilde(19, mods)),
        "f9" => Some(csi_tilde(20, mods)),
        "f10" => Some(csi_tilde(21, mods)),
        "f11" => Some(csi_tilde(23, mods)),
        "f12" => Some(csi_tilde(24, mods)),

        key => {
            // Ctrl+key combinations produce control characters.
            if has_ctrl {
                if let Some(ch) = key.chars().next() {
                    if let Some(ctrl_byte) = ctrl_char(ch) {
                        return if has_alt {
                            Some(vec![0x1b, ctrl_byte])
                        } else {
                            Some(vec![ctrl_byte])
                        };
                    }
                }
            }

            // Alt with option_as_alt: send ESC + base key character.
            if has_alt {
                let base = key.as_bytes();
                if !base.is_empty() {
                    let mut bytes = vec![0x1b];
                    bytes.extend_from_slice(base);
                    return Some(bytes);
                }
            }

            // Fall through to key_char for printable text.
            if let Some(text) = &keystroke.key_char {
                if !text.is_empty() {
                    return Some(text.as_bytes().to_vec());
                }
            }

            None
        }
    }
}

/// Compute xterm modifier parameter: 1 + (Shift:1 | Alt:2 | Ctrl:4).
/// Returns 0 if no modifiers (meaning parameter should be omitted).
fn modifier_param(keystroke: &Keystroke) -> u8 {
    let mut bits: u8 = 0;
    if keystroke.modifiers.shift {
        bits |= 1;
    }
    if keystroke.modifiers.alt {
        bits |= 2;
    }
    if keystroke.modifiers.control {
        bits |= 4;
    }
    bits
}

/// Cursor key encoding: SS3 in application mode (no modifiers), CSI otherwise.
fn cursor_key(letter: u8, mods: u8, app_cursor: bool) -> Vec<u8> {
    if mods == 0 && app_cursor {
        // Application mode: SS3 letter
        vec![0x1b, b'O', letter]
    } else if mods == 0 {
        // Normal mode: CSI letter
        vec![0x1b, b'[', letter]
    } else {
        // With modifiers: CSI 1;{param} letter
        let mut buf = Vec::with_capacity(8);
        // infallible: writing to Vec<u8>
        write!(buf, "\x1b[1;{}{}", mods + 1, letter as char).unwrap();
        buf
    }
}

/// F1-F4 encoding: SS3 letter (no modifiers) or CSI 1;{mod} letter.
fn f1_f4(letter: u8, mods: u8) -> Vec<u8> {
    if mods == 0 {
        vec![0x1b, b'O', letter]
    } else {
        let mut buf = Vec::with_capacity(8);
        // infallible: writing to Vec<u8>
        write!(buf, "\x1b[1;{}{}", mods + 1, letter as char).unwrap();
        buf
    }
}

/// CSI number [;modifier] ~ encoding for editing and function keys.
fn csi_tilde(number: u32, mods: u8) -> Vec<u8> {
    let mut buf = Vec::with_capacity(8);
    if mods == 0 {
        // infallible: writing to Vec<u8>
        write!(buf, "\x1b[{number}~").unwrap();
    } else {
        // infallible: writing to Vec<u8>
        write!(buf, "\x1b[{number};{}~", mods + 1).unwrap();
    }
    buf
}

/// Map Ctrl+character to the corresponding C0 control code.
fn ctrl_char(ch: char) -> Option<u8> {
    match ch {
        'a'..='z' => Some(ch as u8 - b'a' + 1),
        'A'..='Z' => Some(ch as u8 - b'A' + 1),
        '@' => Some(0),
        '[' => Some(27),
        '\\' => Some(28),
        ']' => Some(29),
        '^' => Some(30),
        '_' => Some(31),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::{Keystroke, Modifiers};

    fn make_keystroke(key: &str, key_char: Option<&str>, mods: Modifiers) -> Keystroke {
        Keystroke {
            key: key.to_string(),
            key_char: key_char.map(|s| s.to_string()),
            modifiers: mods,
        }
    }

    #[test]
    fn test_enter() {
        let ks = make_keystroke("enter", None, Modifiers::default());
        assert_eq!(
            keystroke_to_bytes(&ks, TermMode::empty(), OptionAsAlt::Both),
            Some(b"\r".to_vec())
        );
    }

    #[test]
    fn test_printable_char() {
        let ks = make_keystroke("a", Some("a"), Modifiers::default());
        assert_eq!(
            keystroke_to_bytes(&ks, TermMode::empty(), OptionAsAlt::Both),
            Some(b"a".to_vec())
        );
    }

    #[test]
    fn test_ctrl_c() {
        let ks = make_keystroke(
            "c",
            None,
            Modifiers {
                control: true,
                ..Default::default()
            },
        );
        assert_eq!(
            keystroke_to_bytes(&ks, TermMode::empty(), OptionAsAlt::Both),
            Some(vec![3])
        );
    }

    #[test]
    fn test_arrow_normal() {
        let ks = make_keystroke("up", None, Modifiers::default());
        assert_eq!(
            keystroke_to_bytes(&ks, TermMode::empty(), OptionAsAlt::Both),
            Some(b"\x1b[A".to_vec())
        );
    }

    #[test]
    fn test_arrow_application() {
        let ks = make_keystroke("up", None, Modifiers::default());
        assert_eq!(
            keystroke_to_bytes(&ks, TermMode::APP_CURSOR, OptionAsAlt::Both),
            Some(b"\x1bOA".to_vec())
        );
    }

    #[test]
    fn test_ctrl_arrow() {
        let ks = make_keystroke(
            "up",
            None,
            Modifiers {
                control: true,
                ..Default::default()
            },
        );
        assert_eq!(
            keystroke_to_bytes(&ks, TermMode::empty(), OptionAsAlt::Both),
            Some(b"\x1b[1;5A".to_vec())
        );
    }

    #[test]
    fn test_f1_no_mod() {
        let ks = make_keystroke("f1", None, Modifiers::default());
        assert_eq!(
            keystroke_to_bytes(&ks, TermMode::empty(), OptionAsAlt::Both),
            Some(b"\x1bOP".to_vec())
        );
    }

    #[test]
    fn test_f5_no_mod() {
        let ks = make_keystroke("f5", None, Modifiers::default());
        assert_eq!(
            keystroke_to_bytes(&ks, TermMode::empty(), OptionAsAlt::Both),
            Some(b"\x1b[15~".to_vec())
        );
    }

    #[test]
    fn test_shift_tab() {
        let ks = make_keystroke(
            "tab",
            None,
            Modifiers {
                shift: true,
                ..Default::default()
            },
        );
        assert_eq!(
            keystroke_to_bytes(&ks, TermMode::empty(), OptionAsAlt::Both),
            Some(b"\x1b[Z".to_vec())
        );
    }

    #[test]
    fn test_alt_a() {
        let ks = make_keystroke(
            "a",
            Some("a"),
            Modifiers {
                alt: true,
                ..Default::default()
            },
        );
        assert_eq!(
            keystroke_to_bytes(&ks, TermMode::empty(), OptionAsAlt::Both),
            Some(b"\x1ba".to_vec())
        );
    }

    #[test]
    fn test_option_as_alt_none_sends_keychar() {
        // When option_as_alt is None, Alt+a should send the key_char as-is (not ESC prefix)
        let ks = make_keystroke(
            "a",
            Some("å"),
            Modifiers {
                alt: true,
                ..Default::default()
            },
        );
        assert_eq!(
            keystroke_to_bytes(&ks, TermMode::empty(), OptionAsAlt::None),
            Some("å".as_bytes().to_vec())
        );
    }

    #[test]
    fn test_option_as_alt_both_sends_esc_prefix() {
        let ks = make_keystroke(
            "a",
            Some("å"),
            Modifiers {
                alt: true,
                ..Default::default()
            },
        );
        assert_eq!(
            keystroke_to_bytes(&ks, TermMode::empty(), OptionAsAlt::Both),
            Some(b"\x1ba".to_vec()) // ESC + base key, not ESC + key_char
        );
    }

    #[test]
    fn test_option_as_alt_left_acts_as_both() {
        // Left variant behaves same as Both until platform API available.
        let ks = make_keystroke(
            "a",
            Some("å"),
            Modifiers {
                alt: true,
                ..Default::default()
            },
        );
        assert_eq!(
            keystroke_to_bytes(&ks, TermMode::empty(), OptionAsAlt::Left),
            Some(b"\x1ba".to_vec())
        );
    }

    #[test]
    fn test_option_as_alt_right_acts_as_both() {
        // Right variant behaves same as Both until platform API available.
        let ks = make_keystroke(
            "a",
            Some("å"),
            Modifiers {
                alt: true,
                ..Default::default()
            },
        );
        assert_eq!(
            keystroke_to_bytes(&ks, TermMode::empty(), OptionAsAlt::Right),
            Some(b"\x1ba".to_vec())
        );
    }
}
