//! Kitty keyboard protocol encoder (CSI u format).
//!
//! Implements the Kitty progressive enhancement protocol for keyboard input:
//! https://sw.kovidgoyal.net/kitty/keyboard-protocol/
//!
//! Supported flags:
//! - Flag 1 (DISAMBIGUATE_ESC_CODES): Encode all keys in CSI u format
//! - Flag 2 (REPORT_EVENT_TYPES): Report press/repeat/release events
//! - Flag 4 (REPORT_ALTERNATE_KEYS): Report shifted key and base layout key
//!
//! Wired into keyboard.rs for progressive enhancement when flags are enabled.

use std::io::Write;

use gpui::Keystroke;

use crate::input::OptionAsAlt;

/// Kitty keyboard protocol flags.
///
/// These are separate from TermMode and tracked independently.
/// Applications enable them via CSI > Ps u escape sequences.
#[derive(Debug, Clone, Copy, Default)]
pub struct KittyKeyboardFlags {
    /// Flag 1: Disambiguate escape codes (encode all keys in CSI u format).
    pub disambiguate_esc_codes: bool,
    /// Flag 2: Report event types (press, repeat, release).
    pub report_event_types: bool,
    /// Flag 4: Report alternate keys (shifted key, base layout key).
    pub report_alternate_keys: bool,
}

/// Event type for keyboard events (Flag 2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyEventType {
    /// Key press event (default, omitted if alone).
    Press = 1,
    /// Key repeat event (key held down).
    Repeat = 2,
    /// Key release event (key lifted).
    Release = 3,
}

/// Encode a keystroke using the Kitty keyboard protocol.
///
/// Supports progressive enhancement flags:
/// - Flag 1: CSI u format `\x1b[{codepoint}[;{modifiers}]u`
/// - Flag 2: Event types `\x1b[{codepoint};{modifiers}:{event_type}u`
/// - Flag 4: Alternate keys `\x1b[{base}:{shifted}[:{layout}];{modifiers}u`
///
/// Returns `None` if the key should not be encoded (e.g., when flags are disabled
/// or for keys that remain in legacy format for compatibility).
pub fn kitty_encode_key(
    keystroke: &Keystroke,
    flags: &KittyKeyboardFlags,
    option_as_alt: OptionAsAlt,
    event_type: KeyEventType,
) -> Option<Vec<u8>> {
    // Only encode if Flag 1 (DISAMBIGUATE_ESC_CODES) is active.
    if !flags.disambiguate_esc_codes {
        return None;
    }

    let mods = compute_modifier_param(keystroke, option_as_alt);
    let report_event_types = flags.report_event_types;
    let report_alternate_keys = flags.report_alternate_keys;

    match keystroke.key.as_str() {
        // Esc always encodes to CSI 27 u (with or without modifiers).
        "escape" => Some(encode_csi_u(27, None, None, mods, event_type, report_event_types)),

        // Tab becomes CSI 9 u (vs Ctrl+I → CSI 105;5u).
        "tab" => Some(encode_csi_u(9, None, None, mods, event_type, report_event_types)),

        // Enter becomes CSI 13 u (vs Ctrl+M → CSI 109;5u).
        "enter" => Some(encode_csi_u(13, None, None, mods, event_type, report_event_types)),

        // Backspace → CSI 127 u.
        "backspace" => Some(encode_csi_u(127, None, None, mods, event_type, report_event_types)),

        // Space → CSI 32 u (distinguishes Ctrl+Space from space).
        "space" => Some(encode_csi_u(32, None, None, mods, event_type, report_event_types)),

        // Arrow keys, F1-F4 keep legacy encoding when no modifiers.
        // This is per the Kitty spec: applications rely on these legacy sequences.
        "up" | "down" | "left" | "right" | "home" | "end"
        | "insert" | "delete" | "pageup" | "pagedown"
        | "f1" | "f2" | "f3" | "f4" | "f5" | "f6"
        | "f7" | "f8" | "f9" | "f10" | "f11" | "f12" => {
            // Only encode if there are modifiers. Otherwise, let legacy path handle it.
            if mods > 0 {
                // Map named keys to Unicode codepoints per Kitty protocol spec.
                let codepoint = match keystroke.key.as_str() {
                    "up" => 57362,
                    "down" => 57363,
                    "right" => 57364,
                    "left" => 57361,
                    "home" => 57360,
                    "end" => 57367,
                    "insert" => 57358,
                    "delete" => 57359,
                    "pageup" => 57365,
                    "pagedown" => 57366,
                    "f1" => 57376,
                    "f2" => 57377,
                    "f3" => 57378,
                    "f4" => 57379,
                    "f5" => 57380,
                    "f6" => 57381,
                    "f7" => 57382,
                    "f8" => 57383,
                    "f9" => 57384,
                    "f10" => 57385,
                    "f11" => 57386,
                    "f12" => 57387,
                    _ => return None,
                };
                Some(encode_csi_u(codepoint, None, None, mods, event_type, report_event_types))
            } else {
                None // Let legacy encoder handle unmodified special keys.
            }
        }

        // Text keys: encode if they have modifiers (Ctrl/Alt/Cmd).
        key => {
            // Ctrl+key combinations: encode the base letter codepoint + modifier.
            if keystroke.modifiers.control {
                if let Some(ch) = key.chars().next() {
                    // For Ctrl+letter, encode the lowercase letter codepoint.
                    let base_codepoint = if ch.is_ascii_alphabetic() {
                        ch.to_ascii_lowercase() as u32
                    } else {
                        ch as u32
                    };

                    // Flag 4: Report shifted key if applicable.
                    let shifted_key = if report_alternate_keys && ch.is_ascii_alphabetic() && keystroke.modifiers.shift {
                        Some(ch.to_ascii_uppercase() as u32)
                    } else {
                        None
                    };

                    return Some(encode_csi_u(base_codepoint, shifted_key, None, mods, event_type, report_event_types));
                }
            }

            // Alt+key combinations: encode with Alt modifier.
            let has_alt = keystroke.modifiers.alt
                && !matches!(option_as_alt, OptionAsAlt::None);
            if has_alt {
                if let Some(ch) = key.chars().next() {
                    let base_codepoint = ch as u32;
                    let shifted_key = if report_alternate_keys && ch.is_ascii_alphabetic() && keystroke.modifiers.shift {
                        Some(ch.to_ascii_uppercase() as u32)
                    } else {
                        None
                    };
                    return Some(encode_csi_u(base_codepoint, shifted_key, None, mods, event_type, report_event_types));
                }
            }

            // Cmd+key combinations: encode with Super modifier.
            if keystroke.modifiers.platform {
                if let Some(ch) = key.chars().next() {
                    let base_codepoint = ch as u32;
                    let shifted_key = if report_alternate_keys && ch.is_ascii_alphabetic() && keystroke.modifiers.shift {
                        Some(ch.to_ascii_uppercase() as u32)
                    } else {
                        None
                    };
                    return Some(encode_csi_u(base_codepoint, shifted_key, None, mods, event_type, report_event_types));
                }
            }

            // Shift+key for non-letters (e.g., Shift+1 → !) should still be raw UTF-8.
            // Only encode if there's a non-shift modifier.
            if keystroke.modifiers.shift && !keystroke.modifiers.control && !has_alt && !keystroke.modifiers.platform {
                return None; // Let raw UTF-8 path handle Shift+key.
            }

            // Plain text keys without modifiers: let legacy path handle as raw UTF-8.
            None
        }
    }
}

/// Encode a key as CSI u format with optional event type and alternate keys.
///
/// Format variations:
/// - Basic: `CSI codepoint u`
/// - With modifiers: `CSI codepoint;modifier u`
/// - With event type: `CSI codepoint;modifier:event_type u`
/// - With shifted key: `CSI base:shifted;modifier u`
/// - With layout key: `CSI base::layout;modifier u`
/// - With shifted and layout: `CSI base:shifted:layout;modifier u`
fn encode_csi_u(
    base_codepoint: u32,
    shifted_key: Option<u32>,
    layout_key: Option<u32>,
    modifier: u8,
    event_type: KeyEventType,
    report_event_types: bool,
) -> Vec<u8> {
    let mut buf = Vec::with_capacity(32);

    // Start CSI sequence.
    buf.extend_from_slice(b"\x1b[");

    // Encode the key codepoint(s).
    write!(buf, "{}", base_codepoint).unwrap();

    // Add alternate keys if Flag 4 is active.
    if shifted_key.is_some() || layout_key.is_some() {
        if let Some(shifted) = shifted_key {
            write!(buf, ":{}", shifted).unwrap();
        } else {
            buf.push(b':');
        }

        if let Some(layout) = layout_key {
            write!(buf, ":{}", layout).unwrap();
        }
    }

    // Add modifier parameter if present.
    if modifier > 0 {
        write!(buf, ";{}", modifier).unwrap();

        // Add event type after modifier if Flag 2 is active.
        if report_event_types {
            write!(buf, ":{}", event_type as u8).unwrap();
        }
    } else if report_event_types && event_type != KeyEventType::Press {
        // No modifiers but event type is not default press.
        // Still need to encode: CSI codepoint;:event_type u
        write!(buf, ";:{}", event_type as u8).unwrap();
    }

    // Terminate with 'u'.
    buf.push(b'u');
    buf
}

/// Compute the modifier parameter for CSI u encoding.
///
/// Formula: modifier = 1 + (shift:1 | alt:2 | ctrl:4 | super:8)
/// Returns 0 if no modifiers (meaning parameter should be omitted).
fn compute_modifier_param(keystroke: &Keystroke, option_as_alt: OptionAsAlt) -> u8 {
    let mut bits: u8 = 0;

    if keystroke.modifiers.shift {
        bits |= 1;
    }

    // Alt modifier: respect option_as_alt setting.
    if keystroke.modifiers.alt {
        match option_as_alt {
            OptionAsAlt::None => {} // macOS special char; don't treat as Alt.
            _ => bits |= 2,
        }
    }

    if keystroke.modifiers.control {
        bits |= 4;
    }

    if keystroke.modifiers.platform {
        bits |= 8;
    }

    // Return the final parameter (1-indexed if any modifiers, 0 if none).
    if bits == 0 {
        0
    } else {
        bits + 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gpui::Modifiers;

    fn make_keystroke(key: &str, mods: Modifiers) -> Keystroke {
        Keystroke {
            key: key.to_string(),
            key_char: None,
            modifiers: mods,
        }
    }

    fn flags_with_flag1() -> KittyKeyboardFlags {
        KittyKeyboardFlags {
            disambiguate_esc_codes: true,
            report_event_types: false,
            report_alternate_keys: false,
        }
    }

    #[test]
    fn test_escape_plain() {
        let ks = make_keystroke("escape", Modifiers::default());
        assert_eq!(
            kitty_encode_key(&ks, &flags_with_flag1(), OptionAsAlt::Both, KeyEventType::Press),
            Some(b"\x1b[27u".to_vec())
        );
    }

    #[test]
    fn test_escape_with_shift() {
        let ks = make_keystroke(
            "escape",
            Modifiers {
                shift: true,
                ..Default::default()
            },
        );
        // modifier = 1 + 1 (shift) = 2
        assert_eq!(
            kitty_encode_key(&ks, &flags_with_flag1(), OptionAsAlt::Both, KeyEventType::Press),
            Some(b"\x1b[27;2u".to_vec())
        );
    }

    #[test]
    fn test_tab_plain() {
        let ks = make_keystroke("tab", Modifiers::default());
        assert_eq!(
            kitty_encode_key(&ks, &flags_with_flag1(), OptionAsAlt::Both, KeyEventType::Press),
            Some(b"\x1b[9u".to_vec())
        );
    }

    #[test]
    fn test_ctrl_i_vs_tab() {
        // Ctrl+I should encode as codepoint 105 (lowercase i) with modifier 5 (ctrl).
        let ks = make_keystroke(
            "i",
            Modifiers {
                control: true,
                ..Default::default()
            },
        );
        // modifier = 1 + 4 (ctrl) = 5
        assert_eq!(
            kitty_encode_key(&ks, &flags_with_flag1(), OptionAsAlt::Both, KeyEventType::Press),
            Some(b"\x1b[105;5u".to_vec())
        );
    }

    #[test]
    fn test_enter_plain() {
        let ks = make_keystroke("enter", Modifiers::default());
        assert_eq!(
            kitty_encode_key(&ks, &flags_with_flag1(), OptionAsAlt::Both, KeyEventType::Press),
            Some(b"\x1b[13u".to_vec())
        );
    }

    #[test]
    fn test_ctrl_m_vs_enter() {
        // Ctrl+M should encode as codepoint 109 (lowercase m) with modifier 5.
        let ks = make_keystroke(
            "m",
            Modifiers {
                control: true,
                ..Default::default()
            },
        );
        // modifier = 1 + 4 (ctrl) = 5
        assert_eq!(
            kitty_encode_key(&ks, &flags_with_flag1(), OptionAsAlt::Both, KeyEventType::Press),
            Some(b"\x1b[109;5u".to_vec())
        );
    }

    #[test]
    fn test_ctrl_a() {
        let ks = make_keystroke(
            "a",
            Modifiers {
                control: true,
                ..Default::default()
            },
        );
        // modifier = 1 + 4 (ctrl) = 5
        assert_eq!(
            kitty_encode_key(&ks, &flags_with_flag1(), OptionAsAlt::Both, KeyEventType::Press),
            Some(b"\x1b[97;5u".to_vec()) // 'a' = 97
        );
    }

    #[test]
    fn test_alt_a() {
        let ks = make_keystroke(
            "a",
            Modifiers {
                alt: true,
                ..Default::default()
            },
        );
        // modifier = 1 + 2 (alt) = 3
        assert_eq!(
            kitty_encode_key(&ks, &flags_with_flag1(), OptionAsAlt::Both, KeyEventType::Press),
            Some(b"\x1b[97;3u".to_vec())
        );
    }

    #[test]
    fn test_ctrl_alt_a() {
        let ks = make_keystroke(
            "a",
            Modifiers {
                control: true,
                alt: true,
                ..Default::default()
            },
        );
        // modifier = 1 + 2 (alt) + 4 (ctrl) = 7
        assert_eq!(
            kitty_encode_key(&ks, &flags_with_flag1(), OptionAsAlt::Both, KeyEventType::Press),
            Some(b"\x1b[97;7u".to_vec())
        );
    }

    #[test]
    fn test_arrow_no_modifiers() {
        // Arrow keys without modifiers should return None (legacy path handles them).
        let ks = make_keystroke("up", Modifiers::default());
        assert_eq!(
            kitty_encode_key(&ks, &flags_with_flag1(), OptionAsAlt::Both, KeyEventType::Press),
            None
        );
    }

    #[test]
    fn test_arrow_with_ctrl() {
        // Arrow keys with modifiers should encode.
        let ks = make_keystroke(
            "up",
            Modifiers {
                control: true,
                ..Default::default()
            },
        );
        // modifier = 1 + 4 (ctrl) = 5
        // up arrow = 57362
        assert_eq!(
            kitty_encode_key(&ks, &flags_with_flag1(), OptionAsAlt::Both, KeyEventType::Press),
            Some(b"\x1b[57362;5u".to_vec())
        );
    }

    #[test]
    fn test_plain_a() {
        // Plain 'a' without modifiers should return None (raw UTF-8 path handles it).
        let ks = make_keystroke("a", Modifiers::default());
        assert_eq!(
            kitty_encode_key(&ks, &flags_with_flag1(), OptionAsAlt::Both, KeyEventType::Press),
            None
        );
    }

    #[test]
    fn test_no_flag1_returns_none() {
        // Without Flag 1, should return None (legacy encoder takes over).
        let ks = make_keystroke("escape", Modifiers::default());
        let flags = KittyKeyboardFlags::default(); // All flags disabled
        assert_eq!(
            kitty_encode_key(&ks, &flags, OptionAsAlt::Both, KeyEventType::Press),
            None
        );
    }

    #[test]
    fn test_backspace() {
        let ks = make_keystroke("backspace", Modifiers::default());
        assert_eq!(
            kitty_encode_key(&ks, &flags_with_flag1(), OptionAsAlt::Both, KeyEventType::Press),
            Some(b"\x1b[127u".to_vec())
        );
    }

    #[test]
    fn test_space() {
        let ks = make_keystroke("space", Modifiers::default());
        assert_eq!(
            kitty_encode_key(&ks, &flags_with_flag1(), OptionAsAlt::Both, KeyEventType::Press),
            Some(b"\x1b[32u".to_vec())
        );
    }

    #[test]
    fn test_ctrl_space() {
        let ks = make_keystroke(
            "space",
            Modifiers {
                control: true,
                ..Default::default()
            },
        );
        // modifier = 1 + 4 (ctrl) = 5
        assert_eq!(
            kitty_encode_key(&ks, &flags_with_flag1(), OptionAsAlt::Both, KeyEventType::Press),
            Some(b"\x1b[32;5u".to_vec())
        );
    }

    #[test]
    fn test_f1_no_modifiers() {
        // F1 without modifiers → None (legacy path).
        let ks = make_keystroke("f1", Modifiers::default());
        assert_eq!(
            kitty_encode_key(&ks, &flags_with_flag1(), OptionAsAlt::Both, KeyEventType::Press),
            None
        );
    }

    #[test]
    fn test_shift_f1() {
        // F1 with shift → encode.
        let ks = make_keystroke(
            "f1",
            Modifiers {
                shift: true,
                ..Default::default()
            },
        );
        // modifier = 1 + 1 (shift) = 2
        // F1 = 57376
        assert_eq!(
            kitty_encode_key(&ks, &flags_with_flag1(), OptionAsAlt::Both, KeyEventType::Press),
            Some(b"\x1b[57376;2u".to_vec())
        );
    }

    #[test]
    fn test_option_as_alt_none() {
        // When option_as_alt is None, Alt modifier should not be encoded.
        let ks = make_keystroke(
            "a",
            Modifiers {
                alt: true,
                ..Default::default()
            },
        );
        // modifier = 1 + 0 (alt ignored) = 0 → no encoding (returns None).
        assert_eq!(
            kitty_encode_key(&ks, &flags_with_flag1(), OptionAsAlt::None, KeyEventType::Press),
            None
        );
    }

    // ========================================================================
    // Phase 2 Tests: Flag 2 (Event Types) and Flag 4 (Alternate Keys)
    // ========================================================================

    fn flags_with_flag2() -> KittyKeyboardFlags {
        KittyKeyboardFlags {
            disambiguate_esc_codes: true,
            report_event_types: true,
            report_alternate_keys: false,
        }
    }

    fn flags_with_flag4() -> KittyKeyboardFlags {
        KittyKeyboardFlags {
            disambiguate_esc_codes: true,
            report_event_types: false,
            report_alternate_keys: true,
        }
    }

    fn flags_with_flags_1_2_4() -> KittyKeyboardFlags {
        KittyKeyboardFlags {
            disambiguate_esc_codes: true,
            report_event_types: true,
            report_alternate_keys: true,
        }
    }

    // Flag 2: Event Type Tests

    #[test]
    fn test_flag2_ctrl_a_press() {
        let ks = make_keystroke(
            "a",
            Modifiers {
                control: true,
                ..Default::default()
            },
        );
        // With Flag 2: CSI 97;5:1 u (event type 1 = press)
        assert_eq!(
            kitty_encode_key(&ks, &flags_with_flag2(), OptionAsAlt::Both, KeyEventType::Press),
            Some(b"\x1b[97;5:1u".to_vec())
        );
    }

    #[test]
    fn test_flag2_ctrl_a_repeat() {
        let ks = make_keystroke(
            "a",
            Modifiers {
                control: true,
                ..Default::default()
            },
        );
        // With Flag 2: CSI 97;5:2 u (event type 2 = repeat)
        assert_eq!(
            kitty_encode_key(&ks, &flags_with_flag2(), OptionAsAlt::Both, KeyEventType::Repeat),
            Some(b"\x1b[97;5:2u".to_vec())
        );
    }

    #[test]
    fn test_flag2_ctrl_a_release() {
        let ks = make_keystroke(
            "a",
            Modifiers {
                control: true,
                ..Default::default()
            },
        );
        // With Flag 2: CSI 97;5:3 u (event type 3 = release)
        assert_eq!(
            kitty_encode_key(&ks, &flags_with_flag2(), OptionAsAlt::Both, KeyEventType::Release),
            Some(b"\x1b[97;5:3u".to_vec())
        );
    }

    #[test]
    fn test_flag2_escape_no_modifiers_release() {
        let ks = make_keystroke("escape", Modifiers::default());
        // No modifiers but release event: CSI 27;:3 u
        assert_eq!(
            kitty_encode_key(&ks, &flags_with_flag2(), OptionAsAlt::Both, KeyEventType::Release),
            Some(b"\x1b[27;:3u".to_vec())
        );
    }

    #[test]
    fn test_flag2_escape_no_modifiers_press() {
        let ks = make_keystroke("escape", Modifiers::default());
        // No modifiers and press event (default): CSI 27 u (event type omitted)
        assert_eq!(
            kitty_encode_key(&ks, &flags_with_flag2(), OptionAsAlt::Both, KeyEventType::Press),
            Some(b"\x1b[27u".to_vec())
        );
    }

    // Flag 4: Alternate Key Tests

    #[test]
    fn test_flag4_shift_a_with_ctrl() {
        let ks = make_keystroke(
            "a",
            Modifiers {
                control: true,
                shift: true,
                ..Default::default()
            },
        );
        // Flag 4: Report shifted key (A = 65)
        // CSI 97:65;6 u (modifier = 1 + 1 (shift) + 4 (ctrl) = 6)
        assert_eq!(
            kitty_encode_key(&ks, &flags_with_flag4(), OptionAsAlt::Both, KeyEventType::Press),
            Some(b"\x1b[97:65;6u".to_vec())
        );
    }

    #[test]
    fn test_flag4_shift_a_with_alt() {
        let ks = make_keystroke(
            "a",
            Modifiers {
                alt: true,
                shift: true,
                ..Default::default()
            },
        );
        // Flag 4: CSI 97:65;4 u (modifier = 1 + 1 (shift) + 2 (alt) = 4)
        assert_eq!(
            kitty_encode_key(&ks, &flags_with_flag4(), OptionAsAlt::Both, KeyEventType::Press),
            Some(b"\x1b[97:65;4u".to_vec())
        );
    }

    #[test]
    fn test_flag4_ctrl_a_no_shift() {
        let ks = make_keystroke(
            "a",
            Modifiers {
                control: true,
                ..Default::default()
            },
        );
        // No shift, so no alternate key: CSI 97;5 u
        assert_eq!(
            kitty_encode_key(&ks, &flags_with_flag4(), OptionAsAlt::Both, KeyEventType::Press),
            Some(b"\x1b[97;5u".to_vec())
        );
    }

    // Combined Flags 1+2+4 Tests

    #[test]
    fn test_flags_1_2_4_ctrl_shift_a_press() {
        let ks = make_keystroke(
            "a",
            Modifiers {
                control: true,
                shift: true,
                ..Default::default()
            },
        );
        // All flags: CSI 97:65;6:1 u
        assert_eq!(
            kitty_encode_key(&ks, &flags_with_flags_1_2_4(), OptionAsAlt::Both, KeyEventType::Press),
            Some(b"\x1b[97:65;6:1u".to_vec())
        );
    }

    #[test]
    fn test_flags_1_2_4_ctrl_shift_a_release() {
        let ks = make_keystroke(
            "a",
            Modifiers {
                control: true,
                shift: true,
                ..Default::default()
            },
        );
        // All flags: CSI 97:65;6:3 u (release)
        assert_eq!(
            kitty_encode_key(&ks, &flags_with_flags_1_2_4(), OptionAsAlt::Both, KeyEventType::Release),
            Some(b"\x1b[97:65;6:3u".to_vec())
        );
    }

    #[test]
    fn test_flags_1_2_4_alt_shift_a_repeat() {
        let ks = make_keystroke(
            "a",
            Modifiers {
                alt: true,
                shift: true,
                ..Default::default()
            },
        );
        // All flags: CSI 97:65;4:2 u (repeat, modifier = 1+1+2 = 4)
        assert_eq!(
            kitty_encode_key(&ks, &flags_with_flags_1_2_4(), OptionAsAlt::Both, KeyEventType::Repeat),
            Some(b"\x1b[97:65;4:2u".to_vec())
        );
    }

    #[test]
    fn test_flag1_only_no_event_type() {
        let ks = make_keystroke(
            "a",
            Modifiers {
                control: true,
                ..Default::default()
            },
        );
        // Flag 1 only: CSI 97;5 u (no event type suffix)
        assert_eq!(
            kitty_encode_key(&ks, &flags_with_flag1(), OptionAsAlt::Both, KeyEventType::Repeat),
            Some(b"\x1b[97;5u".to_vec())
        );
    }

    #[test]
    fn test_flag2_arrow_with_modifiers() {
        let ks = make_keystroke(
            "up",
            Modifiers {
                control: true,
                ..Default::default()
            },
        );
        // Flag 2 with arrow key: CSI 57362;5:1 u
        assert_eq!(
            kitty_encode_key(&ks, &flags_with_flag2(), OptionAsAlt::Both, KeyEventType::Press),
            Some(b"\x1b[57362;5:1u".to_vec())
        );
    }
}
