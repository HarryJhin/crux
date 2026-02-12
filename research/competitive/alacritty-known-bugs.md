---
title: Alacritty Known Issues and Lessons Learned
description: Comprehensive research on bugs, architectural issues, and gotchas in Alacritty that Crux should learn from and avoid
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
