---
title: Alacritty Known Issues and Lessons Learned
description: Comprehensive research on open and resolved bugs, architectural issues, and gotchas in Alacritty that Crux should learn from and avoid
phase: all
topics: [terminal-emulation, rendering, ime, performance, architecture]
related: [terminal-architecture.md, terminal-emulation.md, ime-clipboard.md, gap-analysis.md]
---

# Alacritty Known Issues and Lessons Learned

Research on documented bugs and architectural issues in Alacritty terminal emulator to inform Crux development and avoid common pitfalls.

**Research Date**: 2026-02-12
**Target**: macOS terminal with Rust + Metal/GPUI
**Sources**: GitHub issues, bug reports, community discussions

---

## Table of Contents

1. [Top Issues by Community Engagement](#top-issues-by-community-engagement)
2. [CJK/IME Bugs](#cjkime-bugs)
3. [macOS-Specific Issues](#macos-specific-issues)
4. [Font Rendering Issues](#font-rendering-issues)
5. [Memory Leaks and Performance](#memory-leaks-and-performance)
6. [VT Emulation Edge Cases](#vt-emulation-edge-cases)
7. [Clipboard and Mouse Handling](#clipboard-and-mouse-handling)
8. [tmux Compatibility Issues](#tmux-compatibility-issues)
9. [Rendering Architecture Lessons](#rendering-architecture-lessons)
10. [Configuration System Issues](#configuration-system-issues)
11. [Scrollback Buffer Issues](#scrollback-buffer-issues)
12. [Key Takeaways for Crux](#key-takeaways-for-crux)

---

## Top Issues by Community Engagement

The most-requested features and pain points from the Alacritty community (sorted by reactions):

### 1. **Ligature Support** ([#50](https://github.com/alacritty/alacritty/issues/50))
- **Status**: Open since 2017
- **Root cause**: Deliberate design decision - ligatures add complexity and potentially impact performance
- **Lesson**: This is a highly contentious feature. Alacritty prioritizes minimalism over ligatures, while competitors (WezTerm, Kitty) implement them. Consider early architectural support if desired.

### 2. **Sixel Graphics** ([#910](https://github.com/alacritty/alacritty/issues/910))
- **Status**: Open, enhancement labeled
- **Lesson**: Graphics protocols are increasingly expected. Plan for extensibility early (Sixel, Kitty graphics, iTerm2 inline images).

### 3. **Multiple Windows** ([#607](https://github.com/alacritty/alacritty/issues/607))
- **Status**: Closed - completed in v0.10.0
- **Lesson**: Multi-window support is complex but essential. Crux has this in Phase 2 roadmap.

### 4. **Scrollback** ([#124](https://github.com/alacritty/alacritty/issues/124))
- **Status**: Closed - completed Sep 2018
- **Lesson**: Scrollback took significant time to implement correctly. See dedicated section below.

### 5. **Tabs Support** ([#3129](https://github.com/alacritty/alacritty/issues/3129))
- **Status**: Closed - marked "won't fix"
- **Reason**: Alacritty delegates to window managers/tmux for tab management
- **Lesson**: Crux is taking the opposite approach (native tabs in Phase 2) to better integrate with Claude Code Agent Teams.

---

## CJK/IME Bugs

**CRITICAL CATEGORY** for Crux's Korean/CJK focus.

### macOS IME Issues

#### 1. **Keyboard Input Doesn't Work with CJK IME** ([#6942](https://github.com/alacritty/alacritty/issues/6942))
- **Description**: First number/special character typed is ignored when typing non-ASCII characters like Hangul (Korean)
- **Version**: 0.12.1 on macOS
- **Root cause**: Likely improper handling of marked text / composition events
- **Lesson**: **Crux must never send preedit/composition text to PTY**. Only committed text goes to PTY. See `research/platform/ime-clipboard.md`.

#### 2. **Double-Space Bug** ([#8079](https://github.com/alacritty/alacritty/issues/8079))
- **Description**: Space inserted twice per stroke when using CJK input method on macOS
- **Version**: 0.13.2
- **Root cause**: Likely duplicate handling of space in both composition and insertion events
- **Lesson**: Carefully track IME state machine to avoid duplicate key events.

#### 3. **Japanese Kana/Eiso Keys Not Registered** ([#7167](https://github.com/alacritty/alacritty/issues/7167))
- **Description**: Kana key (to switch to Japanese input) only prints space, same with Eiso key
- **Version**: macOS 10.12.6
- **Lesson**: IME mode-switching keys require special handling separate from normal key events.

### Linux IME Issues

#### 4. **Window Freeze with Korean IME (uim)** ([#4469](https://github.com/alacritty/alacritty/issues/4469))
- **Description**: Pressing a key in "Hangul input mode" freezes Alacritty completely, requiring force kill
- **Root cause**: Likely deadlock or infinite loop in IME event handling
- **Lesson**: IME event handlers must be non-blocking and have timeout protection.

### General CJK Problems

#### 5. **Cannot Input Japanese Characters** ([#1101](https://github.com/alacritty/alacritty/issues/1101))
- **Description**: Japanese character input completely non-functional
- **Lesson**: Many terminal emulators struggle with proper IME integration. This is a differentiator opportunity for Crux.

**Crux Strategy**: Phase 3 explicitly focuses on "Korean/CJK IME, rich clipboard, drag-and-drop". These bugs validate the importance of this phase and show common failure modes to avoid.

---

## macOS-Specific Issues

### Display and Rendering

#### 1. **Font Size Inconsistent Across Retina and Non-Retina** ([#3732](https://github.com/alacritty/alacritty/issues/3732), [#1069](https://github.com/alacritty/alacritty/issues/1069))
- **Description**: Font size specified in "points" (physical measurement) doesn't consistently render across Retina and non-Retina displays
- **Root cause**: Incorrect DPI scaling calculations when moving between displays
- **Fixed**: [#71](https://github.com/alacritty/alacritty/issues/71) - "Update DPI/DPR when switching monitors" (Nov 2018)
- **Lesson**: **Monitor for display change events** and recalculate font metrics. GPUI likely handles this, but validate.

#### 2. **Font Rendering Quality on Retina** ([#7333](https://github.com/alacritty/alacritty/issues/7333))
- **Description**: Font rendering too bold and blurry on Retina displays
- **Version**: macOS Sonoma, 2880x1800 display
- **Root cause**: Subpixel rendering not handled correctly ([#3756](https://github.com/alacritty/alacritty/issues/3756))
- **Lesson**: Metal rendering requires careful attention to sRGB color space and subpixel antialiasing.

#### 3. **Text Pixelated on Non-Retina Displays** ([#1368](https://github.com/alacritty/alacritty/issues/1368))
- **Description**: Text on colored backgrounds looks pixelated on non-Retina 4K displays
- **Root cause**: Poor text quality at sub-15px sizes on lower DPI screens
- **Lesson**: Font rendering must be tuned separately for Retina (200+ DPI) vs standard (96 DPI) displays.

### Fullscreen Mode

#### 4. **Menu Bar Inaccessible in Fullscreen** ([#4105](https://github.com/alacritty/alacritty/issues/4105))
- **Description**: Cannot access macOS menu bar when in fullscreen mode
- **Lesson**: Fullscreen implementation should allow menu bar hover-to-reveal (standard macOS behavior).

#### 5. **Fullscreen Startup Mode Alternates** ([#3797](https://github.com/alacritty/alacritty/issues/3797))
- **Description**: Setting `startup_mode: Fullscreen` causes alternating behavior on subsequent launches from Dock/Spotlight
- **Root cause**: Likely improper state persistence or fullscreen flag toggling
- **Lesson**: Carefully manage window state restoration to avoid toggle-like bugs.

#### 6. **App Switching Focus Issues** ([#3659](https://github.com/alacritty/alacritty/issues/3659))
- **Description**: Cmd+Tab sometimes focuses desktop instead of fullscreen Alacritty window
- **Root cause**: macOS Spaces integration issue
- **Lesson**: Fullscreen windows on separate Spaces require explicit focus management.

#### 7. **Creating New Window Turns Old Window Black** ([#7980](https://github.com/alacritty/alacritty/issues/7980))
- **Description**: With `startup_mode = "Fullscreen"`, creating new tab/window with Cmd-N/Cmd-T makes old window go black
- **Root cause**: Likely OpenGL context loss when creating new window
- **Lesson**: **Metal contexts must be properly managed** when creating multiple windows. GPUI should handle this.

#### 8. **Buttonless Decorations Break Fullscreen Toggle** ([#2215](https://github.com/alacritty/alacritty/issues/2215))
- **Description**: With `decorations: buttonless`, can't toggle fullscreen
- **Lesson**: Provide keyboard shortcut and menu item for fullscreen regardless of window decoration style.

### Window Flickering

#### 9. **Window Flickering During Resize** ([#7898](https://github.com/alacritty/alacritty/issues/7898), [#8549](https://github.com/alacritty/alacritty/issues/8549))
- **Description**: Client area intermittently becomes transparent during resize, causing flickering (2024, macOS 14.4.1)
- **Root cause**: Redraw timing issues - drawing called at wrong point in event loop
- **Potential fixes**: Enable vsync, add sleep delays after resize, adjust event handling order
- **Lesson**: **Synchronize redraws with window resize events**. Use vsync. Consider double-buffering strategy.

#### 10. **Vertical Resize Flickering** ([#8549](https://github.com/alacritty/alacritty/issues/8549))
- **Description**: Flickering artifacts when window is vertically resized smaller (horizontal works fine)
- **Lesson**: Asymmetric resize behavior suggests grid recalculation bug. Test both axes independently.

### Performance

#### 11. **macOS Shadow Invalidation Performance Regression** ([#4604](https://github.com/alacritty/alacritty/issues/4604))
- **Description**: Performance regression related to window shadow rendering on macOS
- **Lesson**: macOS window effects (shadows, transparency) can interact poorly with GPU rendering. Provide disable option.

---

## Font Rendering Issues

### Spacing and Kerning

#### 1. **Incorrect Font Spacing** ([#1881](https://github.com/alacritty/alacritty/issues/1881), [#990](https://github.com/alacritty/alacritty/issues/990))
- **Description**: Messed up spacing between characters, particularly on HiDPI devices
- **Root cause**: Incorrect font metric calculations
- **Lesson**: Terminal fonts are monospace - every cell must be exact same width. Use font metrics carefully.

#### 2. **Kerning Terrible Since 0.12.1** ([#7043](https://github.com/alacritty/alacritty/issues/7043))
- **Description**: Kerning with any font became unreadable after update
- **Developer response**: Haven't changed font handling, likely fonts not being found correctly
- **Lesson**: Font fallback logic must be robust. Log which font file is actually loaded.

#### 3. **Font Slightly Too Wide** ([#3293](https://github.com/alacritty/alacritty/issues/3293))
- **Description**: Fonts like Roboto Mono appear wider than in other terminals
- **Lesson**: Different terminals interpret font metrics differently. Provide cell width adjustment option.

#### 4. **Fractional Font Sizes** ([#2780](https://github.com/alacritty/alacritty/issues/2780))
- **Description**: Font rendering issues with fractional sizes (e.g., 9.5pt)
- **Lesson**: Support fractional point sizes but validate rendering at scale factors of 1x, 1.5x, 2x.

### Character Width Issues

#### 5. **Spastic Font Spacing** ([#561](https://github.com/alacritty/alacritty/issues/561))
- **Description**: Character width incorrect, causing overlapping or gaps
- **Root cause**: Incorrect cell width calculation from font metrics
- **Lesson**: Cell dimensions must be calculated once at font load and remain consistent.

---

## Memory Leaks and Performance

### Memory Leaks

#### 1. **Memory Leaks When Idle** ([#6210](https://github.com/alacritty/alacritty/issues/6210))
- **Description**: Memory consumption increases quickly even when idle, particularly on high-refresh displays (120fps, 2560x1440)
- **Root cause**: Likely resource accumulation in render loop or event handler
- **Lesson**: **Profile memory over extended periods**. High refresh rate amplifies per-frame leaks.

#### 2. **Memory Leaks from 80MB to 800MB** ([#4806](https://github.com/alacritty/alacritty/issues/4806))
- **Description**: Severe memory leak reaching OOM levels
- **Lesson**: Set up automated memory leak detection in CI.

#### 3. **Text Selection Memory Leak** ([#1640](https://github.com/alacritty/alacritty/issues/1640))
- **Description**: Selecting large text in `less` and scrolling beyond screen edges causes freeze and memory consumption up to 25.5GB
- **Root cause**: **Incorrect viewport scrolling with underflow** - selection wasn't handling going below 0
- **Fixed**: Yes
- **Lesson**: **Selection bounds must be clamped to valid ranges**. Integer underflow in selection logic is catastrophic.

#### 4. **Font Zoom Memory Leak** ([#4815](https://github.com/alacritty/alacritty/issues/4815))
- **Description**: Repeatedly adjusting font size (Cmd-+/-) increases memory from 54MB to 165MB
- **Root cause**: Texture atlas not freed when regenerating at new size
- **Lesson**: **Free old GPU resources before allocating new ones**. Font size changes should be constant memory.

#### 5. **OpenGL Buffer Leak** ([#4806](https://github.com/alacritty/alacritty/issues/4806) comments)
- **Description**: Missing `DeleteBuffers` calls for OpenGL buffers when QuadRenderer dropped and recreated
- **Root cause**: GPU resources not properly freed in drop implementation
- **Lesson**: **Implement Drop trait for GPU resource wrappers**. Metal has similar requirements.

### Performance Issues

#### 6. **Slower Redraw with Bigger Window** ([#3851](https://github.com/alacritty/alacritty/issues/3851))
- **Description**: Redraw time increases significantly with larger windows, noticeable slowdown on 4K screens
- **Root cause**: Alacritty redraws entire screen every frame (by design)
- **Lesson**: Damage tracking is essential for large displays. Alacritty eventually added this in v0.11.0.

#### 7. **Extremely Slow with Some Unicode** ([#2858](https://github.com/alacritty/alacritty/issues/2858))
- **Description**: Severe performance degradation with certain Unicode characters
- **Root cause**: Expensive glyph rasterization or font fallback
- **Lesson**: **Cache glyph textures aggressively**. Limit font fallback chain depth.

### Input Latency

#### 8. **Input Latency Issues** ([#673](https://github.com/alacritty/alacritty/issues/673))
- **Description**: Worst-case latency of 3 VBLANK intervals
- **Root cause**: Rendering on main thread - input arrives just after VBI, adding latency of VBI - draw_time
- **Potential fix**: Move rendering to separate thread (but windowing APIs often require input on main thread)
- **Status**: Closed (improved Jul 2020)
- **Lesson**: **Minimize time between input event and PTY write**. Decouple input handling from rendering.

#### 9. **Severe Input Lag** ([#5883](https://github.com/alacritty/alacritty/issues/5883), [#4801](https://github.com/alacritty/alacritty/issues/4801))
- **Description**: Character doesn't render until next keypress or ~5 seconds later
- **Root cause**: Event loop stalling or blocking
- **Lesson**: **Never block the event loop**. PTY reads/writes must be async.

---

## VT Emulation Edge Cases

### Standard Compliance

#### 1. **Limited VT Standard Support**
- **Description**: Alacritty only supports up to VT102, missing VT420+ features like DECLRMM (set left-right margin mode)
- **Impact**: Forces tmux to do more work on less capable terminals
- **Lesson**: **Modern terminals should support VT220+ at minimum**. VT420 features are used by tmux for optimization.

#### 2. **vttest Failures** ([#240](https://github.com/alacritty/alacritty/issues/240))
- **Description**: Alacritty panics at start of vttest test 1 and partway through test 2
- **Root cause**: Edge cases in VT sequence parsing
- **Lesson**: **Run vttest in CI**. It catches obscure but important edge cases.

### Unicode Handling

#### 3. **Bi-directional Text Fails**
- **Description**: Right-to-left (RTL) text shown backwards
- **Lesson**: Bidi text requires proper Unicode bidirectional algorithm. Most terminals don't implement this.

#### 4. **Unicode Alignment Issues**
- **Description**: Alignment issues and significant latency loading files with complex Unicode
- **Lesson**: Complex Unicode (combining characters, zero-width joiners) is expensive to render.

### Character Width

#### 5. **Double-Width Emoji Treated as Single-Width** ([#6144](https://github.com/alacritty/alacritty/issues/6144))
- **Description**: Certain Unicode emoji characters treated as single-width despite displaying double-width
- **Root cause**: Incorrect Unicode width calculation
- **Standard**: Unicode Standard Annex #11 - emoji presentation sequences should be East Asian Wide (double-width)
- **Lesson**: **Use unicode-width crate correctly**. Emoji presentation sequences (U+FE0F) change width.

#### 6. **Not All Emojis Render Correctly** ([#7114](https://github.com/alacritty/alacritty/issues/7114))
- **Description**: Some emojis don't render or overlap with other text
- **Root cause**: Font substitution picks wrong font (DejaVu instead of Noto Color Emoji)
- **Lesson**: **Font fallback order matters**. Prefer color emoji fonts over monospace for emoji codepoints.

#### 7. **Emoji Spacing Issues**
- **Description**: Emojis aren't given proper space, other things drawn above them
- **Root cause**: Cell dimensions calculated from base font, emoji from different font with different metrics
- **Lesson**: **All fonts must fit within the same cell dimensions**. Scale or clip if needed.

### Crash Bugs

#### 8. **Unicode Characters Crash Alacritty** ([#473](https://github.com/alacritty/alacritty/issues/473))
- **Description**: Displaying certain Unicode characters causes crash
- **Root cause**: Likely panic in glyph rasterization or font loading
- **Lesson**: **Handle font errors gracefully**. Display replacement character (U+FFFD) instead of crashing.

---

## Clipboard and Mouse Handling

### Clipboard Issues

#### 1. **Copy/Paste Doesn't Work** ([#2383](https://github.com/alacritty/alacritty/issues/2383), [#1307](https://github.com/alacritty/alacritty/issues/1307))
- **Description**: Clipboard not modified after Ctrl-C on Windows
- **Root cause**: Clipboard integration broken on Windows
- **Lesson**: Each platform has different clipboard APIs (macOS: NSPasteboard, Windows: Win32, Linux: X11 selections).

#### 2. **Shift+Right-Click Selects and Pastes** ([#4132](https://github.com/alacritty/alacritty/issues/4132))
- **Description**: Instead of only pasting, Shift+Right-Click highlights everything between previous selection and new cursor position
- **Version**: 0.5.0 on macOS
- **Root cause**: Mouse button action conflates paste and selection behaviors
- **Lesson**: **Separate paste action from selection logic**. Modifier+click combos must be unambiguous.

#### 3. **PasteSelection on macOS Pastes from Clipboard** ([#681](https://github.com/alacritty/alacritty/issues/681))
- **Description**: Middle button action `PasteSelection` pastes from clipboard instead of selection
- **Root cause**: macOS doesn't have separate selection clipboard like X11 PRIMARY vs CLIPBOARD
- **Lesson**: **Selection clipboard is X11-specific**. On macOS, simulate with internal selection buffer.

#### 4. **Clipboard Lags Behind** ([#3601](https://github.com/alacritty/alacritty/issues/3601))
- **Description**: After 1-2 days of uptime, clipboard updates from other programs get "lagged behind" - paste emits old content
- **Platform**: X11/Xorg systems
- **Root cause**: Likely clipboard ownership or selection monitoring issue
- **Lesson**: **Monitor clipboard change events continuously**. Don't cache clipboard content long-term.

### Mouse Handling

#### 5. **Right-Click Paste Also Selects** ([#5236](https://github.com/alacritty/alacritty/issues/5236))
- **Description**: When setting right mouse to paste, it pastes but also triggers selection depending on cursor position
- **Root cause**: Mouse action handlers not mutually exclusive
- **Lesson**: **Mouse bindings must have clear precedence**. Explicit paste binding should disable selection.

#### 6. **Middle-Click Paste Behavior** (various issues)
- **Description**: Middle-click pastes same selected text after the same text instead of replacing
- **Root cause**: Selection and paste not properly coordinated
- **Lesson**: **Paste should replace selection if paste target is within selection bounds**.

#### 7. **Incorrect Mouse Position** ([#3191](https://github.com/alacritty/alacritty/issues/3191))
- **Description**: Mouse position calculations incorrect
- **Root cause**: Likely cell coordinate calculation error with window padding/decorations
- **Lesson**: **Account for window chrome when converting mouse coordinates to cell coordinates**.

---

## tmux Compatibility Issues

### Scrollback Problems

#### 1. **tmux Scrollback Doesn't Work** ([#5374](https://github.com/alacritty/alacritty/issues/5374), [#1000](https://github.com/alacritty/alacritty/issues/1000))
- **Description**: Alacritty's scrollback doesn't work with tmux's alternative screen buffer
- **Root cause**: Scrollback implementation doesn't support alternative screen
- **Lesson**: **Terminal must have separate scrollback for normal and alternate screen buffers**. tmux uses alternate screen.

#### 2. **Faux Scroll Detection** ([#1194](https://github.com/alacritty/alacritty/issues/1194))
- **Description**: With tmux, mouse wheel sends arrow keys instead of scrolling terminal history
- **Root cause**: Terminal thinks there's a "faux scroll" and sends keys
- **Workaround**: Disable faux scrolling in .tmux.conf: `set -ga terminal-overrides ',*256color*:smcup@:rmcup@'`
- **Lesson**: **Respect smcup/rmcup terminfo capabilities** for entering/exiting alternate screen.

### Keyboard Issues

#### 3. **Vi Mode Doesn't Work with tmux**
- **Description**: Alacritty's vi mode (navigate scrollback with vi keys) doesn't work with tmux by default
- **Root cause**: Vi mode operates on Alacritty's scrollback, but tmux manages its own scrollback
- **Lesson**: **Terminal-level features may conflict with tmux**. Document interaction.

#### 4. **Hotkeys Stop Working** ([#3516](https://github.com/tmux/tmux/issues/3516))
- **Description**: Some Alacritty hotkeys (Shift+PageUp/Down/Home/End) stop working under tmux, returning escape sequences instead
- **Root cause**: Key binding precedence between terminal and tmux
- **Lesson**: **Provide terminfo entries that tmux can use** to understand terminal capabilities.

---

## Rendering Architecture Lessons

### Performance Characteristics

#### 1. **Full Screen Redraw by Design**
- **Description**: Alacritty redraws entire screen every frame because "it's so cheap"
- **Philosophy**: Simplicity over optimization
- **Reality**: Doesn't scale to 4K+ displays or high refresh rates
- **Evolution**: Eventually added damage tracking in v0.11.0
- **Lesson**: **Start with damage tracking from day one**. 4K/5K displays are common now.

#### 2. **Empty Cell Processing Inefficiency** ([#5300](https://github.com/alacritty/alacritty/issues/5300))
- **Description**: Alacritty creates renderable cells for every visible cell, only filtering empty cells after transformation
- **Impact**: Significantly slows down rendering with large grids that are mostly empty
- **Lesson**: **Skip empty cells early in the pipeline**. Check cell content before building render data.

#### 3. **Partial Rendering** ([#5843](https://github.com/alacritty/alacritty/issues/5843))
- **Description**: After landing damage tracking, explored using EGL_EXT_buffer_age to benefit platforms beyond Wayland
- **Lesson**: **Damage tracking improves performance across all platforms**, not just Wayland.

### GPU Architecture

#### 4. **Efficient OpenGL Usage**
- **Success**: ~500 FPS with large screen full of text
- **Techniques**:
  - Minimize state changes
  - Rasterize glyphs once, store in texture atlas
  - Upload instance data once per frame
  - Render in only two draw calls
- **Lesson**: **Batch rendering is critical**. Crux should adopt similar approach with Metal.

#### 5. **sRGB Color Space Issues** ([#3756](https://github.com/alacritty/alacritty/issues/3756))
- **Description**: Subpixel fonts not rendered correctly due to sRGB handling
- **Root cause**: Linear vs sRGB color space confusion in rendering pipeline
- **Lesson**: **Metal requires explicit sRGB texture formats**. Don't assume linear RGB.

#### 6. **vsync Configuration**
- **Issue**: Kitty does vsync differently, potentially better
- **Lesson**: **Make vsync configurable**. Some users prefer uncapped FPS, others want smooth scrolling.

### Damage Tracking Implementation

#### 7. **Damage Tracking Bugs** ([#8220](https://github.com/alacritty/alacritty/issues/8220))
- **Issues found**:
  - Off-by-one errors in damage regions
  - Writing to non-existing line when column value changes during resize
  - Highlight invalidation on grid scroll (underline appears on wrong line)
  - URL highlights not constrained to grid, assumption fails on resize
- **Lesson**: **Damage tracking is subtle**. Must handle:
  - Grid resize
  - Scroll regions
  - Cursor movement
  - Selection changes
  - Hints/overlays

---

## Configuration System Issues

### YAML to TOML Migration

#### 1. **Breaking Configuration Migration** ([#6592](https://github.com/alacritty/alacritty/issues/6592))
- **Change**: Version 0.13.0 switched from YAML to TOML
- **Tool**: `alacritty migrate` command with dry-run option
- **Issues**:
  - Loss of advanced YAML features (anchors)
  - Many "unused config" warnings after migration ([#6999](https://github.com/alacritty/alacritty/issues/6999))
  - Some configs failed to parse during migration ([#7289](https://github.com/alacritty/alacritty/issues/7289))
- **Lesson**: **Choose config format carefully upfront**. Migration is painful. TOML is more standard but less expressive.

#### 2. **Deprecated Options**
- **Examples**:
  - `draw_bold_text_with_bright_colors` → `colors.draw_bold_text_with_bright_colors`
  - `key_bindings` → `keyboard.bindings`
  - `mouse_bindings` → `mouse.bindings`
- **Lesson**: **Namespace config keys from the start**. Flat structure causes conflicts.

---

## Scrollback Buffer Issues

### Implementation Problems

#### 1. **Alternative Screen Buffer Not Supported** ([#1000](https://github.com/alacritty/alacritty/issues/1000))
- **Description**: Initial scrollback implementation didn't support tmux (alternate screen)
- **Impact**: tmux scrollback completely broken
- **Lesson**: **Separate scrollback buffers for normal and alternate screens** from day one.

#### 2. **Buffer Clearing Incomplete** ([#6809](https://github.com/alacritty/alacritty/issues/6809))
- **Description**: Cmd-K clears visible buffer but can still scroll back to see it. Requires Cmd-K twice.
- **Root cause**: First clear hides buffer off-screen instead of actually clearing
- **Lesson**: **Clear command must actually free memory**, not just scroll viewport.

#### 3. **Excessive Memory Pre-allocation** ([#1236](https://github.com/alacritty/alacritty/issues/1236))
- **Description**: Alacritty pre-allocates 191MB for 20k lines vs tmux's 34MB
- **Root cause**: Over-eager memory allocation
- **Lesson**: **Allocate scrollback lazily** or in chunks. Most users never scroll back 20k lines.

#### 4. **Buffer Lost on Window State Change** ([#7074](https://github.com/alacritty/alacritty/issues/7074))
- **Description**: Windows version deletes scrollback when resuming from hidden state (Maximized/Fullscreen)
- **Root cause**: Likely improper state restoration on minimize/restore
- **Lesson**: **Scrollback must persist across all window state changes**.

#### 5. **Runtime Resizing Not Supported** ([#1235](https://github.com/alacritty/alacritty/issues/1235))
- **Description**: Can't change scrollback size at runtime via live-config-reload
- **Lesson**: **Design for runtime reconfiguration**. Growing/shrinking scrollback should work dynamically.

---

## Key Takeaways for Crux

### Must-Avoid Bugs

1. **IME Preedit to PTY**: Never send composition/preedit text to PTY. Only committed text. This causes all the CJK bugs.

2. **Selection Underflow**: Clamp selection bounds to valid ranges. Integer underflow crashes terminals.

3. **GPU Resource Leaks**: Always free old resources before allocating new. Implement Drop for GPU wrappers.

4. **Display Switching**: Monitor for display change events and recalculate font/DPI metrics.

5. **Fullscreen Context Loss**: Metal contexts must be preserved when creating new windows.

6. **Empty Cell Processing**: Filter empty cells early in render pipeline, not after transformation.

7. **Damage Tracking Edge Cases**: Handle grid resize, scroll regions, cursor, selection, overlays.

### Architectural Decisions

1. **Start with Damage Tracking**: Don't rely on "full redraw is cheap" - 4K/5K displays exist.

2. **Separate Normal/Alternate Scrollback**: Required for tmux compatibility.

3. **Font Fallback Order**: Emoji fonts before monospace for emoji codepoints.

4. **Namespace Config Keys**: Use hierarchical structure (e.g., `window.opacity`, not `opacity`).

5. **TOML Over YAML**: More standard, better tooling, despite losing anchors.

6. **VT220+ Support**: Minimum viable VT standard for modern terminal apps.

7. **vsync Configurable**: Some users want performance, others want smoothness.

### Testing Strategy

1. **vttest in CI**: Catches VT emulation edge cases.

2. **Memory Leak Detection**: Profile over extended periods, especially high refresh rates.

3. **Multi-Display Testing**: Test Retina ↔ non-Retina transitions.

4. **IME Test Suite**: Test Korean, Japanese, Chinese input modes with all edge cases.

5. **Resize Testing**: Test both horizontal and vertical resize separately.

6. **Long-Running Sessions**: Clipboard, memory, state corruption appear after hours/days.

### Crux Advantages

1. **GPUI Handles Much**: Window management, Metal rendering, input events likely better than Alacritty's OpenGL.

2. **Newer Codebase**: Learn from 7+ years of Alacritty bugs without legacy baggage.

3. **Focused Scope**: macOS-only avoids cross-platform complexity that caused many Alacritty bugs.

4. **IME Priority**: Phase 3 explicit focus on Korean/CJK means we can get it right from the start.

5. **Native Tabs**: Alacritty punted to window managers; Crux embracing tabs for Claude Code integration.

6. **Modern Standards**: Supporting Kitty keyboard protocol, graphics protocols from day one.

### Crux Risks

1. **GPUI Pre-1.0**: Breaking changes between versions. Pin carefully.

2. **Metal-Specific**: Less community knowledge than OpenGL. Rendering bugs may be harder to diagnose.

3. **Font Rendering**: Still need to handle font metrics, texture atlases, emoji fallback ourselves.

4. **IME on GPUI**: Unclear if GPUI has good NSTextInputClient integration. May need manual implementation.

---

## Sources

### GitHub Issues - Top Engagement
- [Ligature Support](https://github.com/alacritty/alacritty/issues/50)
- [Sixel Graphics](https://github.com/alacritty/alacritty/issues/910)
- [Multiple Windows](https://github.com/alacritty/alacritty/issues/607)
- [Scrollback Implementation](https://github.com/alacritty/alacritty/issues/124)
- [Tabs Support](https://github.com/alacritty/alacritty/issues/3129)

### CJK/IME Issues
- [Keyboard Input with CJK IME](https://github.com/alacritty/alacritty/issues/6942)
- [Double Space with CJK](https://github.com/alacritty/alacritty/issues/8079)
- [Japanese Input Issue](https://github.com/alacritty/alacritty/issues/1101)
- [Korean IME Freeze](https://github.com/alacritty/alacritty/issues/4469)
- [Japanese Keyboard Problem](https://github.com/alacritty/alacritty/issues/7167)

### macOS-Specific Issues
- [Font Size on Retina Displays](https://github.com/alacritty/alacritty/issues/3732)
- [Moving Between Displays](https://github.com/alacritty/alacritty/issues/1069)
- [Font Rendering Quality](https://github.com/alacritty/alacritty/issues/7333)
- [Subpixel Rendering](https://github.com/alacritty/alacritty/issues/3756)
- [Window Flickering on Resize](https://github.com/alacritty/alacritty/issues/7898)
- [Redraw Flicker When Shrinking](https://github.com/alacritty/alacritty/issues/8549)
- [Fullscreen Menu Bar](https://github.com/alacritty/alacritty/issues/4105)
- [Fullscreen Startup Alternates](https://github.com/alacritty/alacritty/issues/3797)
- [App Switching Focus](https://github.com/alacritty/alacritty/issues/3659)
- [New Window Turns Old Window Black](https://github.com/alacritty/alacritty/issues/7980)

### Memory and Performance
- [Memory Leaks When Idle](https://github.com/alacritty/alacritty/issues/6210)
- [Memory Leak 80MB to 800MB](https://github.com/alacritty/alacritty/issues/4806)
- [Text Selection Memory Leak](https://github.com/alacritty/alacritty/issues/1640)
- [Font Zoom Memory Leak](https://github.com/alacritty/alacritty/issues/4815)
- [Slower Redraw with Bigger Window](https://github.com/alacritty/alacritty/issues/3851)
- [Slow Unicode Performance](https://github.com/alacritty/alacritty/issues/2858)
- [Input Latency](https://github.com/alacritty/alacritty/issues/673)
- [Severe Input Lag](https://github.com/alacritty/alacritty/issues/5883)

### Font Rendering
- [Incorrect Font Spacing](https://github.com/alacritty/alacritty/issues/1881)
- [Kerning Issues](https://github.com/alacritty/alacritty/issues/7043)
- [Font Too Wide](https://github.com/alacritty/alacritty/issues/3293)
- [Fractional Font Sizes](https://github.com/alacritty/alacritty/issues/2780)

### VT Emulation and Unicode
- [vttest Failures](https://github.com/alacritty/alacritty/issues/240)
- [Double-Width Emoji as Single-Width](https://github.com/alacritty/alacritty/issues/6144)
- [Emoji Rendering Issues](https://github.com/alacritty/alacritty/issues/7114)
- [Unicode Crash](https://github.com/alacritty/alacritty/issues/473)
- [VT Terminal Features Comparison](https://babbagefiles.xyz/terminal-emulator-vtt-features-compatibility/)

### Clipboard and Mouse
- [Copy/Paste Doesn't Work](https://github.com/alacritty/alacritty/issues/2383)
- [Shift+Right-Click Issue](https://github.com/alacritty/alacritty/issues/4132)
- [PasteSelection on macOS](https://github.com/alacritty/alacritty/issues/681)
- [Clipboard Lags Behind](https://github.com/alacritty/alacritty/issues/3601)
- [Right-Click Paste Also Selects](https://github.com/alacritty/alacritty/issues/5236)

### tmux Compatibility
- [tmux Scrollback Doesn't Work](https://github.com/alacritty/alacritty/issues/5374)
- [Alternative Screen Buffer Support](https://github.com/alacritty/alacritty/issues/1000)
- [Faux Scroll Detection](https://github.com/alacritty/alacritty/issues/1194)
- [Hotkeys Stop Working with tmux](https://github.com/tmux/tmux/issues/3516)

### Architecture and Rendering
- [Empty Cell Processing](https://github.com/alacritty/alacritty/issues/5300)
- [Partial Rendering](https://github.com/alacritty/alacritty/issues/5843)
- [Damage Tracking PR](https://github.com/alacritty/alacritty/pull/5773)
- [Highlight Invalidation Fix](https://github.com/alacritty/alacritty/pull/8220)
- [Announcing Alacritty](https://jwilm.io/blog/announcing-alacritty/)

### Configuration
- [Switch to TOML](https://github.com/alacritty/alacritty/issues/6592)
- [TOML Migration Guide](https://medium.com/@pachoyan/migrate-alacritty-terminal-configuration-yaml-to-toml-for-0-13-x-versions-67fda01be18c)
- [Example TOML Config](https://github.com/alacritty/alacritty/issues/6999)

### Scrollback
- [Alternative Screen Support](https://github.com/alacritty/alacritty/issues/1000)
- [Buffer Clearing Incomplete](https://github.com/alacritty/alacritty/issues/6809)
- [Memory Pre-allocation](https://github.com/alacritty/alacritty/issues/1236)
- [Buffer Lost on State Change](https://github.com/alacritty/alacritty/issues/7074)
- [Runtime Resizing](https://github.com/alacritty/alacritty/issues/1235)

### True Color
- [True Color Support](https://github.com/alacritty/alacritty/issues/109)
- [True Color with tmux](https://gist.github.com/andersevenrud/015e61af2fd264371032763d4ed965b6)
- [COLORTERM Environment Variable](https://github.com/alacritty/alacritty/issues/1526)

---

**Document Status**: Initial research complete
**Next Steps**: Review with Crux architecture decisions in Phase 1-3 planning
**Related Tasks**: Phase 3 IME implementation, Phase 2 scrollback, Phase 1 rendering pipeline

---

## Resolved Issues — Root Cause & Fix Analysis

> 아래는 Alacritty에서 **해결된** 주요 버그들의 근본 원인과 수정 방법 분석이다.
> 같은 실수를 반복하지 않기 위해 수정 PR과 코드 레벨 교훈을 포함한다.

**Research Date**: 2026-02-12
**Methodology**: Searched GitHub for highly-voted closed bugs across categories: IME/CJK, memory leaks, macOS rendering, scrollback/selection, clipboard, VT emulation, performance.

---

## Table of Contents (Resolved Issues)

1. [IME & CJK Input Issues](#1-ime--cjk-input-issues-resolved)
2. [Memory Leaks](#2-memory-leaks-resolved)
3. [Rendering Issues (macOS/Metal)](#3-rendering-issues-macosmetal-resolved)
4. [Selection & Mouse Handling](#4-selection--mouse-handling-resolved)
5. [Clipboard & Paste Security](#5-clipboard--paste-security-resolved)
6. [VT Emulation & Escape Sequences](#6-vt-emulation--escape-sequences-resolved)
7. [Scrollback & Grid Management](#7-scrollback--grid-management-resolved)
8. [Performance & Damage Tracking](#8-performance--damage-tracking-resolved)
9. [Unicode Width Handling](#9-unicode-width-handling-resolved)
10. [Color Rendering](#10-color-rendering-resolved)
11. [High CPU Usage Issues](#11-high-cpu-usage-issues-resolved)
12. [Summary Table: Top 20 Bugs](#summary-table-top-20-resolved-bugs)
13. [Architectural Lessons for Crux](#architectural-lessons-for-crux-from-resolved-bugs)

---

## 1. IME & CJK Input Issues (Resolved)

### 1.1 Korean IME Double-Space Bug

**Issue**: [#8079](https://github.com/alacritty/alacritty/issues/8079)

**Symptom**: When using Korean IME on macOS, pressing spacebar once results in two spaces appearing.

**Root Cause**:
- macOS Korean IME generates **dual events** for a single spacebar press:
  1. `Ime(Commit(" "))` - preedit immediately committed
  2. `KeyboardInput { text: Some(" ") }` - keyboard event also contains space
- Both events output a space, resulting in duplicates
- **Design mismatch**: Korean input differs from Japanese/Chinese where space triggers conversion. In Korean, "pressing space in preedit state should simply input a space without any special meaning"

**Fix**: **UNRESOLVED** as of reporting. Issue originates in winit's macOS IME implementation or OS IME layer.

**Workaround**: Users can switch to third-party Korean input methods like [Gureum](https://github.com/gureum/gureum), which don't exhibit this behavior.

**Lesson for Crux**:
- IME event handling must distinguish between composition-triggering spaces (CJK conversion) and literal spaces (Korean)
- Need to deduplicate events when IME Commit and KeyboardInput both contain text
- Test with ALL CJK languages, not just Japanese/Chinese
- Consider platform-specific IME quirks in event routing logic

**Related**: Issue [#6942](https://github.com/alacritty/alacritty/issues/6942) (broader CJK IME keyboard input problems)

---

### 1.2 IME Preedit Clearing Bug

**Issue**: [#6313](https://github.com/alacritty/alacritty/issues/6313)

**Symptom**: When using inline IME with fcitx5, holding Backspace clears the IME popup but leaves the first character visible on screen. After disabling IME, further input becomes unresponsive.

**Root Cause**: Incomplete Wayland text-input protocol handling:
- "When you disable IME, your ime doesn't actually result in `text_input::leave` event"
- Alacritty believes IME is active when it's disabled
- Application never receives **empty preedit events** that should occur when backspacing through all characters

**Fix**: **RESOLVED** in v0.11.0 via PR [#6326](https://github.com/alacritty/alacritty/pull/6326) ("Bump winit to 0.27.3")
- Fix was in upstream winit's Wayland text-input handling
- Protocol now correctly sends empty preedit on full deletion
- Properly fires `text_input::leave` on IME disable

**Technical Context**:
- Wayland text-input protocol specifies applications should reset preedit strings on certain events
- A `commit` action implies an empty preedit state
- Alacritty clears preedit on commit to allow continued IME data reception

**Lesson for Crux**:
- GPUI's IME implementation must handle **empty preedit events** explicitly
- Test IME disable/re-enable cycles thoroughly
- Verify preedit state resets on all commit types
- Document expected event sequences for each IME protocol edge case

---

## 2. Memory Leaks (Resolved)

### 2.1 Selection Memory Leak with `less`

**Issue**: [#1640](https://github.com/alacritty/alacritty/issues/1640)

**Symptom**: Selecting text while using `less` with large files causes terminal freeze and consumes 25.5GB+ of memory when selection exceeds screen edges.

**Root Cause**:
- Improper handling of selection rotation during viewport scrolling
- "Underflow that happens when the selection goes below 0"
- Negative indices caused unbounded memory allocation or infinite loops in selection rendering logic

**Fix**: **RESOLVED** via PR [#1658](https://github.com/alacritty/alacritty/pull/1658) - "Fix rotation of selection below 0"

**Technical Implementation**:
```rust
// BEFORE: Using unsigned integers (cannot represent negative)
type Line = usize;

// AFTER: Using signed integers
type Line = isize;
```

**Two-phase approach**:
1. **During Selection Updates**: Use `isize` to preserve negative coordinates as selections move beyond visible boundaries
2. **During Display Conversion**: "Once the selection is converted to a span, the lines are clamped to the visible region" - restrict negative values to valid viewport range

**Lesson for Crux**:
- Use **signed integers** for grid coordinates that can temporarily go negative during scrolling
- Clamp to visible region only at render time, not during state updates
- Selection must track origin outside visible area (critical for alternate screen buffers)
- Add test cases for selection underflow scenarios

**Code Pattern for Crux**:
```rust
// Internal representation allows negative indices
struct Selection {
    start: Point<isize>,
    end: Point<isize>,
}

// Conversion to visible span clamps coordinates
impl Selection {
    fn to_visible_span(&self, viewport: &Viewport) -> Span {
        Span {
            start: self.start.clamp_to(viewport),
            end: self.end.clamp_to(viewport),
        }
    }
}
```

---

## 3. Rendering Issues (macOS/Metal) (Resolved)

### 3.1 Font Rendering Crash (Metal Texture Descriptor)

**Issue**: [#7915](https://github.com/alacritty/alacritty/issues/7915)

**Symptom**: Alacritty crashes on macOS when loading certain fonts. Error: "MTLTextureDescriptor has width of zero" (Metal framework assertion).

**Root Cause**:
- Font metrics calculation produces zero-width texture descriptor
- Crash sequence:
  1. Font loading initialization
  2. OpenGL context attachment via CGL
  3. Metal texture creation for glyph rendering
  4. **Assertion failure** due to zero-width texture descriptor

- Affects both custom fonts (MesloLG Nerd Font Mono) and system fonts (Menlo)
- Problem in font metrics calculation, not font compatibility

**Fix**: **UNRESOLVED** as of v0.14.0-dev

**Workaround**: Modify config file after application launch with 2-second delay, changing font settings before rendering occurs.

**Technical Context**:
- Alacritty uses Glutin with Apple's CGL (Core OpenGL) backend
- CGL translates OpenGL → Metal on modern macOS
- Engine creates texture descriptors for glyph caching during display init
- Metal validation layer rejects descriptors with zero dimensions

**Lesson for Crux**:
- **Validate font metrics BEFORE creating texture descriptors**
- Add defensive checks: `assert!(width > 0 && height > 0)` before Metal calls
- Consider fallback fonts if primary font yields invalid metrics
- GPUI uses Metal directly - ensure glyph cache texture creation validates dimensions

**Defensive Code Pattern**:
```rust
fn create_glyph_texture(&self, metrics: &FontMetrics) -> Result<Texture> {
    if metrics.width == 0 || metrics.height == 0 {
        return Err(Error::InvalidFontMetrics {
            font: metrics.font_name.clone(),
            width: metrics.width,
            height: metrics.height,
        });
    }
    // Proceed with Metal texture creation
}
```

---

### 3.2 Retina Display Rendering Issues

**Issue**: [#1802](https://github.com/alacritty/alacritty/issues/1802)

**Symptom**: Rendering breaks on external ultrawide monitor (3440×1440). Terminal only uses "bottom-left quadrant" with content scaled down.

**Root Cause**:
- macOS fails to emit high-DPI scaling events when windows transition between displays with different pixel densities
- Logs show `device_pixel_ratio: 1` despite ultrawide requiring higher DPI compensation
- "External monitor with highdpi scaling is used, but no HiDPI event is emitted when the window changes the screen"

**Fix**: **RESOLVED** as duplicate of [#1631](https://github.com/alacritty/alacritty/issues/1631)
- Fix details referenced in #1631 (not detailed in #1802 thread)

**Technical Context**:
- Rendering pipeline relies on accurate `device_pixel_ratio` detection to scale glyphs and calculate terminal dimensions
- Moving windows between MacBook display and external monitor doesn't refresh DPI info
- Problem exists regardless of window decoration settings (full/none)

**Lesson for Crux**:
- **Actively poll for DPI changes** when window moves between displays
- Don't rely solely on windowing system DPI events
- GPUI's `WindowContext` should provide DPI - verify it handles cross-display moves
- Test on multi-monitor setups with different scaling factors

---

### 3.3 Mouse Position Bug on Retina

**Issue**: [#3191](https://github.com/alacritty/alacritty/issues/3191)

**Symptom**: Mouse text selection offset from actual cursor position on Retina displays (master branch, not in v0.4.1).

**Root Cause**:
- Introduced in commit c34ec12
- Mouse coordinate calculations didn't account for 2x pixel density multiplier
- DPI scaling not applied to mouse events

**Fix**: **RESOLVED** via upstream fix in winit (rust-windowing/winit#1389)
- Required updating Alacritty's winit dependency
- DPI scaling correction applied to mouse coordinates

**Workaround**: Patch Cargo.toml:
```toml
[patch.crates-io]
winit = { git = 'https://github.com/rust-windowing/winit' }
```

**Lesson for Crux**:
- **Apply DPI scaling to ALL input coordinates**, not just rendering
- GPUI handles mouse events - verify DPI scaling is applied before coordinate conversion
- Test on Retina/HiDPI displays for input accuracy
- Cell coordinate calculation: `cell = (mouse_pos * dpi_ratio) / cell_size`

---

## 4. Selection & Mouse Handling (Resolved)

### 4.1 Selection Rotation Underflow (covered in 2.1)

See "Selection Memory Leak with `less`" above - same fix applies to selection rendering.

---

## 5. Clipboard & Paste Security (Resolved)

### 5.1 Empty Clipboard Paste Error

**Issue**: [#2389](https://github.com/alacritty/alacritty/issues/2389)

**Symptom**: Pasting from empty clipboard (e.g., after reboot) throws error on macOS instead of handling gracefully.

**Root Cause**:
- macOS returns error when clipboard is empty, but **not** the expected "Empty" error type
- System returns standard boxed errors from stdlib
- These were logged as critical errors instead of expected edge cases

**Fix**: **RESOLVED** via PR [#2391](https://github.com/alacritty/alacritty/pull/2391)

**Technical Implementation**:
- Changed error handling: clipboard loading failures now log to **debug log** instead of error log
- Rationale: "Loading clipboard data usually should not fail, so we do not log it as error if it fails but just print it to the debug log instead"
- Distinguish between genuine failures (user attention) vs. expected edge cases (silent handling)

**Lesson for Crux**:
- Empty clipboard is **expected state**, not error
- NSPasteboard operations should return `Option<String>` not `Result`
- Log at DEBUG level for empty clipboard, ERROR for actual failures
- Never panic or show error dialogs for empty paste

**Code Pattern**:
```rust
fn read_clipboard(&self) -> Option<String> {
    match self.pasteboard.read() {
        Ok(contents) => Some(contents),
        Err(PasteboardError::Empty) => {
            log::debug!("Clipboard is empty");
            None
        }
        Err(e) => {
            log::error!("Failed to read clipboard: {}", e);
            None
        }
    }
}
```

---

### 5.2 Bracketed Paste Security Vulnerability

**Issue**: [#800](https://github.com/alacritty/alacritty/issues/800)

**Symptom**: Malicious content containing END PASTE sequence can bypass bracketed paste mode protection, executing commands without user confirmation.

**Root Cause**: Insufficient filtering of terminal control sequences during paste operations.

**The Exploit**:
- Bracketed paste mode wraps pasted text: `\e[200~<pasted text>\e[201~`
- Malicious content: `malicious_command\e[201~\r`
- When pasted:
  1. Bracketed paste starts: `\e[200~malicious_command`
  2. Embedded END PASTE: `\e[201~` **prematurely terminates paste mode**
  3. Remaining text: `\r` executes the command
- Bypasses safety mechanism requiring explicit Enter key

**Fix**: **RESOLVED** via PR [#1243](https://github.com/alacritty/alacritty/pull/1243) - "Fix Bracketed Paste Mode when input contains end sequence"

**Technical Implementation**:
- Filter/escape the END PASTE sequence (`\e[201~`) from pasted content
- Commit ff5081d in paste handling code
- Strip sequence before delivery to running application

**Lesson for Crux**:
- **Always filter control sequences from pasted text** in bracketed paste mode
- Escape or remove: `\e[200~`, `\e[201~`, and other paste-related sequences
- Security review clipboard path for injection vectors
- Test with malicious payloads before production

**Reference**: [thejh.net/misc/website-terminal-copy-paste](http://thejh.net/misc/website-terminal-copy-paste) (original exploit disclosure)

---

## 6. VT Emulation & Escape Sequences (Resolved)

### 6.1 Truecolor Support

**Issue**: [#109](https://github.com/alacritty/alacritty/issues/109)

**Symptom**: Request for 24-bit truecolor support to simplify color scheme configuration.

**Fix**: **RESOLVED** - implemented natively in Alacritty

**Escape Sequence Format**:
- Foreground: `\e[38;2;R;G;Bm` (RGB 0-255)
- Background: `\e[48;2;R;G;Bm` (RGB 0-255)

**Configuration Requirements**:
- **Vim/Neovim**: Requires terminal capability declarations in vimrc
- **Tmux**: Needs explicit terminal override settings

**Recommended tmux config**:
```bash
set -g default-terminal "tmux-256color"
set -ag terminal-overrides ",alacritty:RGB"
```

**Challenge**: Tmux doesn't advertise truecolor by default - must explicitly declare "RGB" capability in `terminal-overrides`.

**Lesson for Crux**:
- Implement standard truecolor sequences: `38;2;R;G;B` and `48;2;R;G;B`
- Advertise `RGB` or `Tc` capability in terminfo
- Document tmux integration in user guide
- Test color passthrough with: `curl -s https://gist.githubusercontent.com/lifepillar/09a44b8cf0f9397465614e622979107f/raw/24-bit-color.sh | bash`

---

### 6.2 Alternate Screen Buffer Reset Bug

**Issue**: [#2145](https://github.com/alacritty/alacritty/issues/2145)

**Symptom**: After entering alternate screen (`echo -e "\e[?1049h"`) and running `reset`, terminal clears but remains in alternate screen. "No scrollback history available, even after trying to switch out of alt screen buffer again."

**Root Cause**:
- Terminal failed to swap grid buffers during reset operation
- State management issue where reset didn't restore normal screen

**Fix**: **RESOLVED** via PR [#2146](https://github.com/alacritty/alacritty/pull/2146)

**Technical Implementation (two fixes)**:

1. **Primary Fix**: When resetting in alternate screen, properly swap out grids
   - Ensures scrollback functionality remains unaffected by reset
   - Maintains correct grid state

2. **Secondary Fix**: Prevent cursor jumping when exiting alternate screen even when it's not active
   - "Skipping all alt screen swap routines unless the current state matches the expected state"
   - Conditional execution of swap logic

**Lesson for Crux**:
- **Track alternate screen state explicitly** (enum: Normal, Alternate)
- Reset command must check current screen and swap grids accordingly
- Don't execute swap operations if already in expected state (idempotent)
- Preserve scrollback in normal buffer when switching to/from alternate

**State Machine**:
```rust
enum ScreenMode {
    Normal,
    Alternate,
}

impl Terminal {
    fn reset(&mut self) {
        match self.mode {
            ScreenMode::Alternate => {
                // Swap back to normal grid BEFORE reset
                self.swap_to_normal_screen();
                self.clear_normal_grid();
            }
            ScreenMode::Normal => {
                self.clear_normal_grid();
            }
        }
    }
}
```

---

## 7. Scrollback & Grid Management (Resolved)

### 7.1 Scrollback Wrapping After Resize

**Issue**: [#8036](https://github.com/alacritty/alacritty/issues/8036)

**Symptom**: Scrollback content missing or incorrectly wrapped after resizing window on Windows.

**Root Cause**: Not fully documented in issue thread.

**Fix**: **CLOSED** as completed, but no technical resolution details provided
- Labeled as "Windows issue"
- User requested mintty's "RewrapOnResize" feature for text reflow

**Technical Context**:
- Terminal initialized: 6×14 cell size, 30×120 PTY dimensions, OpenGL 3.3 (WGL)
- No explicit grid reflow mechanism documented

**Lesson for Crux**:
- **Implement grid reflow** (rewrap) on window resize
- When columns change: recalculate line breaks in scrollback
- Preserve semantic line boundaries (hard vs. soft line breaks)
- Test with: long lines, URLs, multi-column characters after resize
- This is a KNOWN HARD PROBLEM - see Zed's terminal reflow implementation

**Reflow Algorithm** (conceptual):
```rust
fn reflow_scrollback(&mut self, old_cols: usize, new_cols: usize) {
    let mut new_scrollback = Vec::new();
    let mut current_line = String::new();

    for row in &self.scrollback {
        current_line.push_str(&row.text);

        if row.is_hard_break {
            // Rewrap current_line into new_cols width
            new_scrollback.extend(wrap_text(&current_line, new_cols));
            current_line.clear();
        }
    }

    self.scrollback = new_scrollback;
}
```

---

## 8. Performance & Damage Tracking (Resolved)

### 8.1 Damage Tracking Implementation

**PR**: [#5773](https://github.com/alacritty/alacritty/pull/5773)

**Feature**: Add damage tracking and reporting to compatible compositors

**Implementation Approach**:

**Core Strategy**: Line-based damage tracking (not per-cell)
- Maintain damage bounds (leftmost and rightmost affected cells) for each terminal line
- "We create empty information about damage state of each line and store it on terminal and update damage bounds with left most and right most damaged cells"

**Performance Characteristics**:
- No regression: "The approach is performing the same as master for me"
- Benefits for 4K/large displays: significantly reduced compositing workload
- Improves battery life and reduces latency in remote connections (VNC, RDP, Waypipe)

**Fallback Strategy**:
- Complex scenarios revert to full-screen damage
- "In cases where damaging becomes complicated we fallback to damaging just entire screen (e.g., when scrolling)"

**Debugging**: Highlight damaged regions via `alacritty -o debug.highlight_damage=true`

**Lesson for Crux**:
- GPUI likely has damage tracking built-in - verify and use it
- If implementing custom: track dirty **lines** (not individual cells) for performance
- Fallback to full-frame damage for complex operations (scrolling, resize)
- Expose debug visualization for development
- Critical for remote desktop performance and battery life

**Data Structure**:
```rust
struct LineDamage {
    left: Option<usize>,   // Leftmost damaged cell
    right: Option<usize>,  // Rightmost damaged cell
}

struct Grid {
    lines: Vec<Row>,
    damage: Vec<LineDamage>,  // Parallel array
}

impl Grid {
    fn damage_cell(&mut self, line: usize, col: usize) {
        let damage = &mut self.damage[line];
        damage.left = Some(damage.left.map_or(col, |l| l.min(col)));
        damage.right = Some(damage.right.map_or(col, |r| r.max(col)));
    }
}
```

---

## 9. Unicode Width Handling (Resolved)

### 9.1 Unicode Character Width Bug

**Issue**: [#265](https://github.com/alacritty/alacritty/issues/265)

**Symptom**: Alacritty treats all characters as single-cell width, causing CJK characters to overlap. "ありがとう" occupies 5 cells instead of 10.

**Root Cause**:
- Terminal lacked proper width calculation logic
- Unicode defines character widths: 0 (combining), 1 (ASCII/halfwidth), or 2 (CJK/fullwidth)
- No equivalent of C's `wcwidth(3)` function

**Impact**:
- CJK characters overlap visually
- Unexpected line wrapping triggers tmux display corruption
- Entire window display corrupted in some cases

**Fix**: **RESOLVED** by adopting Rust's `unicode-width` crate
- Provides `width()` function equivalent to `wcwidth(3)`
- Uses Unicode 9 width tables
- Handles combining characters (0 width), East Asian ambiguous, private-use characters

**Complexities**:
- **Combining characters**: 0 cells (stack atop preceding character)
- **East Asian ambiguous**: Configuration option needed (iTerm defaults to 1 cell)
- **Private-use characters**: Special handling required

**Lesson for Crux**:
- Use `unicode-width` crate for character width calculations
- `str.width()` for string width, `UnicodeWidthChar::width()` for individual chars
- **Test with CJK text** to verify 2-cell rendering
- Handle combining marks (diacritics) with 0 width
- Consider configuration for ambiguous-width characters (East Asian vs. Western context)

**Code Pattern**:
```rust
use unicode_width::UnicodeWidthChar;

fn cell_width(c: char) -> usize {
    c.width().unwrap_or(1)  // Default to 1 for undefined
}

// When rendering:
let mut col = 0;
for ch in text.chars() {
    let width = cell_width(ch);
    render_char(ch, col, row);
    col += width;
}
```

---

### 9.2 Ambiguous Width Character Configuration

**PR**: [#1049](https://github.com/alacritty/alacritty/pull/1049) (CLOSED in favor of #1295)

**Attempted Solution**: Add `east_asian_fullwidth` config option
- `false`: Use `width()` function (Western context)
- `true`: Use `width_cjk()` function (CJK context)

**Why It Failed**:
1. **Application-terminal consistency required**: "Terminal emulator and application running on it should use the same `wcwidth` mapping"
2. **Locale dependencies**: Users with non-CJK environments using `east_asian_fullwidth: true` experienced breakage
3. **Not user preference**: "Option not for user preference, but to make alacritty's width calculation consistent with users' systems and locales"

**Preferred Solution**: Use system's `wcwidth()` function (issue #1295)
- Automatically detects correct widths based on system configuration
- Eliminates manual settings
- Ensures application-terminal consistency

**Lesson for Crux**:
- Don't expose "CJK mode" config option - use system locale
- Query system's `wcwidth()` if possible, or use `unicode-width` with locale detection
- **Consistency is critical**: app and terminal must agree on character widths
- Test in both Western (en_US.UTF-8) and CJK (ja_JP.UTF-8, ko_KR.UTF-8, zh_CN.UTF-8) locales

---

## 10. Color Rendering (Resolved)

### 10.1 Truecolor Evolution

**Issues**: [#109](https://github.com/alacritty/alacritty/issues/109), [#1485](https://github.com/alacritty/alacritty/issues/1485)

**Implementation**:
- 256-color mode: `\e[38;5;Nm` (N = 0-255)
- 24-bit truecolor: `\e[38;2;R;G;Bm` (semicolon-separated)

**Evolution** (#1485):
- Old format: `38;2;R;G;B` (semicolon-separated)
- Preferred format: `38:2:R:G:B` (colon-separated) with optional color space argument

**Lesson for Crux**:
- Support BOTH semicolon and colon-separated formats for compatibility
- Parse color sequences: `38;2;R;G;B` and `38:2:R:G:B`
- Implement in VTE parser (likely already in `alacritty_terminal` crate)
- Advertise `RGB` capability in terminfo

---

## 11. High CPU Usage Issues (Resolved)

### 11.1 Idle CPU Usage

**Issues**: [#3775](https://github.com/alacritty/alacritty/issues/3775), [#8413](https://github.com/alacritty/alacritty/issues/8413), [#3108](https://github.com/alacritty/alacritty/issues/3108)

**Symptoms**:
- 100% CPU usage when idle after several days (#3775)
- Busy loop in polling and message handling with repeated EAGAIN errors (#8413)
- 60-70% CPU after couple hours (#1861)
- 19-20 wakeups/s on idle prompt causing battery drain (#3108)

**Root Cause** (inferred from patterns):
- Event loop doesn't properly sleep when no events pending
- Polling with zero timeout instead of blocking
- Repeated syscalls during idle (kevent, recvmsg on FreeBSD: 2774 syscalls/3.4s)

**Fix**: Multiple fixes over versions, but issue recurs
- No single definitive fix documented
- Likely requires event loop tuning in winit or platform-specific event handling

**Lesson for Crux**:
- GPUI's event loop should **block** when no events pending
- Monitor CPU usage in idle state during development
- Use `Instruments.app` (macOS) or `perf` (Linux) to profile wakeups
- Target: <5 wakeups/second when idle
- Disable cursor blink thread when window not focused
- Test: leave terminal idle for 24+ hours, monitor CPU

**Monitoring**:
```bash
# macOS: Check wakeups
sudo powermetrics -i 1000 -n 1 | grep -A 20 "crux"

# Linux: Check syscalls
strace -c -p $(pgrep crux)
```

---

## Summary Table: Top 20 Resolved Bugs

| # | Issue | Category | Impact | Root Cause | Fix | Lesson for Crux |
|---|-------|----------|--------|------------|-----|-----------------|
| 1 | [#1640](https://github.com/alacritty/alacritty/issues/1640) | Memory Leak | Critical | Selection underflow (unsigned → negative) | Use `isize` for coordinates, clamp at render | Use signed ints for grid coords |
| 2 | [#800](https://github.com/alacritty/alacritty/issues/800) | Security | Critical | Bracketed paste injection via END sequence | Filter `\e[201~` from pasted text | Filter control sequences in paste |
| 3 | [#8079](https://github.com/alacritty/alacritty/issues/8079) | IME | High | Korean IME sends dual space events | **UNRESOLVED** (winit issue) | Deduplicate IME+keyboard events |
| 4 | [#265](https://github.com/alacritty/alacritty/issues/265) | Unicode | High | No `wcwidth()` - all chars 1-cell | Use `unicode-width` crate | Use `unicode-width` for CJK |
| 5 | [#3191](https://github.com/alacritty/alacritty/issues/3191) | Mouse | High | Mouse coords didn't scale for Retina | Fixed in winit (DPI scaling) | Apply DPI to ALL input coords |
| 6 | [#5773](https://github.com/alacritty/alacritty/pull/5773) | Performance | Medium | Full-frame redraws every update | Line-based damage tracking | Use GPUI's damage tracking |
| 7 | [#2145](https://github.com/alacritty/alacritty/issues/2145) | VT Emulation | Medium | Reset didn't swap grids in alt screen | Conditional grid swap on reset | Track alt screen state explicitly |
| 8 | [#6313](https://github.com/alacritty/alacritty/issues/6313) | IME | Medium | No empty preedit events on backspace | Fixed in winit 0.27.3 | Handle empty preedit events |
| 9 | [#2389](https://github.com/alacritty/alacritty/issues/2389) | Clipboard | Low | Empty clipboard logged as error | Downgrade to debug log | Empty clipboard is expected state |
| 10 | [#109](https://github.com/alacritty/alacritty/issues/109) | Color | Low | Needed 24-bit color support | Implement `38;2;R;G;B` sequences | Support both `;` and `:` separators |
| 11 | [#1802](https://github.com/alacritty/alacritty/issues/1802) | Rendering | Medium | No DPI event on display change | Fixed in winit (duplicate of #1631) | Poll DPI on window move |
| 12 | [#7915](https://github.com/alacritty/alacritty/issues/7915) | Crash | High | Zero-width Metal texture descriptor | **UNRESOLVED** | Validate font metrics before Metal calls |
| 13 | [#8036](https://github.com/alacritty/alacritty/issues/8036) | Scrollback | Medium | No grid reflow on resize | Fixed (details unclear) | Implement reflow with soft linebreaks |
| 14 | [#3775](https://github.com/alacritty/alacritty/issues/3775) | Performance | Medium | Idle CPU 100% after days | Event loop doesn't block | Block on idle, <5 wakeups/sec |
| 15 | [#1049](https://github.com/alacritty/alacritty/pull/1049) | Unicode | Medium | Ambiguous-width char config failed | Closed for system `wcwidth()` approach | Use system locale, not manual config |
| 16 | [#7965](https://github.com/alacritty/alacritty/pull/7965) | macOS | Low | Opacity breaks title bar transparency | Closed for upstream winit fix | Trust GPUI's window transparency |
| 17 | [#1658](https://github.com/alacritty/alacritty/pull/1658) | Selection | Critical | Selection rotation underflow | Use `isize`, clamp to visible | Same as #1640 |
| 18 | [#2146](https://github.com/alacritty/alacritty/pull/2146) | VT Emulation | Medium | Alt screen state inconsistency | Skip swap if already in state | Idempotent state transitions |
| 19 | [#6326](https://github.com/alacritty/alacritty/pull/6326) | IME | Medium | Wayland text-input protocol gaps | Bump winit to 0.27.3 | Verify GPUI's Wayland IME handling |
| 20 | [#1243](https://github.com/alacritty/alacritty/pull/1243) | Security | Critical | Bracketed paste bypass | Filter END sequence from paste | Security audit clipboard path |

---

## Architectural Lessons for Crux (from Resolved Bugs)

### 1. Dependency on Windowing Library Quality

**Observation**: Many Alacritty bugs originate in **winit**, not Alacritty itself:
- Korean IME double-space (#8079) - winit macOS IME
- Retina mouse position (#3191) - winit DPI scaling
- IME preedit clearing (#6313) - winit Wayland text-input
- macOS transparency (#7965) - winit window management
- Zero-width font crash (#7915) - winit/glutin font metrics

**Crux Advantage**: GPUI is a purpose-built UI framework by Zed, with:
- Direct Metal rendering (no OpenGL translation layer)
- Tight macOS integration (first-class NSTextInputClient)
- Battle-tested in production terminal (Zed's integrated terminal)

**Action Items**:
- Trust GPUI for: DPI handling, window management, IME events, transparency
- Test GPUI edge cases identified in winit bugs (especially IME, multi-monitor)
- Contribute fixes upstream to GPUI if issues found

---

### 2. IME is the Hardest Problem

**Complexity Identified**:
- Multiple event sources: IME Commit, IME Preedit, KeyboardInput
- Platform-specific behavior: Korean ≠ Japanese ≠ Chinese
- Protocol gaps: Wayland text-input incomplete
- State synchronization: preedit overlay vs. committed text

**Critical Requirements**:
1. **Never send preedit to PTY** - render as overlay only
2. **Deduplicate events** - IME Commit + KeyboardInput both firing
3. **Handle empty preedit** - indicates composition cancellation
4. **Verify state transitions** - IME enable/disable must fire correct events

**Testing Checklist**:
- [ ] Japanese (Hiragana → Kanji conversion)
- [ ] Chinese (Pinyin input)
- [ ] Korean (Hangul composition - watch for double-space!)
- [ ] Wayland + fcitx5 (preedit clearing)
- [ ] macOS built-in IMEs for all three languages

**Reference**: `/Users/jjh/Projects/crux/research/platform/ime-clipboard.md` (existing research)

---

### 3. Signed Integers for Grid Coordinates

**Rule**: Use `isize`/`i32` for any coordinate that can temporarily go negative during:
- Selection extending beyond viewport top
- Scrollback navigation
- Grid rotation/scrolling operations

**Rationale**: Prevents underflow → unbounded memory allocation

**Code Pattern**:
```rust
// Internal representation
struct Point {
    line: isize,  // Can be negative during scrolling
    col: usize,   // Never negative
}

// Conversion to visible region
impl Point {
    fn to_display(&self, viewport: &Viewport) -> Option<DisplayPoint> {
        if self.line < 0 || self.line >= viewport.lines as isize {
            None  // Outside visible region
        } else {
            Some(DisplayPoint {
                line: self.line as usize,
                col: self.col.min(viewport.cols - 1),
            })
        }
    }
}
```

---

### 4. Security: Filter Pasted Control Sequences

**Attack Vector**: Malicious clipboard content with embedded escape sequences

**Required Filtering** (bracketed paste mode):
- `\e[200~` (BEGIN PASTE)
- `\e[201~` (END PASTE)
- Any other control sequences? (review security best practices)

**Implementation** (in clipboard paste handler):
```rust
fn paste_text(&mut self, text: String) {
    if self.bracketed_paste_enabled() {
        let filtered = text
            .replace("\x1b[200~", "")  // Remove BEGIN PASTE
            .replace("\x1b[201~", "");  // Remove END PASTE

        self.write_to_pty(format!("\x1b[200~{}\x1b[201~", filtered));
    } else {
        self.write_to_pty(text);
    }
}
```

**Testing**: Copy malicious payloads from [thejh.net](http://thejh.net/misc/website-terminal-copy-paste), verify filtered

---

### 5. Font Metrics Validation

**Defensive Programming**: Validate font metrics **before** Metal texture creation

```rust
fn load_font(&mut self, font_name: &str) -> Result<Font> {
    let metrics = self.calculate_font_metrics(font_name)?;

    // CRITICAL: Validate before Metal calls
    if metrics.cell_width == 0 || metrics.cell_height == 0 {
        return Err(Error::InvalidFontMetrics {
            font: font_name.to_string(),
            width: metrics.cell_width,
            height: metrics.cell_height,
        });
    }

    // Safe to create Metal textures
    self.create_glyph_cache(metrics)
}
```

**Fallback Strategy**:
1. Try user-configured font
2. If invalid metrics → try system monospace font
3. If still invalid → use hardcoded fallback (Menlo 12pt)
4. If even fallback fails → panic with diagnostic

---

### 6. DPI Handling on Multi-Monitor

**Problem**: Moving window between displays with different DPI requires:
1. Recalculating cell size
2. Recreating glyph cache (different rasterization scale)
3. Adjusting mouse coordinate scaling
4. Sending SIGWINCH to PTY (pixel dimensions changed)

**GPUI Integration**:
- GPUI provides `WindowContext::scale_factor()`
- Likely handles DPI change events via `AppContext`
- Verify: Does GPUI fire callback on cross-display window move?

**Testing**:
- MacBook Pro (2x) + external 1080p (1x) + external 4K (2x)
- Drag terminal window between all three displays
- Verify: text remains sharp, mouse selection accurate, PTY size correct

---

### 7. Damage Tracking for Performance

**GPUI Assumption**: GPUI likely has built-in damage tracking as a modern GPU framework

**Verification Needed**:
- Does `gpui::Canvas` track dirty regions?
- How to access damage rect for current frame?
- Fallback: track dirty lines manually if GPUI doesn't provide

**Manual Implementation** (if needed):
```rust
struct TerminalCanvas {
    dirty_lines: BitSet,  // Fast lookup
    full_damage: bool,
}

impl TerminalCanvas {
    fn damage_line(&mut self, line: usize) {
        if !self.full_damage {
            self.dirty_lines.insert(line);
        }
    }

    fn damage_full(&mut self) {
        self.full_damage = true;
        self.dirty_lines.clear();
    }

    fn reset_damage(&mut self) {
        self.dirty_lines.clear();
        self.full_damage = false;
    }
}
```

---

### 8. Unicode Width with unicode-width Crate

**Dependency**: Add to `Cargo.toml`:
```toml
unicode-width = "0.1"
```

**Usage**:
```rust
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

// Single character
let width = 'あ'.width().unwrap_or(1);  // 2

// String (accounts for combining marks)
let width = "Crux".width();  // 4
let width = "한글".width();  // 4 (2 chars × 2 cells)
```

**Edge Cases**:
- Zero-width: combining diacritics, variation selectors
- One-width: ASCII, Latin, halfwidth Katakana
- Two-width: CJK ideographs, fullwidth punctuation, emoji

---

### 9. Alternate Screen State Machine

**States**: Normal, Alternate

**Transitions**:
- `\e[?1049h` - Save cursor, switch to alternate, clear
- `\e[?1049l` - Restore cursor, switch to normal

**Critical Rules**:
1. **Scrollback only exists in Normal screen**
2. **Reset in Alternate must swap to Normal first**
3. **Cursor position saved/restored on transition**
4. **Don't swap if already in target state** (idempotent)

**Implementation**:
```rust
enum ScreenMode {
    Normal,
    Alternate,
}

impl Terminal {
    fn enter_alternate_screen(&mut self) {
        if self.mode == ScreenMode::Normal {
            self.saved_cursor = self.cursor.clone();
            self.alt_grid = Grid::new(self.rows, self.cols);
            self.mode = ScreenMode::Alternate;
        }
    }

    fn exit_alternate_screen(&mut self) {
        if self.mode == ScreenMode::Alternate {
            self.mode = ScreenMode::Normal;
            self.cursor = self.saved_cursor.clone();
            // Normal grid (with scrollback) remains intact
        }
    }
}
```

---

### 10. Idle CPU Usage Monitoring

**Target**: <5 wakeups/second when idle (no input, no output, no cursor blink)

**Profiling Tools**:
```bash
# macOS
sudo powermetrics -i 1000 -n 10 | grep -A 20 "crux"

# Linux
perf record -e 'syscalls:sys_enter_*' -p $(pgrep crux) -- sleep 10
perf report
```

**Common Culprits**:
- Cursor blink timer (should pause when window not focused)
- Event loop polling with zero timeout
- Unnecessary redraws (damage tracking helps)
- Background threads not sleeping

**Optimization Checklist**:
- [ ] Cursor blink thread pauses when unfocused
- [ ] Event loop blocks when no events pending
- [ ] No redraws when grid unchanged
- [ ] PTY read thread uses blocking I/O (not polling)

---

### 11. Grid Reflow on Resize

**Requirement**: When terminal width changes, rewrap scrollback lines

**Challenges**:
- Distinguish hard line breaks (Enter) vs. soft breaks (wrapped)
- Preserve URLs spanning multiple lines
- Handle double-width characters at wrap boundary
- Reflow is computationally expensive for large scrollback

**Approach** (simplified):
```rust
struct Row {
    cells: Vec<Cell>,
    wrapped: bool,  // true if line continues on next row
}

fn reflow(&mut self, new_cols: usize) {
    let mut reflowed = Vec::new();
    let mut buffer = Vec::new();

    for row in &self.scrollback {
        buffer.extend(row.cells.iter().cloned());

        if !row.wrapped {
            // Hard break - rewrap buffer
            reflowed.extend(wrap_cells(&buffer, new_cols));
            buffer.clear();
        }
    }

    self.scrollback = reflowed;
}
```

**Testing**:
- Long lines (200+ chars), resize narrow → wide → narrow
- URLs, code snippets, tables
- CJK text (double-width chars at wrap boundary)

---

### 12. Trust alacritty_terminal Crate

**Good News**: Many bugs are already fixed in `alacritty_terminal` crate (Crux's dependency)

**What's Inherited**:
- VT100/xterm parsing (escape sequences, CSI, OSC, DCS)
- Grid management (scrollback, alternate screen)
- Selection logic (with #1658 fix)
- Unicode width handling (with #265 fix)
- Color support (256-color, truecolor)

**What Crux Must Implement**:
- GPUI rendering (canvas, glyph cache, cursor)
- Input encoding (keyboard → VT sequences)
- IME overlay (preedit rendering)
- Clipboard integration (NSPasteboard, security filtering)
- IPC protocol (pane control for Claude Code)

**Strategy**:
- Use `alacritty_terminal::Term` for VT emulation
- Don't reimplement grid, scrollback, or escape sequence parsing
- Focus Crux effort on GPUI integration and macOS-specific features

---

## Conclusion (Resolved Bugs Analysis)

Alacritty's resolved bugs provide a **treasure trove** of lessons for Crux:

1. **Dependency Quality Matters**: winit issues caused 40% of bugs → GPUI is better positioned
2. **IME is Hard**: Test all CJK languages, deduplicate events, handle empty preedit
3. **Signed Integers**: Use `isize` for grid coordinates that can go negative
4. **Security First**: Filter control sequences from pasted text
5. **Validate Early**: Check font metrics before Metal texture creation
6. **DPI Everywhere**: Apply scaling to rendering AND input coordinates
7. **Damage Tracking**: Line-based tracking yields performance without complexity
8. **Unicode Matters**: Use `unicode-width` crate, test CJK extensively
9. **State Machines**: Explicit alt screen tracking prevents reset bugs
10. **Reflow is Expected**: Users demand grid rewrap on resize
11. **Monitor Idle CPU**: Target <5 wakeups/sec for battery efficiency
12. **Trust alacritty_terminal**: Don't reimplement VT emulation

**Next Steps**:
1. Review Crux codebase against these lessons (audit checklist)
2. Add test cases for identified edge cases (especially IME, selection underflow)
3. Verify GPUI handles: DPI scaling, damage tracking, window transparency
4. Implement security filtering in clipboard paste path
5. Profile idle CPU usage and optimize event loop

---

## Sources (Resolved Bugs)

- [Issue #8079 - Korean IME Double-Space](https://github.com/alacritty/alacritty/issues/8079)
- [Issue #1640 - Selection Memory Leak](https://github.com/alacritty/alacritty/issues/1640)
- [PR #1658 - Fix Selection Rotation](https://github.com/alacritty/alacritty/pull/1658)
- [Issue #7915 - Font Metal Crash](https://github.com/alacritty/alacritty/issues/7915)
- [Issue #1802 - Retina Rendering](https://github.com/alacritty/alacritty/issues/1802)
- [Issue #3191 - Retina Mouse Position](https://github.com/alacritty/alacritty/issues/3191)
- [Issue #2389 - Empty Clipboard](https://github.com/alacritty/alacritty/issues/2389)
- [PR #2391 - Fix Empty Clipboard](https://github.com/alacritty/alacritty/pull/2391)
- [Issue #800 - Bracketed Paste Security](https://github.com/alacritty/alacritty/issues/800)
- [PR #1243 - Fix Bracketed Paste](https://github.com/alacritty/alacritty/pull/1243)
- [Issue #6313 - IME Preedit Clearing](https://github.com/alacritty/alacritty/issues/6313)
- [PR #6326 - Fix Preedit via Winit](https://github.com/alacritty/alacritty/pull/6326)
- [Issue #109 - Truecolor Support](https://github.com/alacritty/alacritty/issues/109)
- [Issue #265 - Unicode Width](https://github.com/alacritty/alacritty/issues/265)
- [Issue #2145 - Alt Screen Reset](https://github.com/alacritty/alacritty/issues/2145)
- [PR #2146 - Fix Alt Screen Reset](https://github.com/alacritty/alacritty/pull/2146)
- [PR #5773 - Damage Tracking](https://github.com/alacritty/alacritty/pull/5773)
- [Issue #8036 - Scrollback Wrapping](https://github.com/alacritty/alacritty/issues/8036)
- [PR #1049 - Ambiguous Width Config](https://github.com/alacritty/alacritty/pull/1049)
- [PR #7965 - macOS Transparency](https://github.com/alacritty/alacritty/pull/7965)
- [Issue #3775 - Idle CPU 100%](https://github.com/alacritty/alacritty/issues/3775)
- [Issue #8413 - High CPU Busy Loop](https://github.com/alacritty/alacritty/issues/8413)
- [thejh.net Bracketed Paste Exploit](http://thejh.net/misc/website-terminal-copy-paste)
