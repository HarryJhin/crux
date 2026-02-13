//! Color conversion from alacritty_terminal colors to GPUI Hsla.

use crux_terminal::{Color, NamedColor};
use gpui::Hsla;

/// Catppuccin Mocha palette (default theme).
/// These constants serve as fallback defaults.
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

#[cfg(test)]
mod tests {
    use super::*;

    // Helper to extract approximate RGB from Hsla for assertions.
    fn hsla_to_rgb_u8(color: Hsla) -> (u8, u8, u8) {
        let rgba: gpui::Rgba = color.into();
        (
            (rgba.r * 255.0).round() as u8,
            (rgba.g * 255.0).round() as u8,
            (rgba.b * 255.0).round() as u8,
        )
    }

    #[test]
    fn test_named_black() {
        let hsla = named_color_to_hsla(NamedColor::Black);
        let (r, g, b) = hsla_to_rgb_u8(hsla);
        assert_eq!((r, g, b), (0x1e, 0x1e, 0x2e));
    }

    #[test]
    fn test_named_foreground_variants() {
        // Foreground, BrightForeground, DimForeground should all map to FOREGROUND
        let fg = hsla_to_rgb_u8(named_color_to_hsla(NamedColor::Foreground));
        let bright_fg = hsla_to_rgb_u8(named_color_to_hsla(NamedColor::BrightForeground));
        let dim_fg = hsla_to_rgb_u8(named_color_to_hsla(NamedColor::DimForeground));
        assert_eq!(fg, bright_fg);
        assert_eq!(fg, dim_fg);
    }

    #[test]
    fn test_indexed_standard_colors_match_named() {
        // Indexed 0-15 should produce same colors as the corresponding named colors
        assert_eq!(
            hsla_to_rgb_u8(indexed_color_to_hsla(0)),
            hsla_to_rgb_u8(named_color_to_hsla(NamedColor::Black))
        );
        assert_eq!(
            hsla_to_rgb_u8(indexed_color_to_hsla(1)),
            hsla_to_rgb_u8(named_color_to_hsla(NamedColor::Red))
        );
        assert_eq!(
            hsla_to_rgb_u8(indexed_color_to_hsla(7)),
            hsla_to_rgb_u8(named_color_to_hsla(NamedColor::White))
        );
        assert_eq!(
            hsla_to_rgb_u8(indexed_color_to_hsla(8)),
            hsla_to_rgb_u8(named_color_to_hsla(NamedColor::BrightBlack))
        );
        assert_eq!(
            hsla_to_rgb_u8(indexed_color_to_hsla(15)),
            hsla_to_rgb_u8(named_color_to_hsla(NamedColor::BrightWhite))
        );
    }

    #[test]
    fn test_256_color_cube_corners() {
        // Index 16 = (0,0,0) -> rgb(0, 0, 0)
        let (r, g, b) = hsla_to_rgb_u8(indexed_color_to_hsla(16));
        assert_eq!((r, g, b), (0, 0, 0));

        // Index 231 = (5,5,5) -> rgb(255, 255, 255)
        let (r, g, b) = hsla_to_rgb_u8(indexed_color_to_hsla(231));
        assert_eq!((r, g, b), (255, 255, 255));

        // Index 196 = (5,0,0) -> rgb(255, 0, 0) pure red
        let (r, g, b) = hsla_to_rgb_u8(indexed_color_to_hsla(196));
        assert_eq!((r, g, b), (255, 0, 0));
    }

    #[test]
    fn test_256_color_cube_math() {
        // Index 16 + r*36 + g*6 + b where each component is 0-5
        // For r=1, g=2, b=3: index = 16 + 36 + 12 + 3 = 67
        // r=1 -> 95, g=2 -> 135, b=3 -> 175
        let (r, g, b) = hsla_to_rgb_u8(indexed_color_to_hsla(67));
        assert_eq!((r, g, b), (95, 135, 175));
    }

    #[test]
    fn test_grayscale_ramp() {
        // Index 232 = darkest gray: 8 + 10*(232-232) = 8
        let (r, g, b) = hsla_to_rgb_u8(indexed_color_to_hsla(232));
        assert_eq!(r, 8);
        assert_eq!(g, 8);
        assert_eq!(b, 8);

        // Index 255 = lightest gray: 8 + 10*(255-232) = 8 + 230 = 238
        let (r, g, b) = hsla_to_rgb_u8(indexed_color_to_hsla(255));
        assert_eq!(r, 238);
        assert_eq!(g, 238);
        assert_eq!(b, 238);
    }

    #[test]
    fn test_rgb_spec_conversion() {
        use alacritty_terminal::vte::ansi::Rgb;
        let color = Color::Spec(Rgb {
            r: 128,
            g: 64,
            b: 32,
        });
        let hsla = color_to_hsla(color);
        let (r, g, b) = hsla_to_rgb_u8(hsla);
        // Allow +/-1 for floating point conversion
        assert!((r as i16 - 128).abs() <= 1, "r={}", r);
        assert!((g as i16 - 64).abs() <= 1, "g={}", g);
        assert!((b as i16 - 32).abs() <= 1, "b={}", b);
    }

    #[test]
    fn test_hex_to_hsla_roundtrip() {
        // Pure white
        let (r, g, b) = hsla_to_rgb_u8(hex_to_hsla(0xFFFFFF));
        assert_eq!((r, g, b), (255, 255, 255));

        // Pure black
        let (r, g, b) = hsla_to_rgb_u8(hex_to_hsla(0x000000));
        assert_eq!((r, g, b), (0, 0, 0));
    }

    #[test]
    fn test_background_foreground_cursor_helpers() {
        // Just verify they return valid colors without panicking
        let _ = background_hsla();
        let _ = foreground_hsla();
        let _ = cursor_hsla();

        // Background and foreground should be different
        let bg = hsla_to_rgb_u8(background_hsla());
        let fg = hsla_to_rgb_u8(foreground_hsla());
        assert_ne!(
            bg, fg,
            "background and foreground should be different colors"
        );
    }
}
