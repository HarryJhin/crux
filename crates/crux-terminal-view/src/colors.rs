//! Color conversion from alacritty_terminal colors to GPUI Hsla.

use crux_terminal::{Color, NamedColor};
use gpui::Hsla;

/// Catppuccin Mocha palette (default theme).
const BLACK: u32 = 0x1e1e2e;
const RED: u32 = 0xf38ba8;
const GREEN: u32 = 0xa6e3a1;
const YELLOW: u32 = 0xf9e2af;
const BLUE: u32 = 0x89b4fa;
const MAGENTA: u32 = 0xcba6f7;
const CYAN: u32 = 0x94e2d5;
const WHITE: u32 = 0xcdd6f4;
const BRIGHT_BLACK: u32 = 0x585b70;
const BRIGHT_RED: u32 = 0xeba0ac;
const BRIGHT_GREEN: u32 = 0x94e2d5;
const BRIGHT_YELLOW: u32 = 0xf5e0dc;
const BRIGHT_BLUE: u32 = 0x74c7ec;
const BRIGHT_MAGENTA: u32 = 0xf5c2e7;
const BRIGHT_CYAN: u32 = 0x89dceb;
const BRIGHT_WHITE: u32 = 0xffffff;
const FOREGROUND: u32 = 0xcdd6f4;
const BACKGROUND: u32 = 0x1e1e2e;
const CURSOR: u32 = 0xf5e0dc;

/// Convert an alacritty `Color` to a GPUI `Hsla`.
pub fn color_to_hsla(color: Color) -> Hsla {
    match color {
        Color::Named(named) => named_color_to_hsla(named),
        Color::Spec(rgb) => rgb_to_hsla(rgb.r, rgb.g, rgb.b),
        Color::Indexed(idx) => indexed_color_to_hsla(idx),
    }
}

fn named_color_to_hsla(color: NamedColor) -> Hsla {
    let rgb = match color {
        NamedColor::Black => BLACK,
        NamedColor::Red => RED,
        NamedColor::Green => GREEN,
        NamedColor::Yellow => YELLOW,
        NamedColor::Blue => BLUE,
        NamedColor::Magenta => MAGENTA,
        NamedColor::Cyan => CYAN,
        NamedColor::White => WHITE,
        NamedColor::BrightBlack => BRIGHT_BLACK,
        NamedColor::BrightRed => BRIGHT_RED,
        NamedColor::BrightGreen => BRIGHT_GREEN,
        NamedColor::BrightYellow => BRIGHT_YELLOW,
        NamedColor::BrightBlue => BRIGHT_BLUE,
        NamedColor::BrightMagenta => BRIGHT_MAGENTA,
        NamedColor::BrightCyan => BRIGHT_CYAN,
        NamedColor::BrightWhite => BRIGHT_WHITE,
        NamedColor::Foreground | NamedColor::BrightForeground | NamedColor::DimForeground => {
            FOREGROUND
        }
        NamedColor::Background => BACKGROUND,
        NamedColor::Cursor => CURSOR,
        _ => FOREGROUND,
    };
    hex_to_hsla(rgb)
}

/// Convert a 256-color index to GPUI Hsla.
fn indexed_color_to_hsla(idx: u8) -> Hsla {
    match idx {
        // Standard 16 colors map to named colors.
        0 => hex_to_hsla(BLACK),
        1 => hex_to_hsla(RED),
        2 => hex_to_hsla(GREEN),
        3 => hex_to_hsla(YELLOW),
        4 => hex_to_hsla(BLUE),
        5 => hex_to_hsla(MAGENTA),
        6 => hex_to_hsla(CYAN),
        7 => hex_to_hsla(WHITE),
        8 => hex_to_hsla(BRIGHT_BLACK),
        9 => hex_to_hsla(BRIGHT_RED),
        10 => hex_to_hsla(BRIGHT_GREEN),
        11 => hex_to_hsla(BRIGHT_YELLOW),
        12 => hex_to_hsla(BRIGHT_BLUE),
        13 => hex_to_hsla(BRIGHT_MAGENTA),
        14 => hex_to_hsla(BRIGHT_CYAN),
        15 => hex_to_hsla(BRIGHT_WHITE),
        // 216 color cube (indices 16..=231).
        16..=231 => {
            let idx = idx - 16;
            let r_idx = idx / 36;
            let g_idx = (idx % 36) / 6;
            let b_idx = idx % 6;
            let r = if r_idx == 0 { 0 } else { 55 + 40 * r_idx };
            let g = if g_idx == 0 { 0 } else { 55 + 40 * g_idx };
            let b = if b_idx == 0 { 0 } else { 55 + 40 * b_idx };
            rgb_to_hsla(r, g, b)
        }
        // Grayscale ramp (indices 232..=255).
        232..=255 => {
            let v = 8 + 10 * (idx - 232);
            rgb_to_hsla(v, v, v)
        }
    }
}

fn hex_to_hsla(hex: u32) -> Hsla {
    let r = ((hex >> 16) & 0xFF) as u8;
    let g = ((hex >> 8) & 0xFF) as u8;
    let b = (hex & 0xFF) as u8;
    rgb_to_hsla(r, g, b)
}

fn rgb_to_hsla(r: u8, g: u8, b: u8) -> Hsla {
    Hsla::from(gpui::Rgba {
        r: r as f32 / 255.0,
        g: g as f32 / 255.0,
        b: b as f32 / 255.0,
        a: 1.0,
    })
}

/// Background color as GPUI Hsla.
pub fn background_hsla() -> Hsla {
    hex_to_hsla(BACKGROUND)
}

/// Foreground color as GPUI Hsla.
pub fn foreground_hsla() -> Hsla {
    hex_to_hsla(FOREGROUND)
}

/// Cursor color as GPUI Hsla.
pub fn cursor_hsla() -> Hsla {
    hex_to_hsla(CURSOR)
}
