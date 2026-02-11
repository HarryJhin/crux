---
title: "Font Discovery and CJK Fallback"
description: "Core Text API for font discovery, CJK fallback chains, Korean-first rendering, GPUI font handling, ligature support, variable fonts, box drawing and block element GPU rendering"
date: 2026-02-12
phase: [1]
topics: [fonts, cjk, core-text, fallback, ligatures]
status: final
related:
  - terminal-architecture.md
  - ../platform/ime-clipboard.md
---

# Font Discovery and CJK Fallback

> 작성일: 2026-02-12
> 목적: Crux 터미널의 폰트 시스템 설계 — macOS Core Text 기반 폰트 탐색, CJK (한중일) 폴백 체인, GPUI 연동, 박스 드로잉 문자 GPU 렌더링

---

## 목차

1. [개요](#1-개요)
2. [macOS Core Text Font System](#2-macos-core-text-font-system)
3. [CJK Font Fallback Chain](#3-cjk-font-fallback-chain)
4. [GPUI Font Handling](#4-gpui-font-handling)
5. [Font Metrics for Terminals](#5-font-metrics-for-terminals)
6. [Ligature Support](#6-ligature-support)
7. [Variable Fonts](#7-variable-fonts)
8. [Box Drawing and Block Elements](#8-box-drawing-and-block-elements)
9. [Crux Implementation Recommendations](#9-crux-implementation-recommendations)

---

## 1. 개요

Terminal font rendering is uniquely constrained:

- **Monospace grid**: Every cell must be exactly the same width
- **CJK wide characters**: Korean/Chinese/Japanese occupy exactly 2 cells
- **Box drawing**: U+2500–U+257F must connect seamlessly between cells
- **Mixed scripts**: A single line may contain ASCII, Korean, emoji, and box drawing
- **Performance**: Glyph rasterization must not block frame rendering

The font system must solve:
1. **Discovery**: Find available fonts on the system
2. **Fallback**: When the primary font lacks a glyph, find one that has it
3. **Metrics**: Calculate cell width and height consistently
4. **Rendering**: Rasterize glyphs to the GPU texture atlas

On macOS, Core Text is the authoritative font system. All other approaches ultimately call into it.

---

## 2. macOS Core Text Font System

### Core Text Architecture

```
User Request ("JetBrains Mono", 14pt)
    │
    ▼
CTFontCreateWithName()        → Primary font
    │
    ▼
CTFontCopyDefaultCascadeListForLanguages()  → Fallback chain
    │
    ▼
CTFontCreateForString()       → Per-string font matching
    │
    ▼
CTFontGetGlyphsForCharacters() → Glyph IDs
    │
    ▼
CTFontDrawGlyphs()            → Rasterization
```

### Key Core Text APIs

#### Font Creation

```rust
use core_text::font as ct_font;

// Create font by name and size
let font = ct_font::new_from_name("JetBrains Mono", 14.0)
    .expect("Font not found");

// Create font from font descriptor
let descriptor = ct_font::new_from_descriptor(&descriptor, 14.0);
```

#### Font Discovery

```rust
use core_text::font_collection::CTFontCollection;
use core_text::font_descriptor::CTFontDescriptor;

// Get all available font families
let collection = CTFontCollection::create_for_all_families();
let descriptors = collection.get_descriptors();

// Search for a specific font
let descriptor = CTFontDescriptor::new_from_attributes(&attributes);
let matching = descriptor.create_matching_font_descriptors();
```

#### Fallback Chain

```rust
use core_foundation::string::CFString;
use core_foundation::array::CFArray;

// Get locale-aware fallback chain
let languages = CFArray::from_CFTypes(&[
    CFString::new("ko"),  // Korean first
    CFString::new("ja"),  // Then Japanese
    CFString::new("zh"),  // Then Chinese
]);

let cascade_list = ct_font.copy_default_cascade_list_for_languages(&languages);
// Returns: [Apple SD Gothic Neo, Hiragino Sans, PingFang SC, ...]
```

This is the critical API: `CTFontCopyDefaultCascadeListForLanguages` returns an ordered list of fallback fonts based on the user's language preferences. By passing `ko` first, Korean fonts are prioritized.

#### Per-String Font Matching

```rust
// For a specific string, find the best font
let matched_font = ct_font.create_for_string("한글", 0..4);
// Returns: A CTFont that can render "한글"
```

### Rust Crates for Core Text

| Crate | Description | Notes |
|-------|-------------|-------|
| `core-text` | Safe Rust bindings to Core Text | Stable, used by many projects |
| `core-foundation` | CF types (CFString, CFArray, etc.) | Required by core-text |
| `font-kit` | Cross-platform font discovery | Uses Core Text on macOS |
| `cosmic-text` | Text shaping + layout | Full text engine, may be overkill |

**Recommendation**: Use `font-kit` for discovery (simpler API) and `core-text` directly for the fallback cascade.

---

## 3. CJK Font Fallback Chain

### Korean-First Fallback Chain

For a Korean-focused terminal, the recommended fallback order:

```
1. [User-configured font]         e.g., "JetBrains Mono"
2. Apple SD Gothic Neo             Korean — bundled with macOS
3. PingFang SC                     Simplified Chinese — bundled with macOS
4. Hiragino Sans                   Japanese — bundled with macOS
5. Noto Sans Mono CJK KR           Korean — if installed (Homebrew: font-noto-sans-cjk-kr)
6. Apple Color Emoji               Emoji — bundled with macOS
7. LastResort                      Unicode fallback — bundled with macOS
```

### Why This Order Matters

The Han Unification problem: CJK characters share Unicode code points but have different preferred glyphs in each locale. For example, U+9AA8 (骨, "bone") has different standard forms in Korean, Japanese, and Chinese.

By placing Korean fonts first in the fallback chain:
- **U+AC00–U+D7AF** (Hangul Syllables): Rendered by Apple SD Gothic Neo
- **U+4E00–U+9FFF** (CJK Unified Ideographs): Rendered with Korean glyph variants
- **U+3040–U+309F** (Hiragana): Falls through to Hiragino Sans

### macOS Bundled CJK Fonts

| Font | Language | Coverage | Weight Range |
|------|----------|----------|-------------|
| Apple SD Gothic Neo | Korean | Hangul + CJK | Thin–Heavy (9 weights) |
| PingFang SC/TC/HK | Chinese (Simplified/Traditional) | CJK + Chinese-specific | Thin–Semibold (6 weights) |
| Hiragino Sans | Japanese | Kana + CJK | W0–W9 (10 weights) |
| Apple Color Emoji | Emoji | Full Unicode emoji | N/A (bitmap) |

These are always available on macOS 13+ — no installation required.

### Configuration

```toml
[font]
family = "JetBrains Mono"
size = 14.0

[font.fallback]
# Explicit CJK fallback chain (overrides system default)
families = [
    "Apple SD Gothic Neo",    # Korean
    "PingFang SC",            # Chinese
    "Hiragino Sans",          # Japanese
    "Noto Sans Mono CJK KR",  # Extra Korean (if installed)
]

# Language priority for auto-discovery
# Used when families list is empty — calls CTFontCopyDefaultCascadeListForLanguages
locale_priority = ["ko", "ja", "zh-Hans"]
```

---

## 4. GPUI Font Handling

### GPUI's MacTextSystem

GPUI uses Core Text internally via its `MacTextSystem` backend. Key types:

```rust
// GPUI's text system API
pub trait PlatformTextSystem {
    fn font_id(&self, font: &Font) -> Result<FontId>;
    fn typographic_bounds(&self, font_id: FontId, font_size: Pixels) -> Result<Bounds<Pixels>>;
    fn advance(&self, font_id: FontId, font_size: Pixels, ch: char) -> Result<Size<Pixels>>;
    fn layout_line(&self, text: &str, font_size: Pixels, runs: &[FontRun]) -> LineLayout;
    fn rasterize_glyph(&self, params: &RenderGlyphParams) -> Option<(Size<DevicePixels>, Vec<u8>)>;
}
```

### How GPUI Renders Text

1. **Font loading**: GPUI loads fonts by family name via Core Text
2. **Shaping**: Core Text shapes the text (handles ligatures, combining marks)
3. **Layout**: `layout_line()` returns glyph positions and font runs
4. **Rasterization**: Glyphs are rasterized to a GPU texture atlas
5. **Drawing**: The atlas is sampled during Metal rendering

### Using GPUI for Crux Terminal Text

For terminal rendering, Crux should use GPUI's text system rather than calling Core Text directly:

```rust
// In CruxTerminalElement::paint()
fn paint_cell(
    &self,
    cx: &mut WindowContext,
    cell: &RenderableCell,
    origin: Point<Pixels>,
) {
    let font = self.font_for_cell(cell, cx);
    let text = SharedString::from(cell.c.to_string());

    // Use GPUI's text rendering
    let line = cx.text_system().layout_line(
        &text,
        self.font_size,
        &[FontRun { len: text.len(), font_id: font }],
    );

    line.paint(origin, self.line_height, cx);
}
```

### Glyph Atlas

GPUI maintains a glyph atlas (texture containing rasterized glyphs). Important for terminal performance:

- **Atlas size**: GPUI auto-manages atlas growth
- **Cache invalidation**: Font size change requires atlas rebuild
- **Emoji**: Rendered as color bitmaps (Apple Color Emoji)
- **Subpixel positioning**: GPUI supports fractional glyph positions

---

## 5. Font Metrics for Terminals

### Cell Size Calculation

The terminal grid requires exactly uniform cell dimensions:

```rust
struct CellSize {
    width: Pixels,   // Width of a single cell
    height: Pixels,  // Height of a single cell (= line_height)
}

fn calculate_cell_size(font_id: FontId, font_size: Pixels, cx: &WindowContext) -> CellSize {
    let text_system = cx.text_system();
    let metrics = text_system.font_metrics(font_id);

    // Cell width = advance width of ASCII character
    let advance = text_system.advance(font_id, font_size, 'M')
        .unwrap_or(Size { width: font_size * 0.6, height: font_size });

    // Line height = ascent + descent + leading (+ optional extra)
    let line_height = (metrics.ascent + metrics.descent + metrics.leading) * font_size;

    CellSize {
        width: advance.width,
        height: line_height.ceil(),
    }
}
```

### CJK Width: The `ic` Metric Problem

CJK characters are conventionally 2 cells wide. But the actual glyph width varies:

- **Naive approach**: `cjk_width = 2 * cell_width` — This is wrong!
- **Correct approach**: Use the font's `ic` (ideographic character) width metric

Ghostty discovered this the hard way: using `2 * em-width` caused CJK characters to be slightly misaligned. The `ic` metric from the font gives the actual ideographic advance width.

```rust
fn cjk_cell_width(font_id: FontId, font_size: Pixels, cx: &WindowContext) -> Pixels {
    let text_system = cx.text_system();

    // Try to get the 'ic' (ideographic character) advance
    // This is the correct width for CJK full-width characters
    let ic_width = text_system.advance(font_id, font_size, '水')
        .map(|s| s.width);

    // Fallback: use 2x cell width (less accurate)
    let fallback = text_system.advance(font_id, font_size, 'M')
        .map(|s| s.width * 2.0);

    ic_width.or(fallback).unwrap_or(font_size)
}
```

### Unicode Width Detection

For each character, determine if it's narrow (1 cell) or wide (2 cells):

```rust
use unicode_width::UnicodeWidthChar;

fn char_width(c: char) -> usize {
    // unicode-width crate handles the standard correctly
    c.width().unwrap_or(0)
}

// Examples:
assert_eq!(char_width('A'), 1);    // ASCII
assert_eq!(char_width('한'), 2);   // Hangul
assert_eq!(char_width('中'), 2);   // CJK
assert_eq!(char_width('→'), 1);    // Arrow (ambiguous — treat as narrow)
```

**Crate**: `unicode-width = "0.2"` — Implements UAX #11 (East Asian Width)

### Ambiguous Width Characters

Some Unicode characters (e.g., `→`, `●`, `α`) have "ambiguous" width — they're wide in East Asian contexts and narrow in Western contexts. Terminals handle this differently:

| Terminal | Ambiguous Width | Configurable |
|----------|----------------|--------------|
| Alacritty | Narrow (1) | No |
| Kitty | Narrow (1) | No |
| Ghostty | Narrow (1) | No |
| WezTerm | Narrow (1) | Yes |
| iTerm2 | Narrow (1) | Yes (per-profile) |

**Recommendation for Crux**: Default to narrow (1 cell). Make configurable for users who need wide ambiguous characters (common in Korean/Japanese terminal workflows).

---

## 6. Ligature Support

### Ligatures in Terminal Context

Programming ligatures (e.g., `->` → `→`, `!=` → `≠`) are available in fonts like:
- Fira Code
- JetBrains Mono
- Cascadia Code
- Iosevka

### Should Terminals Support Ligatures?

**Pros**: Aesthetic, popular with developers
**Cons**: Break column alignment assumptions, confuse cursor positioning, slow rendering

| Terminal | Ligatures | Default |
|----------|-----------|---------|
| Alacritty | No (explicitly rejected) | N/A |
| Kitty | Yes | Disabled |
| Ghostty | Yes | Disabled |
| WezTerm | Yes | Disabled |
| iTerm2 | Yes | Disabled |

### Implementation Approach

```toml
# Config
[font]
ligatures = false  # Default: disabled

# Per-font override
[[font.ligatures_override]]
family = "Fira Code"
enabled = true
```

When enabled, use GPUI's text shaping (which calls Core Text's CTRunGetGlyphs). Core Text handles ligature substitution automatically via OpenType GSUB tables.

When disabled, render each character individually without text shaping.

```rust
fn layout_cell_text(&self, text: &str, cx: &WindowContext) -> LineLayout {
    if self.config.font.ligatures {
        // Full text shaping — Core Text handles ligatures
        cx.text_system().layout_line(text, self.font_size, &runs)
    } else {
        // Character-by-character — no ligature substitution
        // Layout each char independently
        text.chars()
            .map(|c| cx.text_system().layout_line(&c.to_string(), self.font_size, &runs))
            .collect()
    }
}
```

**Recommendation for Crux**: Support ligatures but disable by default. Terminal users expect exact column alignment.

---

## 7. Variable Fonts

### Overview

Variable fonts (OpenType 1.8+) contain multiple styles (weight, width, slant) in a single file. macOS fully supports them via Core Text.

### Common Variable Font Axes

| Axis | Tag | Range | Example |
|------|-----|-------|---------|
| Weight | `wght` | 100–900 | Thin (100) to Black (900) |
| Width | `wdth` | 75–125 | Condensed to Expanded |
| Slant | `slnt` | -90–0 | Upright to fully slanted |
| Italic | `ital` | 0–1 | Roman to italic |

### Terminal Use Case

Variable fonts allow fine-tuned weight for:
- **Bold text** (`SGR 1`): Use `wght=700` instead of a separate bold font file
- **Dim text** (`SGR 2`): Use `wght=300` for a lighter appearance
- **Custom weight**: Let users set exact bold/dim weights

```toml
[font]
family = "Inter Variable"
weight = 400       # Normal weight
bold_weight = 700  # SGR 1 weight
dim_weight = 300   # SGR 2 weight
```

### Core Text Variable Font API

```rust
use core_text::font as ct_font;

// Create variable font with specific weight
let base_font = ct_font::new_from_name("Inter", 14.0)?;
let bold_font = base_font.create_copy_with_attributes(
    14.0,
    None,
    &font_descriptor_with_weight(700),
)?;
```

**Recommendation**: Support variable fonts for weight axis only. Other axes (width, slant) are not useful for terminal rendering.

---

## 8. Box Drawing and Block Elements

### The Problem

Box drawing characters (U+2500–U+257F) and block elements (U+2580–U+259F) must:
1. **Connect seamlessly** between adjacent cells (no gaps)
2. **Fill their cell exactly** (not be scaled/offset by font metrics)
3. **Look crisp** at all sizes (no anti-aliasing blur at cell boundaries)

Font-rendered box drawing often fails criteria 1 and 2 because font metrics vary between the primary font and the fallback font that provides these glyphs.

### Solution: Custom GPU Rendering

All modern terminals render box drawing procedurally rather than using font glyphs:

| Terminal | Box Drawing | Block Elements |
|----------|------------|----------------|
| Alacritty | Custom rendering | Custom rendering |
| Kitty | Custom rendering | Custom rendering |
| Ghostty | Custom rendering | Custom rendering |
| WezTerm | Custom rendering | Custom rendering |

### Characters to Render Procedurally

#### Box Drawing (U+2500–U+257F)

```
Light:  ─ │ ┌ ┐ └ ┘ ├ ┤ ┬ ┴ ┼
Heavy:  ━ ┃ ┏ ┓ ┗ ┛ ┣ ┫ ┳ ┻ ╋
Double: ═ ║ ╔ ╗ ╚ ╝ ╠ ╣ ╦ ╩ ╬
Mixed:  ╒ ╓ ╕ ╖ ╘ ╙ ╛ ╜ ╞ ╟ ╡ ╢ ╤ ╥ ╧ ╨ ╪ ╫
Dash:   ┄ ┅ ┆ ┇ ┈ ┉ ┊ ┋
Round:  ╭ ╮ ╯ ╰
```

#### Block Elements (U+2580–U+259F)

```
▀ ▁ ▂ ▃ ▄ ▅ ▆ ▇ █ ▉ ▊ ▋ ▌ ▍ ▎ ▏
▐ ░ ▒ ▓ ▔ ▕ ▖ ▗ ▘ ▙ ▚ ▛ ▜ ▝ ▞ ▟
```

### Rendering Algorithm

```rust
fn render_box_drawing(
    c: char,
    cell_width: f32,
    cell_height: f32,
    fg_color: Color,
    line_width: f32,  // 1px for light, 2-3px for heavy
) -> Vec<Line> {
    let cx = cell_width / 2.0;   // Center X
    let cy = cell_height / 2.0;  // Center Y

    match c {
        '─' => vec![
            Line::horizontal(0.0, cell_width, cy, line_width)
        ],
        '│' => vec![
            Line::vertical(cx, 0.0, cell_height, line_width)
        ],
        '┌' => vec![
            Line::horizontal(cx, cell_width, cy, line_width),
            Line::vertical(cx, cy, cell_height, line_width),
        ],
        '┼' => vec![
            Line::horizontal(0.0, cell_width, cy, line_width),
            Line::vertical(cx, 0.0, cell_height, line_width),
        ],
        // ... etc for all 128 box drawing characters
        _ => vec![],
    }
}

fn render_block_element(
    c: char,
    cell_width: f32,
    cell_height: f32,
    fg_color: Color,
) -> Rect {
    match c {
        '█' => Rect::new(0.0, 0.0, cell_width, cell_height),           // Full block
        '▀' => Rect::new(0.0, 0.0, cell_width, cell_height / 2.0),     // Upper half
        '▄' => Rect::new(0.0, cell_height / 2.0, cell_width, cell_height), // Lower half
        '▌' => Rect::new(0.0, 0.0, cell_width / 2.0, cell_height),     // Left half
        '▐' => Rect::new(cell_width / 2.0, 0.0, cell_width, cell_height),  // Right half
        '▁' => Rect::new(0.0, cell_height * 7.0/8.0, cell_width, cell_height), // 1/8 block
        // ... fractional blocks for ▂▃▄▅▆▇
        _ => Rect::ZERO,
    }
}
```

### Integration with GPUI Element

In `CruxTerminalElement::paint()`:

```rust
fn paint_cell(&self, cx: &mut WindowContext, cell: &RenderableCell, origin: Point<Pixels>) {
    let c = cell.c;

    if is_box_drawing(c) || is_block_element(c) {
        // Custom GPU rendering — bypass font system
        self.paint_box_drawing(cx, c, origin, cell.fg);
    } else {
        // Normal text rendering via GPUI text system
        self.paint_text(cx, cell, origin);
    }
}

fn is_box_drawing(c: char) -> bool {
    ('\u{2500}'..='\u{257F}').contains(&c)
}

fn is_block_element(c: char) -> bool {
    ('\u{2580}'..='\u{259F}').contains(&c)
}
```

### Braille Patterns (U+2800–U+28FF)

Some terminals also render Braille patterns procedurally for pixel-level graphics (used by tools like `gnuplot`, `spark`, `timg`):

```
⠀⠁⠂⠃⠄⠅⠆⠇⡀⡁⡂⡃⡄⡅⡆⡇
⠈⠉⠊⠋⠌⠍⠎⠏⡈⡉⡊⡋⡌⡍⡎⡏
...
```

Each Braille character is a 2×4 dot pattern. Procedural rendering ensures pixel-perfect dots.

**Recommendation**: Consider for Phase 4 (graphics protocols).

---

## 9. Crux Implementation Recommendations

### Phase 1 — Core Font System

1. **Use GPUI's text system** for all text rendering (do not call Core Text directly for rendering)
2. **Font discovery**: Accept font family name in config, resolve via GPUI
3. **CJK fallback**: Set up `CTFontCopyDefaultCascadeListForLanguages` with Korean-first priority
4. **Cell size**: Calculate from primary font's advance width and line metrics
5. **Wide char detection**: Use `unicode-width` crate for UAX #11 compliance
6. **Box drawing**: Custom rendering for U+2500–U+257F, U+2580–U+259F

### Phase 1+ — Enhanced

7. **`ic` metric**: Use ideographic character width instead of `2 * em-width`
8. **Bold/italic**: Use separate font lookups or variable font weight axis
9. **Emoji rendering**: Ensure Apple Color Emoji fallback works via Core Text

### Phase 3+ — Advanced

10. **Ligature support**: Configurable, disabled by default
11. **Variable font weight**: Custom bold/dim weights
12. **Ambiguous width config**: User-configurable East Asian ambiguous width
13. **Braille pattern rendering**: Procedural dot rendering

### Key Crate Dependencies

```toml
[dependencies]
unicode-width = "0.2"    # East Asian Width detection
font-kit = "0.14"        # Font discovery (wraps Core Text)

# Only if calling Core Text directly (beyond GPUI):
# core-text = "20.1"
# core-foundation = "0.10"
```

---

## Sources

- [Core Text Programming Guide](https://developer.apple.com/library/archive/documentation/StringsTextFonts/Conceptual/CoreText_Programming/Overview/Overview.html) — Apple official docs
- [CTFontCopyDefaultCascadeListForLanguages](https://developer.apple.com/documentation/coretext/1509991-ctfontcopydefaultcascadelistforl) — Locale-aware fallback API
- [UAX #11: East Asian Width](https://www.unicode.org/reports/tr11/) — Unicode standard for character width
- [font-kit crate](https://docs.rs/font-kit/latest/font_kit/) — Cross-platform font discovery
- [Ghostty Font Rendering](https://mitchellh.com/writing/ghostty-devlog-003) — Mitchell Hashimoto's devlog on `ic` metric discovery
- [Alacritty Box Drawing](https://github.com/alacritty/alacritty/blob/master/alacritty/src/renderer/rects.rs) — Procedural box drawing implementation
- [unicode-width crate](https://docs.rs/unicode-width/latest/unicode_width/) — UAX #11 implementation
- [OpenType Variable Fonts](https://learn.microsoft.com/en-us/typography/opentype/spec/otvaroverview) — Variable font specification
