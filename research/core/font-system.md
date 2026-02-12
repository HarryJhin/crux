---
title: "Font Discovery and CJK Fallback"
description: "Core Text API for font discovery, CJK fallback chains, Korean-first rendering, GPUI font handling, ligature support, variable fonts, box drawing and block element GPU rendering, Nerd Fonts PUA, Powerline symbols, emoji rendering, texture atlas, font shaping performance"
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
10. [Built-in Box Drawing Implementation Patterns](#10-built-in-box-drawing-implementation-patterns)
11. [Nerd Fonts and Private Use Area (PUA)](#11-nerd-fonts-and-private-use-area-pua)
12. [Powerline Symbol Rendering](#12-powerline-symbol-rendering)
13. [Emoji Rendering Details](#13-emoji-rendering-details)
14. [GPUI Text Rendering Architecture](#14-gpui-text-rendering-architecture)
15. [Font Shaping Performance](#15-font-shaping-performance)

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

## 10. Built-in Box Drawing Implementation Patterns

### Production Terminal Implementations

All modern GPU terminals have migrated from font-based box drawing to procedural rendering. This section documents real-world implementation patterns.

#### Alacritty (commit f717710)

Alacritty moved box-drawing from font rendering to built-in GPU rendering for pixel-perfect alignment.

**Implementation location**: `alacritty/src/renderer/rects.rs`

**Approach**: Procedural line/rect drawing
- Light lines: 1px strokes
- Heavy lines: 2-3px strokes
- Double lines: Two parallel 1px strokes with gap
- Coordinates calculated from cell boundaries
- Rendered as GPU primitives (not textured quads)

#### Ghostty (`src/font/sprite/Box.zig`)

Comprehensive sprite renderer that generates pixel-perfect box-drawing characters.

**Coverage**: Full U+2500-U+259F range plus legacy computing symbols (U+1FB00-U+1FBFF)

**Architecture**:
- Sprite-based rendering (pre-rasterized to texture)
- Covers box drawing, block elements, and legacy computing
- Pixel-perfect alignment via integer coordinate math
- Handles all line weights (light/heavy/double)

#### Kitty (commit 533688a)

Added rounded corners (╭╮╯╰) via built-in rendering.

**Discussion context**: Issue #7680 discusses pixel-perfect alignment challenges

**Key insight**: Font-based rounded corners had inconsistent arc rendering across different fonts. Built-in rendering ensures consistent appearance.

#### Adobe Reference

`adobe-type-tools/box-drawing` on GitHub provides canonical reference implementation.

**Purpose**: Reference for font designers, but also serves as algorithm reference for terminal authors

**Coverage**: Complete U+2500-U+257F specification with mathematical definitions

### Industry Consensus

**ALL modern GPU terminals now render box-drawing procedurally.** Font-based rendering is considered legacy due to:

1. **Alignment issues**: Font metrics cause gaps/overlaps between cells
2. **Inconsistency**: Different fallback fonts render boxes differently
3. **Crispness**: Anti-aliasing blurs cell boundaries
4. **Control**: Procedural rendering allows exact pixel placement

### Unicode Range Coverage Priorities

| Range | Name | Built-in Priority | Phase |
|-------|------|-------------------|-------|
| U+2500–U+257F | Box Drawing | Must have | Phase 1 |
| U+2580–U+259F | Block Elements | Must have | Phase 1 |
| U+E0B0–U+E0B3 | Powerline | Should have | Phase 2 |
| U+1FB00–U+1FBFF | Legacy Computing | Nice to have | Phase 4 |
| U+2800–U+28FF | Braille Patterns | Nice to have | Phase 4 |

**Recommendation for Crux**:
- Phase 1: U+2500–U+257F (box drawing) + U+2580–U+259F (block elements)
- Phase 2: U+E0B0–U+E0B3 (Powerline arrows) — see §12
- Phase 4: U+1FB00–U+1FBFF (legacy computing), U+2800–U+28FF (Braille)

### Implementation Pattern

```rust
fn is_builtin_glyph(c: char) -> bool {
    matches!(c,
        '\u{2500}'..='\u{257F}' |  // Box Drawing
        '\u{2580}'..='\u{259F}' |  // Block Elements
        '\u{E0B0}'..='\u{E0B3}'    // Powerline (Phase 2)
    )
}

fn render_builtin_glyph(
    c: char,
    cell: Bounds<Pixels>,
    line_width: Pixels,
    color: Color,
) -> Vec<Primitive> {
    match c {
        '\u{2500}'..='\u{257F}' => render_box_drawing(c, cell, line_width, color),
        '\u{2580}'..='\u{259F}' => render_block_element(c, cell, color),
        '\u{E0B0}'..='\u{E0B3}' => render_powerline(c, cell, color),
        _ => vec![],
    }
}
```

---

## 11. Nerd Fonts and Private Use Area (PUA)

### Nerd Fonts Overview

Nerd Fonts patch popular programming fonts with thousands of glyphs (icons, ligatures, symbols) using the Unicode Private Use Area (U+E000–U+F8FF, U+F0000–U+10FFFF).

**Three variants** (as of Nerd Fonts v3):
1. **Nerd Font Mono (NFM)**: Single-width glyphs (1 cell) — terminal-safe
2. **Nerd Font (NF)**: Double-width glyphs (~1.5–2 cells) — GUI editors
3. **Nerd Font Propo**: Proportional width — documents/web

### The wcwidth() Problem

The `wcwidth()` function (and Rust's `unicode-width` crate) returns width for Unicode characters:
- **Standard characters**: Returns 1 (narrow) or 2 (wide)
- **PUA codepoints**: Returns **-1** (undefined) or **1** (fallback)

However, **Nerd Font (NF) variant renders PUA as ~1.5–2 cells wide**, causing misalignment.

#### Terminal Handling Strategies

| Terminal | PUA Width Default | Configurable | Notes |
|----------|-------------------|--------------|-------|
| Kitty | 1 cell, with exceptions | Yes | Issue #1029: Treats PUA+space as 2 cells |
| URxvt | Requires NFM variant | No | Crashes on double-width PUA |
| WezTerm | 1 cell | Yes | `unicode_width` config override |
| iTerm2 | 1 cell | No | Strict wcwidth() adherence |
| Alacritty | 1 cell | No | Strict wcwidth() adherence |

**Kitty's exception** (issue #1029): If a PUA codepoint is followed by a space, treat it as 2 cells. This heuristic handles some Nerd Font icons correctly but is not foolproof.

### Nerd Fonts v3 Changes

Nerd Fonts v3 (2023) clarified the three variants:
- **NFM**: Explicitly single-width, safe for terminals
- **NF**: Explicitly double-width, for GUI
- **Propo**: Explicitly proportional, for documents

Before v3, all variants were ambiguous, causing widespread terminal issues.

### wcwidth-icons Workaround

The `wcwidth-icons` project provides an `LD_PRELOAD` library that overrides `wcwidth()` to return 2 for specific PUA ranges.

**Not applicable to Crux**: Rust native code doesn't use libc `wcwidth()`. This is a workaround for C-based terminals and shell utilities.

### Recommendation for Crux

**Default behavior**: Treat PUA codepoints as **single-width (1 cell)**.

**Rationale**:
- Matches `wcwidth()` behavior
- Compatible with Nerd Font Mono (NFM) variant
- Avoids misalignment in `ls` output, prompts, etc.

**Future enhancement** (Phase 3+): Add configurable PUA width override

```toml
[font.pua_width]
# Override specific PUA ranges to 2 cells
ranges = [
    { start = 0xE0A0, end = 0xE0A3, width = 2 },  # Powerline branch/LN/lock/flag
]
```

**User guidance**: Recommend **Nerd Font Mono** variant in documentation.

---

## 12. Powerline Symbol Rendering

### Powerline Symbols

Powerline is a popular shell prompt/statusline framework that uses triangular arrow symbols for segment separation:

| Codepoint | Glyph | Name |
|-----------|-------|------|
| U+E0B0 | `` | Right-pointing triangle (solid) |
| U+E0B1 | `` | Right-pointing triangle (outline) |
| U+E0B2 | `` | Left-pointing triangle (solid) |
| U+E0B3 | `` | Left-pointing triangle (outline) |

**Visual requirement**: Arrows must **fill the entire cell boundary** with no gaps or overlaps.

### Root Causes of Gaps

When rendered via fonts, Powerline symbols often show visible gaps between segments:

1. **ClearType vs grayscale anti-aliasing**: ClearType (subpixel AA) causes color fringing at cell boundaries
2. **Font bounding box clipping**: Font's glyph bounding box doesn't extend to cell edges
3. **Height alignment**: Vertical centering causes gaps at top/bottom
4. **Sub-pixel positioning**: Fractional coordinates cause edge blur

### Production Solutions

#### Microsoft Terminal (issue #7260)

**Problem**: ClearType anti-aliasing caused color fringing on Powerline arrows

**Solution**: Switch from ClearType to **grayscale anti-aliasing** for custom glyphs

**Result**: Clean edges, no color fringing

#### VS Code (issue #128917)

**Problem**: Font-rendered Powerline had persistent gaps

**Solution**: Custom glyph rendering system that bypasses the font entirely

**Implementation**: Canvas API renders triangles procedurally

#### iTerm2

**Built-in Powerline renderer**: Preferences → Text → "Use built-in Powerline glyphs"

**Behavior**: When enabled, U+E0B0–U+E0B3 are rendered as GPU triangles instead of font glyphs

**Result**: Pixel-perfect alignment, no gaps

### Recommendation for Crux

**Phase 2**: Render U+E0B0–U+E0B3 procedurally (like box drawing)

**Implementation approach**:
1. Detect Powerline codepoints in `is_builtin_glyph()`
2. Render as filled triangles using Metal/GPUI primitives
3. Ensure triangle vertices exactly align with cell boundaries
4. Use **grayscale anti-aliasing** (not ClearType/subpixel)

```rust
fn render_powerline(c: char, cell: Bounds<Pixels>, color: Color) -> Vec<Primitive> {
    let (x, y, w, h) = (cell.origin.x, cell.origin.y, cell.size.width, cell.size.height);

    match c {
        '\u{E0B0}' => {
            // Right-pointing solid triangle
            // Vertices: top-left, bottom-left, middle-right
            vec![Triangle {
                points: [(x, y), (x, y + h), (x + w, y + h/2.0)],
                color,
            }]
        },
        '\u{E0B1}' => {
            // Right-pointing outline triangle
            vec![TriangleOutline {
                points: [(x, y), (x, y + h), (x + w, y + h/2.0)],
                stroke_width: Pixels(1.0),
                color,
            }]
        },
        '\u{E0B2}' => {
            // Left-pointing solid triangle
            vec![Triangle {
                points: [(x + w, y), (x + w, y + h), (x, y + h/2.0)],
                color,
            }]
        },
        '\u{E0B3}' => {
            // Left-pointing outline triangle
            vec![TriangleOutline {
                points: [(x + w, y), (x + w, y + h), (x, y + h/2.0)],
                stroke_width: Pixels(1.0),
                color,
            }]
        },
        _ => vec![],
    }
}
```

**Anti-aliasing**: Use grayscale (not subpixel). On macOS, this is the default post-10.15.

---

## 13. Emoji Rendering Details

### Variation Selectors

Unicode provides two variation selectors for controlling emoji rendering:

| Selector | Codepoint | Effect | Width |
|----------|-----------|--------|-------|
| VS-15 | U+FE0E | Text presentation (monochrome) | 1 cell |
| VS-16 | U+FE0F | Emoji presentation (color) | 2 cells |

**Example**:
- `☺` (U+263A) alone → implementation-dependent
- `☺︎` (U+263A U+FE0E) → text, 1 cell, monochrome
- `☺️` (U+263A U+FE0F) → emoji, 2 cells, color

**Terminal behavior**: Must track variation selectors to determine cell width.

### ZWJ (Zero-Width Joiner) Sequences

U+200D (ZWJ) combines multiple emoji into a single **grapheme cluster**.

**Example**: 👨‍👩‍👧‍👦 (family)
- Codepoints: U+1F468 U+200D U+1F469 U+200D U+1F467 U+200D U+1F466 (7 codepoints)
- Rendered as: Single glyph
- Width: **2 cells** (not 14!)

**Terminal requirement**:
1. Detect ZWJ sequences using Unicode segmentation
2. Treat entire cluster as single grapheme
3. Assign width based on first emoji (usually 2 cells)

```rust
use unicode_segmentation::UnicodeSegmentation;

fn grapheme_width(s: &str) -> usize {
    let graphemes = s.graphemes(true);
    graphemes.map(|g| {
        // For ZWJ sequences, width of first character
        let first_char = g.chars().next().unwrap();
        first_char.width().unwrap_or(1)
    }).sum()
}
```

### Apple Color Emoji Format

macOS ships with **Apple Color Emoji** font in **CBDT/CBLC** format (embedded PNG bitmaps).

**Critical requirement**: Must use **Core Text**, not Core Graphics

- **Core Text**: Handles CBDT/CBLC correctly, renders color emoji
- **Core Graphics**: Only renders outlines, emoji appear as missing glyphs

**GPUI implication**: GPUI uses Core Text on macOS, so color emoji work automatically via the text system.

### Terminal Emoji Support Inconsistencies

| Terminal | Unicode Version | ZWJ Support | Variation Selector | Notes |
|----------|----------------|-------------|-------------------|-------|
| Konsole | 15.0 | Full | Yes | Best support |
| iTerm2 | 15.0 | Full | Yes | Uses Core Text |
| Kitty | 15.0 | Full | Yes | Custom emoji renderer |
| VS Code | 12.1.0 | Limited | Partial | Outdated Unicode data |
| Hyper | 12.1.0 | Limited | Partial | Electron limitation |

**Kitty's approach**: Custom emoji data file (`unicode_names.txt`) updated with each Unicode release.

### Testing Tool

**ucs-detect** (https://ucs-detect.readthedocs.io/): Command-line tool for validating Unicode support

```bash
pip install ucs-detect
ucs-detect --unicode-version 15.0
```

Generates visual test cases for:
- Emoji sequences
- ZWJ families
- Variation selectors
- Skin tone modifiers (U+1F3FB–U+1F3FF)
- Country flags (regional indicators)

### Recommendation for Crux

**Phase 1**: Basic emoji via Core Text fallback
1. Add "Apple Color Emoji" to fallback chain
2. Use `unicode-segmentation` crate for grapheme clustering
3. Track variation selectors for width calculation

**Phase 3**: Enhanced emoji support
1. Update to Unicode 15.0+ (or latest stable)
2. Test ZWJ sequences with `ucs-detect`
3. Handle skin tone modifiers correctly
4. Support regional indicator pairs (flag emoji)

```rust
use unicode_segmentation::UnicodeSegmentation;

fn emoji_width(cluster: &str) -> usize {
    // Check for variation selector
    if cluster.contains('\u{FE0E}') {
        return 1;  // Text presentation
    }
    if cluster.contains('\u{FE0F}') {
        return 2;  // Emoji presentation
    }

    // Default: first character determines width
    cluster.chars().next()
        .and_then(|c| c.width())
        .unwrap_or(1)
}
```

---

## 14. GPUI Text Rendering Architecture

### Zed's GPU Text Rendering Approach

From the Zed blog post "Leveraging Rust and the GPU to render user interfaces at 120 FPS":

**Pipeline**:
1. **OS handles font rasterization** → High-quality platform-native glyphs
2. **Texture atlas caching on GPU** → Rasterize once, reuse many times
3. **Parallel assembly** → Multiple text runs rendered in parallel

**Text shaping**: Uses **Core Text** on macOS for native consistency
- Handles ligatures, combining marks, BiDi, complex scripts
- Platform-native rendering matches system appearance

**Cache key**: `(font_id, glyph_id, font_size)`
- Invariant: Same glyph at same size always produces same texture
- LRU eviction when atlas fills

### cosmic-text: Pure Rust Alternative

**pop-os/cosmic-text**: Full-featured text engine in pure Rust

**Features**:
- **rustybuzz**: HarfBuzz port for text shaping
- **Font fallback**: Automatic fallback chain
- **Color emoji**: CBDT/CBLC support
- **BiDi**: Unicode bidirectional algorithm
- **Ligatures**: OpenType GSUB tables

**Core API**:
```rust
use cosmic_text::{FontSystem, SwashCache, Buffer, Attrs};

let mut font_system = FontSystem::new();
let mut cache = SwashCache::new();
let mut buffer = Buffer::new(&mut font_system, Metrics::new(14.0, 16.0));

buffer.set_text("Hello, world!", Attrs::new());
buffer.shape_until_scroll();

for run in buffer.layout_runs() {
    for glyph in run.glyphs {
        let image = cache.get_image(&font_system, glyph.cache_key);
        // Render glyph image to screen
    }
}
```

**GPUI compatibility issue**: Zed issue #30526 reports version incompatibility
- GPUI uses older cosmic-text version
- API breaking changes between cosmic-text releases
- Direct integration requires version alignment

### Metal Texture Atlas

**Strategy**: Rasterize glyphs once → store in GPU texture → fast GPU copy to output

**Structure**:
- 2D texture array (multiple "pages" for large glyph sets)
- Bin-packing algorithm for atlas placement (e.g., rectpack2D, guillotiere)
- LRU eviction when atlas is full

**Cache invalidation triggers**:
- Font size change
- Font weight change (bold/italic)
- New font loaded
- Window DPI change

**Performance**: Texture atlas avoids re-rasterizing glyphs every frame
- Rasterization: ~1-5ms per glyph (expensive)
- Texture lookup: ~0.001ms per glyph (fast)

### Ghostty's Approach

From Ghostty source code and Mitchell Hashimoto's devlogs:

**Platform**: Metal on macOS
- Custom Metal shaders for cell rendering
- Sprite renderer for box drawing (see §10)
- Standard texture atlas for text

**Font system**: Core Text for macOS consistency

**Performance optimization**: Damage tracking from alacritty_terminal
- Only re-render changed cells
- Atlas lookup for unchanged cells

### Anti-aliasing on macOS

**Subpixel AA removed in macOS 10.15**: Apple deprecated ClearType-style subpixel anti-aliasing

**Modern standard**: **Grayscale anti-aliasing**
- Better for Retina displays (high DPI)
- No color fringing
- Simpler rendering pipeline

**Kitty's approach** (issue #214):
- sRGB linear gamma blending for correct anti-aliasing
- Avoids "dark halo" around light-on-dark text

```rust
// Pseudo-code for correct blending
fn blend_glyph(glyph_alpha: f32, fg: Color, bg: Color) -> Color {
    // Convert sRGB to linear
    let fg_linear = srgb_to_linear(fg);
    let bg_linear = srgb_to_linear(bg);

    // Alpha blend in linear space
    let blended = fg_linear * glyph_alpha + bg_linear * (1.0 - glyph_alpha);

    // Convert back to sRGB
    linear_to_srgb(blended)
}
```

### Recommended Path for Crux

**Phase 1**: Start with GPUI text system
- GPUI provides Core Text backend on macOS
- Automatic texture atlas management
- Platform-native text rendering

**Phase 1+**: Add built-in rendering for special glyphs
- Box drawing (U+2500–U+257F)
- Block elements (U+2580–U+259F)
- Bypass text system for these ranges

**Phase 2**: Add Powerline symbols (U+E0B0–U+E0B3)

**Phase 3**: CJK/emoji fallback optimization
- Explicit fallback chain configuration
- Korean-first priority (see §3)

**Phase 4+**: Defer ligatures to later phase
- High performance cost (see §15)
- Ligatures disabled by default

---

## 15. Font Shaping Performance

### WezTerm HarfBuzz Performance Issue

**WezTerm issue #5280**: Performance regression with ligature fonts

**Profiling results**:
- With ligature font (Fira Code): **49.8–85.5% CPU** in `HarfbuzzShaper::do_shape`
- Without ligatures (JetBrains Mono, ligatures disabled): **24.1% CPU**
- **2–3.5x performance penalty** for ligature support

**Root cause**: HarfBuzz text shaping is computationally expensive
- Analyzes OpenType GSUB tables
- Performs glyph substitution for ligatures
- Must re-run on every text change

### Alacritty's Position on Ligatures

**Alacritty issue #50** (locked, definitive):

> "Ligatures are not worth dropping a single frame for."

**Rationale**:
- Alacritty targets 60+ FPS on all hardware
- Ligature shaping adds 10–20ms per frame on complex text
- Terminal responsiveness > aesthetic features
- **Explicitly refused** to implement ligatures

### Ghostty's Trade-off

Ghostty **accepts the HarfBuzz cost** for feature completeness:
- Provides `font-feature` config for OpenType features
- Defaults to ligatures **disabled**
- Users opt-in by enabling `calt`, `liga`, `dlig` features

**Ghostty's `font-shaper-run-breaking`**: Breaks shaping at cursor position
- Prevents ligatures from spanning cursor
- Improves editing clarity (e.g., `fi` ligature split by cursor)

### Ligature Limitation: Color Split

Syntax highlighting breaks ligatures when operators have different colors.

**Example**: `>=` in code
- `>` may be highlighted as operator (orange)
- `=` may be highlighted as operator (orange) or part of `=>` (blue)
- Color change **forces separate rendering runs**
- Ligature cannot form across runs

**Result**: Ligatures work in plain text (logs, prose) but break in syntax-highlighted code (editors, `bat`, `delta`).

### Caching Strategy

**Cache key**: `(text, font, features, size)` → shaped glyphs

**Recomputation triggers**:
- Text content changes
- Font size changes
- Cursor position changes (if shaper-run-breaking enabled)
- Color changes (forces separate runs)

**Optimization**: Shape only visible lines
- Don't shape off-screen buffer content
- Re-shape on scroll (amortize over frames)

```rust
struct ShapedLineCache {
    cache: HashMap<(String, FontId, Pixels), ShapedLine>,
    max_entries: usize,
}

impl ShapedLineCache {
    fn get_or_shape(&mut self, text: &str, font: FontId, size: Pixels) -> &ShapedLine {
        let key = (text.to_string(), font, size);
        self.cache.entry(key).or_insert_with(|| {
            // Expensive: call HarfBuzz
            shape_text(text, font, size)
        })
    }
}
```

### Break Shaping Runs at Cursor

**Ghostty's approach**: `font-shaper-run-breaking` option

**Behavior**: Split text into separate shaping runs at:
- Cursor position
- Selection boundaries
- Color change boundaries

**Benefits**:
- Clearer editing (ligature doesn't obscure cursor)
- Faster shaping (shorter runs)

**Cost**: Ligatures break at cursor

```rust
fn shape_line_with_cursor(
    text: &str,
    cursor_col: usize,
    font: FontId,
    size: Pixels,
) -> Vec<ShapedRun> {
    let (before, after) = text.split_at(cursor_col);
    vec![
        shape_text(before, font, size),
        shape_text(after, font, size),
    ]
}
```

### Recommendation for Crux

**Phase 1**: No ligature support
- Keep rendering pipeline simple
- Avoid HarfBuzz performance cost
- Character-by-character rendering

**Phase 4+**: Optional ligature support (disabled by default)
- Add HarfBuzz shaping via GPUI (which already has it)
- Config: `font.ligatures = false` (default)
- When enabled: Use GPUI's layout_line (which calls Core Text)
- Break runs at cursor position for editing clarity

**Performance target**: 60 FPS on 4K displays
- Ligature shaping must not drop frames
- Profile on older hardware (e.g., 2019 MacBook Pro)

---

## Sources

- [Core Text Programming Guide](https://developer.apple.com/library/archive/documentation/StringsTextFonts/Conceptual/CoreText_Programming/Overview/Overview.html) — Apple official docs
- [CTFontCopyDefaultCascadeListForLanguages](https://developer.apple.com/documentation/coretext/1509991-ctfontcopydefaultcascadelistforl) — Locale-aware fallback API
- [UAX #11: East Asian Width](https://www.unicode.org/reports/tr11/) — Unicode standard for character width
- [font-kit crate](https://docs.rs/font-kit/latest/font_kit/) — Cross-platform font discovery
- [Ghostty Font Rendering](https://mitchellh.com/writing/ghostty-devlog-003) — Mitchell Hashimoto's devlog on `ic` metric discovery
- [Ghostty Box Drawing Sprite Renderer](https://github.com/ghostty-org/ghostty/blob/main/src/font/sprite/Box.zig) — Comprehensive built-in box drawing implementation
- [Alacritty Box Drawing](https://github.com/alacritty/alacritty/blob/master/alacritty/src/renderer/rects.rs) — Procedural box drawing implementation (commit f717710)
- [Kitty Box Drawing Discussions](https://github.com/kovidgoyal/kitty/issues/7680) — Rounded corner implementation (commit 533688a)
- [Kitty Subpixel Anti-aliasing](https://github.com/kovidgoyal/kitty/issues/214) — sRGB linear gamma blending
- [Adobe Box Drawing Reference](https://github.com/adobe-type-tools/box-drawing) — Canonical reference implementation
- [unicode-width crate](https://docs.rs/unicode-width/latest/unicode_width/) — UAX #11 implementation
- [unicode-segmentation crate](https://docs.rs/unicode-segmentation/latest/unicode_segmentation/) — Grapheme cluster segmentation
- [OpenType Variable Fonts](https://learn.microsoft.com/en-us/typography/opentype/spec/otvaroverview) — Variable font specification
- [Nerd Fonts Wiki](https://github.com/ryanoasis/nerd-fonts/wiki) — PUA glyph documentation
- [Nerd Fonts v3 Release](https://github.com/ryanoasis/nerd-fonts/discussions/1074) — NFM/NF/Propo variant clarification
- [Kitty PUA Width Handling](https://github.com/kovidgoyal/kitty/issues/1029) — PUA followed by space heuristic
- [Microsoft Terminal Powerline ClearType Issue](https://github.com/microsoft/terminal/issues/7260) — Grayscale vs subpixel anti-aliasing
- [Microsoft Terminal Powerline Gaps](https://github.com/microsoft/terminal/issues/13029) — Additional Powerline rendering challenges
- [VS Code Powerline Rendering](https://github.com/microsoft/vscode/issues/128917) — Custom glyph rendering system
- [ucs-detect Documentation](https://ucs-detect.readthedocs.io/) — Unicode support validation tool
- [Zed GPU Rendering Blog Post](https://zed.dev/blog/leveraging-rust-and-the-gpu) — "Leveraging Rust and the GPU to render user interfaces at 120 FPS"
- [cosmic-text GitHub](https://github.com/pop-os/cosmic-text) — Pure Rust text shaping engine
- [cosmic-text GPUI Compatibility](https://github.com/zed-industries/zed/issues/30526) — Version incompatibility issue
- [Warp Text Rendering Blog](https://www.warp.dev/blog/how-warp-works) — GPU terminal text rendering architecture
- [Jeff Quast Terminal Battle Royale](https://github.com/jquast/terminal-battle-royale) — Comprehensive Unicode terminal testing
- [macOS Font Rendering](https://skip.house/blog/mac-font-rendering/) — Modern anti-aliasing on macOS
- [WezTerm HarfBuzz Performance](https://github.com/wez/wezterm/issues/5280) — Ligature shaping performance analysis
- [Alacritty Ligatures Issue](https://github.com/alacritty/alacritty/issues/50) — "Not worth dropping a single frame for" (locked)
- [Ghostty Font Features Documentation](https://ghostty.org/docs/config/reference#font-feature) — OpenType feature configuration
