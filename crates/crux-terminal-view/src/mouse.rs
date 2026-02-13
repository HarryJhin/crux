//! Mouse event encoding for terminal applications.
//!
//! When a TUI app (vim, tmux, htop) enables mouse tracking via DECSET escape
//! sequences, the terminal must encode mouse events as SGR escape sequences
//! and write them to the PTY instead of handling selection locally.
//!
//! SGR format: `CSI < Pb ; Px ; Py M` (press) or `CSI < Pb ; Px ; Py m` (release)
//! where Pb = button + modifiers, Px = 1-based column, Py = 1-based row.

use std::io::Write;

use crux_terminal::{Point, TermMode};
use gpui::{Modifiers, MouseButton};

/// Encode a mouse event as an SGR escape sequence.
///
/// `button` is the Cb byte (button number + modifier bits).
/// `point` is the terminal grid position (0-based, converted to 1-based for output).
/// `pressed` selects 'M' (press) or 'm' (release) suffix.
pub fn sgr_mouse_report(button: u8, point: Point, pressed: bool) -> Vec<u8> {
    let suffix = if pressed { 'M' } else { 'm' };
    // SGR uses 1-based coordinates.
    let col = point.column.0 + 1;
    let row = point.line.0 + 1;
    let mut buf = Vec::with_capacity(16);
    // infallible: writing to Vec<u8>
    write!(buf, "\x1b[<{button};{col};{row}{suffix}").unwrap();
    buf
}

/// Map a GPUI MouseButton to the SGR Cb base value.
///
/// If `is_motion` is true, adds the motion flag (+32) per xterm spec.
pub fn mouse_button_to_cb(button: MouseButton, is_motion: bool) -> u8 {
    let base = match button {
        MouseButton::Left => 0,
        MouseButton::Middle => 1,
        MouseButton::Right => 2,
        // Forward/back buttons map to button 6/7 in some implementations,
        // but standard SGR uses 0 as fallback.
        _ => 0,
    };
    if is_motion {
        base + 32
    } else {
        base
    }
}

/// Compute modifier bits for the SGR Cb byte from GPUI modifiers.
///
/// Bit layout: Shift = +4, Alt/Meta = +8, Ctrl = +16.
pub fn modifier_bits(modifiers: &Modifiers) -> u8 {
    let mut bits: u8 = 0;
    if modifiers.shift {
        bits += 4;
    }
    if modifiers.alt {
        bits += 8;
    }
    if modifiers.control {
        bits += 16;
    }
    bits
}

/// Encode a scroll wheel event as an SGR Cb value.
///
/// Scroll up = 64, scroll down = 65. These already include the "button 4/5"
/// encoding per the xterm mouse protocol.
pub fn scroll_button(up: bool) -> u8 {
    if up {
        64
    } else {
        65
    }
}

/// Check if any mouse reporting mode is active, respecting Shift bypass.
///
/// When Shift is held, mouse events should fall through to the terminal's
/// local selection handler (the "Shift bypass" convention used by all major
/// terminal emulators).
pub fn mouse_mode_active(mode: TermMode, shift: bool) -> bool {
    mode.intersects(TermMode::MOUSE_MODE) && !shift
}

#[cfg(test)]
mod tests {
    use super::*;
    use crux_terminal::{Column, Line};

    #[test]
    fn test_sgr_left_click_press() {
        let point = Point::new(Line(4), Column(9)); // 0-based row 4, col 9
        let report = sgr_mouse_report(0, point, true);
        // Should be 1-based: col 10, row 5
        assert_eq!(report, b"\x1b[<0;10;5M");
    }

    #[test]
    fn test_sgr_left_click_release() {
        let point = Point::new(Line(4), Column(9));
        let report = sgr_mouse_report(0, point, false);
        assert_eq!(report, b"\x1b[<0;10;5m");
    }

    #[test]
    fn test_sgr_right_click() {
        let point = Point::new(Line(0), Column(0));
        let report = sgr_mouse_report(2, point, true);
        assert_eq!(report, b"\x1b[<2;1;1M");
    }

    #[test]
    fn test_sgr_scroll_up() {
        let point = Point::new(Line(4), Column(9));
        let cb = scroll_button(true);
        let report = sgr_mouse_report(cb, point, true);
        assert_eq!(report, b"\x1b[<64;10;5M");
    }

    #[test]
    fn test_sgr_scroll_down() {
        let point = Point::new(Line(4), Column(9));
        let cb = scroll_button(false);
        let report = sgr_mouse_report(cb, point, true);
        assert_eq!(report, b"\x1b[<65;10;5M");
    }

    #[test]
    fn test_sgr_with_shift_modifier() {
        let point = Point::new(Line(4), Column(9));
        let cb = 0 + modifier_bits(&Modifiers {
            shift: true,
            ..Default::default()
        });
        let report = sgr_mouse_report(cb, point, true);
        // Shift adds 4 to Cb
        assert_eq!(report, b"\x1b[<4;10;5M");
    }

    #[test]
    fn test_sgr_with_ctrl_modifier() {
        let point = Point::new(Line(0), Column(0));
        let cb = 0 + modifier_bits(&Modifiers {
            control: true,
            ..Default::default()
        });
        let report = sgr_mouse_report(cb, point, true);
        // Ctrl adds 16 to Cb
        assert_eq!(report, b"\x1b[<16;1;1M");
    }

    #[test]
    fn test_mouse_button_to_cb_left() {
        assert_eq!(mouse_button_to_cb(MouseButton::Left, false), 0);
    }

    #[test]
    fn test_mouse_button_to_cb_middle() {
        assert_eq!(mouse_button_to_cb(MouseButton::Middle, false), 1);
    }

    #[test]
    fn test_mouse_button_to_cb_right() {
        assert_eq!(mouse_button_to_cb(MouseButton::Right, false), 2);
    }

    #[test]
    fn test_mouse_button_to_cb_motion() {
        assert_eq!(mouse_button_to_cb(MouseButton::Left, true), 32);
    }

    #[test]
    fn test_modifier_bits_none() {
        assert_eq!(modifier_bits(&Modifiers::default()), 0);
    }

    #[test]
    fn test_modifier_bits_all() {
        let mods = Modifiers {
            shift: true,
            alt: true,
            control: true,
            ..Default::default()
        };
        assert_eq!(modifier_bits(&mods), 4 + 8 + 16);
    }

    #[test]
    fn test_mouse_mode_active_with_mouse_mode() {
        assert!(mouse_mode_active(TermMode::MOUSE_REPORT_CLICK, false));
        assert!(mouse_mode_active(TermMode::MOUSE_DRAG, false));
        assert!(mouse_mode_active(TermMode::MOUSE_MOTION, false));
    }

    #[test]
    fn test_mouse_mode_active_no_mode() {
        assert!(!mouse_mode_active(TermMode::empty(), false));
    }

    #[test]
    fn test_mouse_mode_active_shift_bypass() {
        // Shift held: mouse mode should be bypassed for local selection.
        assert!(!mouse_mode_active(TermMode::MOUSE_REPORT_CLICK, true));
        assert!(!mouse_mode_active(TermMode::MOUSE_DRAG, true));
    }

    #[test]
    fn test_large_coordinates() {
        // SGR supports coordinates >223 (unlike X10 encoding).
        let point = Point::new(Line(299), Column(499)); // 0-based
        let report = sgr_mouse_report(0, point, true);
        assert_eq!(report, b"\x1b[<0;500;300M");
    }
}
