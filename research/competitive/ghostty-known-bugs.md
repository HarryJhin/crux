---
title: "Ghostty Lessons Learned: Known Bugs and Issues to Avoid"
description: "Comprehensive analysis of Ghostty terminal emulator bugs (open and resolved), issues, and design decisions that inform Crux development strategy"
phase: 0
topics:
  - terminal emulation
  - ghostty
  - bugs
  - lessons learned
  - Korean IME
  - CJK
  - rendering
  - Metal
  - GPUI
related:
  - ime-clipboard.md
  - terminal-emulation.md
  - terminal-architecture.md
---

# Ghostty Lessons Learned: Known Bugs and Issues to Avoid

Research compiled: 2026-02-12

This document catalogs known bugs, issues, and design decisions from the Ghostty terminal emulator project that should inform Crux's development. Ghostty uses Zig + Metal rendering on macOS, similar to Crux's Rust + GPUI (which also uses Metal). Many lessons are directly applicable.

**Research Methodology**: GitHub issue tracker analysis, community discussions, release notes, and developer blog posts from Ghostty 1.0 through 1.2.1 (December 2024 - February 2026).

---

## Table of Contents

1. [CJK/IME Issues (Critical for Crux)](#cjkime-issues-critical-for-crux)
2. [TERM/Terminfo Compatibility](#termterminfo-compatibility)
3. [Metal/GPU Rendering Issues](#metalgpu-rendering-issues)
4. [Font Rendering (CoreText)](#font-rendering-coretext)
5. [Mouse Reporting](#mouse-reporting)
6. [Keyboard Handling](#keyboard-handling)
7. [Window Management (macOS)](#window-management-macos)
8. [Clipboard Issues](#clipboard-issues)
9. [Split Panes/Layout](#split-paneslayout)
10. [Scrollback Buffer](#scrollback-buffer)
11. [tmux Compatibility](#tmux-compatibility)
12. [Unicode/Emoji Rendering](#unicodeemoji-rendering)
13. [Performance](#performance)
14. [Configuration Gotchas](#configuration-gotchas)
15. [Color/Theme Rendering](#colortheme-rendering)
16. [Summary: Top 10 Lessons for Crux](#summary-top-10-lessons-for-crux)

---

## CJK/IME Issues (Critical for Crux)

### 1. Korean Hangul Character Disappearance

**Bug**: When typing Korean (Hangul) characters, completed characters display briefly then disappear. Only partial text appears.

**Example**: Typing "dkssud" to produce "한글" and pressing space results in only "한" appearing instead of the full text.

**Root Cause**: Character width calculation issue with full-width characters during composition state.

**Status**: Reported on Linux with fcitx5-hangul ([#6772](https://github.com/ghostty-org/ghostty/issues/6772))

**Lesson for Crux**:
- Test Hangul composition extensively with macOS native IME
- Verify character width calculations during composition state
- Ensure completed characters aren't being discarded or overwritten
- Test with both jamo (consonant-vowel) and completed syllable states

**Reference**: [linux: fcitx5-hangful "한글" input does not work](https://github.com/ghostty-org/ghostty/issues/6772)

---

### 2. Pre-edit Text Disappears with Modifier Keys

**Bug**: Pressing any modifier key (Shift, Ctrl, Option, Command) causes IME pre-edit text to disappear during composition.

**Behavior**:
- Terminal.app retains pre-edit text when modifier keys are pressed
- Ghostty caused pre-edit text to disappear
- If you press Enter after text disappears, input commits correctly (state was maintained)

**Affected IMEs**: macOS default IME, Google Japanese Input, macSKK

**Status**: **FIXED** in Ghostty 1.1.0 ([#4634](https://github.com/ghostty-org/ghostty/issues/4634))

**Root Cause**: Incorrect handling of NSTextInputClient events when modifier keys are pressed during composition.

**Lesson for Crux**:
- **CRITICAL**: Modifier keys during composition MUST NOT clear pre-edit text
- Test all modifier combinations during Hangul composition
- Verify with multiple IMEs (system default, Google Korean Input, etc.)
- Monitor NSTextInputClient lifecycle carefully
- Pre-edit state must persist through modifier key presses

**Reference**: [macOS: Pre-edit text disappears when pressing modifier keys during Japanese IME input](https://github.com/ghostty-org/ghostty/issues/4634)

---

### 3. IME Candidate Window Position

**Bug**: IME candidate window appears at bottom-left corner of screen instead of at cursor position.

**Impact**: Makes CJK input difficult as users can't see conversion candidates near typing location.

**Status**: Ongoing issue

**Root Cause**: NSTextInputClient not providing correct cursor position in screen coordinates.

**Lesson for Crux**:
- Implement `firstRectForCharacterRange:actualRange:` correctly
- Return cursor position in *screen* coordinates, not window coordinates
- Test with Korean, Japanese, and Chinese IMEs
- Verify candidate window follows cursor during scrolling
- Already documented in `research/platform/ime-clipboard.md` Section 2.4

**Reference**: [Feature Request: IME cursor position support for CJK input](https://github.com/anthropics/claude-code/issues/19207)

---

### 4. Japanese "ます" Ligature Bug

**Bug**: Japanese text "ます" (common word) displays as "〼" (obscure symbol).

**Root Cause**: Font ligature issue. Ghostty enables discretionary ligatures by default (`font-features = +dlig`), which caused this unexpected rendering.

**Fix**: Disable discretionary ligatures: `font-features = -dlig`

**Status**: **FIXED** - Documented in [#5372](https://github.com/ghostty-org/ghostty/issues/5372)

**Lesson for Crux**:
- Be cautious with ligature features when CJK fonts are involved
- Test common CJK text patterns, not just ASCII
- Provide user control over font features
- Consider different defaults for CJK vs. Latin fonts

**References**:
- [Font quirk: Broken "ます" ligature with BIZ UDGothic](https://github.com/ghostty-org/ghostty/issues/5372)
- [input the Japanese "ます" in ghostty, it is displayed as the symbol "〼"](https://github.com/ghostty-org/ghostty/discussions/5203)

---

### 5. CJK Font Size Issue

**Bug**: CJK characters appear much larger than Latin characters, looking "unwieldy and large."

**Root Cause**: CJK characters are constrained to two cells width-wise, and when using a wide Latin typeface, the optical size difference becomes exaggerated.

**Status**: Feature request for height-constrained CJK rendering ([#8709](https://github.com/ghostty-org/ghostty/issues/8709))

**Lesson for Crux**:
- Plan for independent font sizing of CJK vs. Latin
- Consider optical size balancing between scripts
- Test with mixed Latin/CJK text (common in Korean development)
- Provide `font-size-cjk` configuration option

**References**:
- [CJK characters should be height-constrained relative to Latin characters](https://github.com/ghostty-org/ghostty/issues/8709)
- [Chinese font size too big, revert 1.2 "Fallback Font Size Adjustment"](https://github.com/ghostty-org/ghostty/discussions/8651)

---

### 6. Japanese Keyboard Layout - Backslash Input

**Bug**: On macOS with Japanese (JIS) keyboard layout, pressing `Option + ¥` does not enter a backslash.

**Status**: Reported as bug ([#7147](https://github.com/ghostty-org/ghostty/discussions/7147))

**Lesson for Crux**:
- Test with JIS keyboard layout (common for Korean users too)
- Verify key mapping for special characters
- Ensure parity with Terminal.app for non-English keyboards

**Reference**: [Cannot input backslash \\ with Japanese keyboard layout on macOS](https://github.com/ghostty-org/ghostty/discussions/7147)

---

### CJK Summary for Crux

**High Priority**:
1. Pre-edit text persistence during modifier keys (CRITICAL)
2. Candidate window positioning
3. Hangul composition character width calculation
4. Font size balancing for mixed scripts

**Testing Checklist**:
- [ ] Hangul composition with all consonant/vowel combinations
- [ ] Modifier keys during composition (Shift, Ctrl, Option, Cmd)
- [ ] Candidate window follows cursor during scrolling
- [ ] Mixed Latin/Korean text rendering
- [ ] JIS keyboard layout special characters
- [ ] Third-party Korean IMEs (Google Korean Input, etc.)

---

## TERM/Terminfo Compatibility

### 7. The xterm- Prefix Decision

**Issue**: Ghostty chose `TERM=xterm-ghostty` instead of `TERM=ghostty`.

**Rationale**: Many programs do string matching on `$TERM` to determine capabilities (which is wrong, but common). The `xterm-` prefix provides better compatibility.

**Problems with Remote SSH**:
- Remote machines without Ghostty terminfo show: "Error opening terminal: xterm-ghostty"
- Requires copying terminfo to every remote server

**Solutions Ghostty Provides**:
1. One-liner to copy terminfo: `infocmp -x xterm-ghostty | ssh SERVER -- tic -x -`
2. Shell integration auto-sets `TERM=xterm-256color` for SSH
3. SSH config fallback (OpenSSH 8.7+): `SetEnv TERM=xterm-256color`

**Status**: Working as designed

**Lesson for Crux**:
- **MUST use `xterm-crux` not `crux`** (already documented in CLAUDE.md)
- Provide similar one-liner for terminfo copying
- Consider shell integration that auto-falls back to `xterm-256color` over SSH
- Document in setup guide prominently
- xterm-256color compatibility is sufficient for 99% of use cases

**References**:
- [Terminfo - Help](https://ghostty.org/docs/help/terminfo)
- [Error opening terminal: xterm-ghostty](https://github.com/ghostty-org/ghostty/discussions/3161)
- [Terminal Compatibility Issue with xterm-ghostty](https://github.com/ghostty-org/ghostty/discussions/4268)
- [Fix "Unknown Terminal xterm-ghostty" SSH Error](https://travis.media/blog/ghostty-ssh-unknown-terminal-error/)

---

## Metal/GPU Rendering Issues

### 8. Intel Mac GPU Artifacts

**Bug**: Red/white reverse "E" artifacts appear in fullscreen on Intel MacBook Pros.

**Root Cause**: Metal driver bug suspected. Undefined behavior on Intel Macs with discrete GPUs.

**Status**: **FIXED** in Ghostty 1.1.1 - proper discrete GPU detection and API usage

**Lesson for Crux**:
- Test on both Apple Silicon and Intel Macs
- Verify discrete GPU detection (use `MTLCreateSystemDefaultDevice()` correctly)
- Test fullscreen mode extensively
- Watch for undefined behavior in Metal shader code

**Reference**: [Fullscreen Ghostty has red/white reverse "E" artifacts on some Intel Mac laptops](https://github.com/ghostty-org/ghostty/discussions/3352)

---

### 9. Legacy GPU Compatibility

**Bug**: Ghostty crashes immediately on legacy GPUs (ATI Radeon HD 5xxx series).

**Root Cause**: Requires modern Metal support, not available on older hardware.

**Status**: Won't fix - Metal 2 is minimum requirement

**Lesson for Crux**:
- Document minimum macOS version clearly (macOS 13+ for Crux)
- Metal 2 requirement means macOS 10.13+, but Crux targets 13+ anyway
- No need to support legacy GPUs
- Fail gracefully with clear error message

**Reference**: [Default Ghostty Terminal Fails/Instantly Crashes on Legacy GPUs](https://github.com/basecamp/omarchy/issues/3581)

---

### 10. PNG Image Rendering Artifacts (Kitty Protocol)

**Bug**: PNG images displayed via Kitty protocol show diagonal line artifacts, precision errors in scaling.

**Status**: Ongoing issue ([#7350](https://github.com/ghostty-org/ghostty/discussions/7350))

**Lesson for Crux**:
- When implementing graphics protocols (Sixel, Kitty), use high-precision scaling
- Test diagonal lines and gradients
- Metal texture sampling may need quality hints

**Reference**: [Rendering bug on macos for png image using kitty protocol](https://github.com/ghostty-org/ghostty/discussions/7350)

---

### 11. Multi-Display Text Rendering Issues

**Bug**: Text rendering issues on multi-display setups.

**Status**: Reported ([#8295](https://github.com/ghostty-org/ghostty/discussions/8295))

**Lesson for Crux**:
- Test on external displays with different DPI
- Verify GPUI handles display changes correctly
- Monitor `NSScreen` notifications for display changes

**Reference**: [Terminal Text Rendering Issues on Multi-Display Setup](https://github.com/ghostty-org/ghostty/discussions/8295)

---

## Font Rendering (CoreText)

### 12. Font Style Regression (1.2.x)

**Bug**: Ghostty 1.2.x doesn't render `font-style` properties (italic) properly, regression from 1.1.x.

**Root Cause**: Major font rendering overhaul in 1.2.0 introduced bugs.

**Status**: Issues addressed in 1.2.1

**Lesson for Crux**:
- Font rendering changes are high-risk
- Test all font style combinations: regular, bold, italic, bold-italic
- Regression testing critical for font updates
- GPUI handles font rendering, verify updates don't break styles

**Reference**: [Upgrading from 1.1.x to 1.2.x changed (or broke?) the font-style rendering](https://github.com/ghostty-org/ghostty/discussions/9435)

---

### 13. RTL Language Crash with Trailing Spaces

**Bug**: Crash with certain RTL (right-to-left) languages and trailing spaces.

**Root Cause**: CoreText bug/edge case.

**Status**: **FIXED** in Ghostty 1.2.1

**Lesson for Crux**:
- Test RTL text rendering (Arabic, Hebrew)
- Edge case: RTL text with trailing whitespace
- CoreText has quirks - defensive programming needed

**Reference**: [1.2.1 Release Notes](https://ghostty.org/docs/install/release-notes/1-2-1)

---

### 14. Nerd Fonts Glyph Width Issues

**Bug**: In Ghostty 1.2.0, Nerd Fonts glyphs changed to always take 2 cells, causing misalignment.

**Status**: **FIXED** in 1.2.1 font rendering improvements

**Lesson for Crux**:
- Test Nerd Fonts explicitly (common in dev environments)
- Verify private use area (PUA) character widths
- Some glyphs should be 1-cell, some 2-cell
- Unicode width database may not cover PUA correctly

**Reference**: [Nerd fonts glyph width in 1.2.0](https://github.com/ghostty-org/ghostty/discussions/8822)

---

### 15. Font Missing Variants Causes Total Failure

**Bug**: When a font lacks italic variants, Ghostty fails to render *all* text, including non-italicized text.

**Example**: Geist Mono font without italic variant caused complete rendering failure.

**Status**: Reported ([#8367](https://github.com/ghostty-org/ghostty/discussions/8367))

**Lesson for Crux**:
- Gracefully fall back when font variant missing
- Don't fail completely - synthesize or use regular weight
- Warn user about missing font variants
- Test with fonts that lack italic/bold

**Reference**: [Font not rendering at all](https://github.com/ghostty-org/ghostty/discussions/8367)

---

### 16. Custom Border Character Rendering

**Bug**: Wrong font rendering on custom border characters.

**Status**: Reported ([#3415](https://github.com/ghostty-org/ghostty/issues/3415))

**Lesson for Crux**:
- Box-drawing characters (U+2500-U+257F) need special handling
- Consider using vector rendering for box-drawing vs. font glyphs
- Test with powerline/custom prompt characters

**Reference**: [Wrong font rendering on custom border characters](https://github.com/ghostty-org/ghostty/issues/3415)

---

## Mouse Reporting

### 17. Mouse Coordinates Outside Window

**Bug**: When mouse moves outside window (above/left), escape sequences contain negative coordinates.

**Behavior**: XTerm, Ghostty, Foot, Kitty all report negative values (de facto standard).

**Status**: Expected behavior

**Lesson for Crux**:
- Allow negative coordinates in mouse reporting
- Match XTerm behavior for compatibility
- Document this behavior

**Reference**: [mouse tracking escape sequences reporting negative numbers when outside of window](https://github.com/ghostty-org/ghostty/discussions/9647)

---

### 18. Right Mouse Button Stuck State

**Bug**: Right mouse button can get stuck in mouse-down state, especially with modifier keys.

**Reproduction**: Shift + right click to show macOS context menu.

**Status**: Reported as bug

**Lesson for Crux**:
- Track mouse button state carefully
- Handle platform context menus separately from terminal mouse events
- Test modifier + mouse button combinations
- Ensure button-up events always fire

**Reference**: [Mouse reporting issues bugs](https://github.com/ghostty-org/ghostty/issues/8430)

---

### 19. tmux Mouse Mode Unreliable

**Bug**: tmux mouse mode doesn't reliably select panes in Ghostty.

**Workaround**: Set `TERM=xterm-256color` in tmux.

**Status**: Working as designed (tmux compatibility issue)

**Lesson for Crux**:
- tmux has special mouse handling quirks
- Test mouse selection in tmux explicitly
- Document tmux compatibility in help

**Reference**: [tmux mouse mode doesnt reliably select panes](https://github.com/ghostty-org/ghostty/discussions/5362)

---

### 20. Hide Mouse While Typing Bug

**Bug**: If typing hides cursor via macOS feature, `Cmd+Tab` out and back makes mouse permanently hidden until restart.

**Platform**: macOS only

**Status**: Reported ([#2525](https://github.com/ghostty-org/ghostty/issues/2525))

**Lesson for Crux**:
- Handle macOS cursor visibility states carefully
- Reset cursor visibility on window focus changes
- Test `Cmd+Tab` switching during active typing

**Reference**: [macOS: hide mouse while typing bug](https://github.com/ghostty-org/ghostty/issues/2525)

---

## Keyboard Handling

### 21. Kitty Keyboard Protocol - Ctrl+[ Encoding Bug

**Bug**: `Ctrl+[` encoded as `^[[91;5u` instead of `^[` per Kitty Keyboard Protocol spec.

**Status**: Reported ([#5071](https://github.com/ghostty-org/ghostty/discussions/5071))

**Lesson for Crux**:
- Kitty Keyboard Protocol is complex with many edge cases
- Test all Ctrl combinations explicitly
- `Ctrl+[` is ESC historically - maintain compatibility
- Document deviations from spec if necessary

**Reference**: [Bug: `Ctrl+[` is encoded `^[[91;5u`, and not `^[` as specified](https://github.com/ghostty-org/ghostty/discussions/5071)

---

### 22. Compose Key with Kitty Protocol

**Bug**: No text reported for input from Compose key when Kitty Keyboard Protocol is enabled.

**Status**: Reported ([#10049](https://github.com/ghostty-org/ghostty/issues/10049))

**Lesson for Crux**:
- Not a macOS concern (no Compose key)
- If implementing Kitty protocol, be aware of Linux edge case

**Reference**: [Kitty keyboard protocol: No text reported for input from Compose key](https://github.com/ghostty-org/ghostty/issues/10049)

---

## Window Management (macOS)

### 23. Quick Terminal Visibility Toggle Bug

**Bug**: When using `toggle_visibility` to hide window, `toggle_quick_terminal` makes hidden window visible again.

**Expected**: Hidden window stays hidden.

**Status**: Reported ([#8414](https://github.com/ghostty-org/ghostty/issues/8414))

**Lesson for Crux**:
- Track window visibility state separately from focus state
- Test window state interactions thoroughly
- Not critical for Crux (no Quick Terminal feature planned Phase 1)

**Reference**: [macOS: toggle_quick_terminal makes hidden window visible again](https://github.com/ghostty-org/ghostty/issues/8414)

---

### 24. New Windows Always Open on Primary Monitor

**Bug**: New windows always open on primary monitor, even when existing window is on secondary monitor. Opening new tab also moves window to primary monitor.

**Status**: Reported ([#9310](https://github.com/ghostty-org/ghostty/issues/9310))

**Lesson for Crux**:
- Respect current monitor when opening new windows/tabs
- Use NSScreen to track which monitor has focus
- GPUI window management - verify multi-monitor behavior
- Common complaint, affects user experience significantly

**Reference**: [macOS: new windows/tabs always open on primary monitor, moving existing windows](https://github.com/ghostty-org/ghostty/issues/9310)

---

### 25. Tiling Window Manager Compatibility (Yabai/Aerospace)

**Bug**: Ghostty tabs render as separate windows in tiling WMs like Yabai/Aerospace.

**Workaround**: Make Ghostty floating, then unfloat after launch.

**Status**: Known limitation

**Lesson for Crux**:
- Native macOS tabs may not work well with tiling WMs
- Document this limitation
- Consider alternative approach for splits/panes that works better with tiling WMs
- Phase 2 concern (tabs/splits not in Phase 1)

**Reference**: [macOS Tiling Window Managers - Help](https://ghostty.org/docs/help/macos-tiling-wms)

---

## Clipboard Issues

### 26. Clipboard Format Limitations

**Bug**: Ghostty on macOS only populates text/plain - formatting information is lost.

**Status**: Supports text/plain and text/html. RTF not planned.

**Lesson for Crux**:
- Plan for rich text clipboard from the start (already in Phase 3)
- NSPasteboard supports multiple formats simultaneously
- Copy both plain text and RTF/HTML for rich paste
- Already documented in `research/platform/ime-clipboard.md` Section 3

**Reference**: [Rich text (RTF) copy on macOS](https://github.com/ghostty-org/ghostty/discussions/9798)

---

### 27. Copy-on-Select Clipboard Confusion

**Bug**: `copy-on-select = clipboard` breaks middle-click paste when copying from other apps.

**Root Cause**: Selection clipboard vs. system clipboard confusion.

**Status**: Working as designed (X11-style vs. macOS-style clipboard)

**Lesson for Crux**:
- macOS doesn't have selection clipboard like X11
- Middle-click paste is not native to macOS
- Focus on macOS-native clipboard behavior
- Don't try to emulate X11 selection buffer on macOS

**Reference**: [Clipboard bug in MacOS and better clipboard management in general](https://github.com/ghostty-org/ghostty/discussions/5600)

---

### 28. Cmd+V vs. Cmd+Shift+V

**Issue**: Users confused that `Cmd+V` and `Cmd+Shift+V` paste different content.

**Explanation**: They paste from different clipboards (selection vs. primary). This matches Terminal.app.

**Status**: Working as designed

**Lesson for Crux**:
- Follow Terminal.app conventions for macOS users
- Document clipboard shortcuts clearly
- `Cmd+V` = system clipboard (standard)
- Consider whether `Cmd+Shift+V` is needed on macOS

**Reference**: [Command+Shift+V doesn't paste what is expected](https://github.com/ghostty-org/ghostty/discussions/9447)

---

### 29. Bracketed Paste Mode CR LF Handling

**Bug**: Multi-line paste in bracketed paste mode renders as single line in some apps due to CR LF handling differences vs. Terminal.app.

**Status**: Reported ([#9592](https://github.com/ghostty-org/ghostty/discussions/9592))

**Lesson for Crux**:
- Bracketed paste line ending normalization is critical
- Test with multi-line pastes
- Match Terminal.app behavior for macOS consistency
- Test with various shells (bash, zsh, fish)

**Reference**: [macOS paste behavior inconsistent with Terminal.app](https://github.com/ghostty-org/ghostty/discussions/9592)

---

## Split Panes/Layout

### 30. Split Panes Get 1px Out of Sync

**Bug**: Aggressive resizing causes split panes to get 1px out of sync in height.

**Workaround**: Hide/show quick terminal fixes alignment.

**Platform**: macOS

**Status**: Reported ([#2944](https://github.com/ghostty-org/ghostty/issues/2944))

**Lesson for Crux**:
- Integer rounding during resize can accumulate errors
- Redistribute remaining pixels across splits
- Test rapid window resizing
- GPUI layout system - verify split calculations

**Reference**: [Quick terminal splits get 1px out of sync with aggressive resizing](https://github.com/ghostty-org/ghostty/issues/2944)

---

### 31. goto_split Navigation Depends on Creation Order

**Bug**: `goto_split` command works differently depending on order splits were created.

**Status**: Reported ([#3408](https://github.com/ghostty-org/ghostty/issues/3408))

**Lesson for Crux**:
- Split navigation should use spatial coordinates, not creation order
- Consider vim-style hjkl navigation (up/down/left/right)
- Phase 2 concern

**Reference**: [`goto_split` works differently depending on the order in which splits are created](https://github.com/ghostty-org/ghostty/issues/3408)

---

### 32. No Default Split Layout Configuration

**Issue**: Cannot define default split layout; must recreate manually every time.

**Status**: Feature request ([#2480](https://github.com/ghostty-org/ghostty/discussions/2480))

**Lesson for Crux**:
- Plan for saved layouts in Phase 2
- Consider session restoration
- Allow configuration file to specify initial layout

**Reference**: [Ability to define split layouts](https://github.com/ghostty-org/ghostty/discussions/2480)

---

## Scrollback Buffer

### 33. Screen Clear Drops Scrollback

**Bug**: When screen is cleared, output is dropped from scrollback buffer.

**Status**: Expected behavior (matches kitty)

**Lesson for Crux**:
- Document this behavior clearly
- Some users expect scrollback to persist after clear
- Consider configuration option to preserve scrollback on clear

**Reference**: [implement "scroll and clear" sequence](https://github.com/ghostty-org/ghostty/issues/905)

---

### 34. Vim Status Line Leaks into Scrollback

**Bug**: vim status line ("-- INSERT --") appears in scrollback buffer instead of staying at bottom.

**Root Cause**: Not recognizing vim's alternate screen buffer usage correctly.

**Status**: Reported ([#7066](https://github.com/ghostty-org/ghostty/issues/7066))

**Lesson for Crux**:
- Handle alternate screen buffer correctly
- Status line content should not enter scrollback
- Test with vim, tmux, other alternate screen apps

**Reference**: [vim status line leaks into scrollback](https://github.com/ghostty-org/ghostty/issues/7066)

---

### 35. Scrollback Memory Inefficiency

**Bug**: Scrollback preallocates memory for every cell, even blank ones. Wide terminals waste memory.

**Example**: Terminal width matters more than actual content length for memory usage.

**Status**: Acknowledged design limitation ([#9821](https://github.com/ghostty-org/ghostty/discussions/9821))

**Lesson for Crux**:
- Consider sparse storage for scrollback
- Don't preallocate full width for every line
- alacritty_terminal grid - verify memory efficiency
- Monitor memory usage with large scrollback

**Reference**: [scrollback buffer is extremely memory-inefficient?](https://github.com/ghostty-org/ghostty/discussions/9821)

---

### 36. No Unlimited Scrollback

**Issue**: Cannot set unlimited scrollback; capped at u32::MAX bytes.

**Status**: Planned future feature

**Lesson for Crux**:
- Consider unlimited scrollback option
- Implement memory pressure handling
- Allow user to choose reasonable limit

**Reference**: [Scrollback buffer is limited to `u32::MAX` bytes](https://github.com/ghostty-org/ghostty/discussions/3884)

---

## tmux Compatibility

### 37. Terminal Type Over SSH

**Bug**: SSHing into remote + starting tmux: "missing or unsuitable terminal: xterm-ghostty"

**Root Cause**: Remote server lacks terminfo database entry.

**Workaround**: Set `TERM=xterm-256color` for tmux.

**Status**: Working as designed

**Lesson for Crux**:
- Same as TERM issue (#7)
- Document tmux-specific configuration
- Shell integration can auto-set TERM over SSH

**Reference**: [Getting Ghostty to work with Tmux-in-SSH](https://abacusnoir.com/2025/03/07/getting-ghostty-to-work-with-tmux-in-ssh/)

---

### 38. tmux Doesn't Support Ghostty Features

**Issue**: When running tmux inside Ghostty, Ghostty-specific features become unavailable.

**Philosophy**: Some users suggest replacing tmux with native terminal multiplexing.

**Status**: Design limitation

**Lesson for Crux**:
- Plan Phase 5: tmux compatibility testing
- Document feature limitations when running under tmux
- Consider native multiplexing as tmux alternative (Phase 2 splits)

**Reference**: [Replacing tmux with Ghostty](https://sterba.dev/posts/replacing-tmux/)

---

### 39. Keyboard Navigation with tmux Plugins

**Bug**: Pane navigation with tmux plugins (vim-tmux-navigator) only works partially.

**Fix**: Requires specific tmux configuration:
```
set -s extended-keys on
set -as terminal-features 'xterm-ghostty:extkeys'
```

**Status**: Workaround exists

**Lesson for Crux**:
- Document tmux configuration requirements
- Test with popular tmux plugins
- Provide sample tmux.conf snippet

**Reference**: [Tmux & Ghostty](https://mansoorbarri.com/tmux-ghostty/)

---

## Unicode/Emoji Rendering

### 40. Wide Character Width Detection

**Issue**: Ghostty uses Unicode standard for grapheme width, which can cause cursor-desync with legacy programs using `wcswidth()`.

**Configuration**: `grapheme-width = unicode` (default) vs. `legacy`

**Status**: Working as designed

**Lesson for Crux**:
- Provide configuration option for width calculation method
- Default to Unicode standard
- Test with programs expecting legacy width
- Document this setting

**Reference**: [Option Reference - Configuration](https://ghostty.org/docs/config/reference)

---

### 41. Double-Width Unicode Overflow

**Bug**: Certain double-width Unicode characters overflow single cells when only one cell available.

**Expected**: Should shrink to fit when insufficient space.

**Status**: Reported ([#5588](https://github.com/ghostty-org/ghostty/discussions/5588))

**Lesson for Crux**:
- Handle double-width character clipping gracefully
- Respect cell boundaries
- Test with wide characters at line end

**Reference**: [Certain double-width unicode characters overflow single cells](https://github.com/ghostty-org/ghostty/discussions/5588)

---

### 42. Emoji Memory Growth

**Bug**: Memory grows unboundedly with heavy emoji/hyperlink output.

**Status**: Reported ([#10244](https://github.com/ghostty-org/ghostty/discussions/10244))

**Lesson for Crux**:
- Monitor memory usage with emoji-heavy output
- Implement memory pressure handling
- Test with emoji spam (cat emoji-heavy file)

**Reference**: [Memory grows unboundedly on non-ASCII terminal output (emoji, hyperlinks)](https://github.com/ghostty-org/ghostty/discussions/10244)

---

### 43. Some Unicode Symbols Render Small

**Bug**: Some Unicode symbols render unexpectedly small.

**Root Cause**: PUA (Private Use Area) characters and Nerd Fonts sizing.

**Status**: Reported ([#1903](https://github.com/ghostty-org/ghostty/issues/1903))

**Lesson for Crux**:
- Test Nerd Fonts symbol sizing
- Verify icon fonts render at correct size
- May need manual size adjustments for PUA

**Reference**: [Some unicode symbols are rendered unexpectedly small](https://github.com/ghostty-org/ghostty/issues/1903)

---

## Performance

### 44. macOS Tahoe Scrolling Performance Degradation

**Bug**: After upgrading to macOS Tahoe Developer Beta, extreme scrolling lag that got worse over time.

**Root Cause**: New Tahoe autofill feature interacting poorly with custom NSTextInputClients.

**Status**: **FIXED** after identification

**Lesson for Crux**:
- Monitor macOS beta releases for NSTextInputClient changes
- Test scrolling performance on new macOS versions
- Opt out of unwanted autofill features
- Performance can degrade over session time - test long-running instances

**Reference**: [Progressively worse scrolling performance over time on macOS Tahoe Developer Beta](https://github.com/ghostty-org/ghostty/discussions/8616)

---

### 45. Terminal Renders Too Fast (Flashing)

**"Bug"**: Ghostty renders while applications are still updating, causing flashing.

**Root Cause**: Ghostty is *too fast* compared to applications' screen update rate.

**Status**: Feature, not bug

**Lesson for Crux**:
- Frame synchronization with terminal apps is hard
- May need to throttle rendering for smoother appearance
- Consider vsync or fixed refresh rate option

**Reference**: [Terminal Flashes with persistent UI tools](https://github.com/ghostty-org/ghostty/discussions/8162)

---

### 46. Discrete GPU Undefined Behavior

**Bug**: Undefined behavior triggering on macOS discrete GPUs (Intel Macs).

**Impact**: Rendering artifacts, strange visual behaviors.

**Status**: **FIXED** in 1.1.1 - proper discrete GPU detection

**Lesson for Crux**:
- Already covered in #8 (Intel Mac GPU artifacts)
- Test on both integrated and discrete GPUs
- Verify Metal device selection

**Reference**: [1.1.1 Release Notes](https://ghostty.org/docs/install/release-notes/1-1-1)

---

## Configuration Gotchas

### 47. Font Style Disabling Gotcha

**Gotcha**: Disabling bold or italic does NOT disable bold-italic.

**Example**:
```
font-style-bold = false
font-style-italic = false
font-style-bold-italic = true  # STILL ENABLED!
```

**Lesson for Crux**:
- Document this clearly
- Consider cascading: disabling bold should disable bold-italic too
- Or require explicit configuration for all combinations

**Reference**: [Configuration](https://ghostty.org/docs/config)

---

### 48. Path Configuration Gotchas

**Gotcha**: Don't use device paths like `/dev/stdin` or `/dev/urandom` in config - they block startup indefinitely.

**Gotcha**: Config files limited to 10MB to prevent memory exhaustion.

**Lesson for Crux**:
- Validate configuration file paths
- Check for device files and reject them
- Set reasonable file size limits
- Fail gracefully with clear error

**Reference**: [Option Reference - Configuration](https://ghostty.org/docs/config/reference)

---

### 49. Theme Name Changes (1.2.0)

**Gotcha**: Theme names changed from kebab-case to Title Case with spaces in version 1.2.0.

**Example**: `dracula` → `"Dracula"` (needs quotes now)

**Lesson for Crux**:
- Keep theme naming stable across versions
- Support both formats for backward compatibility
- Provide `list-themes` command

**Reference**: [1.2.0 Release Notes](https://ghostty.org/docs/install/release-notes/1-2-0)

---

### 50. Keyboard Binding Conflicts

**Gotcha**: Binding `ctrl+l` conflicts with clear screen in SSH/byobu sessions.

**Lesson for Crux**:
- Document common keybinding conflicts
- Warn about overriding standard terminal shortcuts
- Provide list of "reserved" keybindings

**Reference**: [Ghostty Config: Power Up Your Terminal](https://centlinux.com/ghostty-config/)

---

## Color/Theme Rendering

### 51. Color Space Display-P3 Washed Out Colors

**Bug**: Colors appear "washed out" on some systems when using `window-colorspace = display-p3`.

**Status**: Configuration issue

**Lesson for Crux**:
- Provide color space configuration option
- Default to sRGB for compatibility
- Allow P3 for wide-gamut displays
- Test on both sRGB and P3 displays

**Reference**: [Ghostty terminal colors](https://github.com/ghostty-org/ghostty/discussions/5961)

---

### 52. Light/Dark Mode Theme Switching Bugs

**Bug**: macOS titlebar tabs style not updated when switching themes (light/dark mode).

**Status**: Known bug

**Lesson for Crux**:
- Monitor NSAppearance changes
- Update all UI elements on theme change
- Test automatic light/dark mode switching
- GPUI theme handling - verify completeness

**Reference**: [Color Theme - Features](https://ghostty.org/docs/features/theme)

---

### 53. Third-Party Theme Palette Changes

**Issue**: iTerm2-Color-Schemes update "wrecked" color palette mid-release.

**Lesson for Crux**:
- Pin theme definitions or vendor them
- Don't auto-update themes without review
- Provide stable theme versioning

**Reference**: [Theme color palette changed in 1.2.1](https://github.com/ghostty-org/ghostty/discussions/9063)

---

## Summary: Top 10 Lessons for Crux

### 1. **IME Pre-edit MUST Persist Through Modifier Keys** (CRITICAL)

**Issue**: Pre-edit text disappearing when pressing modifier keys during Korean/Japanese composition.

**Action**:
- Test exhaustively: Shift, Ctrl, Option, Cmd during Hangul composition
- Verify NSTextInputClient lifecycle
- Never clear pre-edit on modifier-only events

**Priority**: P0 - Blocks Korean input use case

---

### 2. **Use xterm-crux TERM Name with Prefix**

**Issue**: Many programs do string matching on `$TERM` variable.

**Action**:
- Use `xterm-crux` not `crux`
- Provide SSH auto-fallback to `xterm-256color`
- One-liner terminfo copy command

**Priority**: P1 - Critical for SSH workflows

**Status**: Already documented in CLAUDE.md ✓

---

### 3. **Test on Intel Macs AND Apple Silicon**

**Issue**: Metal discrete GPU undefined behavior on Intel Macs.

**Action**:
- Test on both architectures
- Verify Metal device selection
- Test fullscreen mode on Intel

**Priority**: P1 - Affects half of Mac user base

---

### 4. **IME Candidate Window Positioning**

**Issue**: Candidate window appears at screen corner instead of cursor.

**Action**:
- Implement `firstRectForCharacterRange:actualRange:` correctly
- Return screen coordinates, not window coordinates
- Test with scrolling

**Priority**: P0 - Critical for CJK input UX

**Status**: Documented in `research/platform/ime-clipboard.md` ✓

---

### 5. **CJK Font Size Balancing**

**Issue**: CJK characters appear much larger than Latin in mixed text.

**Action**:
- Plan independent font sizing for CJK
- Test mixed Latin/Korean text
- Provide `font-size-cjk` option

**Priority**: P2 - Quality of life for Korean developers

---

### 6. **Split Pane Resize Integer Rounding**

**Issue**: 1px misalignment accumulates during aggressive resizing.

**Action**:
- Redistribute remaining pixels across splits
- Test rapid window resizing
- Verify GPUI layout calculations

**Priority**: P2 - Phase 2 concern

---

### 7. **Scrollback Memory Efficiency**

**Issue**: Preallocating full width wastes memory.

**Action**:
- Consider sparse storage for blank cells
- Monitor alacritty_terminal grid memory
- Test with large scrollback + wide terminals

**Priority**: P2 - Performance optimization

---

### 8. **Rich Text Clipboard from Day 1**

**Issue**: Text-only clipboard loses formatting.

**Action**:
- Copy both plain text and RTF to NSPasteboard
- Support styled text copy

**Priority**: P2 - Phase 3 feature

**Status**: Documented in `research/platform/ime-clipboard.md` ✓

---

### 9. **Bracketed Paste Line Ending Normalization**

**Issue**: Multi-line paste renders incorrectly due to CR LF differences.

**Action**:
- Match Terminal.app behavior
- Test multi-line paste in bash, zsh, fish
- Normalize line endings correctly

**Priority**: P1 - Common operation

---

### 10. **Font Rendering Variant Fallback**

**Issue**: Missing italic variant causes complete rendering failure.

**Action**:
- Gracefully fall back to regular when variant missing
- Warn user about missing variants
- Don't fail completely

**Priority**: P1 - Robustness

---

## Testing Checklist for Crux

### Phase 1 (Basic Terminal MVP)

**CJK/IME** (P0):
- [ ] Hangul composition with all consonant/vowel combinations
- [ ] Pre-edit text persists through Shift, Ctrl, Option, Cmd
- [ ] Candidate window follows cursor position
- [ ] Mixed Latin/Korean text renders correctly
- [ ] Test with macOS native Korean IME
- [ ] Test with Google Korean Input (if available)

**Rendering** (P1):
- [ ] Test on Intel Mac with discrete GPU
- [ ] Test on Apple Silicon Mac
- [ ] Fullscreen mode on both architectures
- [ ] Multi-display setup (external monitor)
- [ ] Font styles: regular, bold, italic, bold-italic
- [ ] Fonts missing variants (fallback handling)
- [ ] Nerd Fonts glyph sizing
- [ ] Box-drawing characters
- [ ] Double-width CJK characters
- [ ] Emoji rendering

**TERM/Terminfo** (P1):
- [ ] `$TERM` set to `xterm-crux`
- [ ] Local shell works
- [ ] SSH to remote without terminfo (should fail gracefully)
- [ ] SSH with terminfo copied (should work)
- [ ] Test with tmux locally
- [ ] Test with vim alternate screen

**Keyboard** (P1):
- [ ] All Ctrl combinations
- [ ] Modifier + mouse combinations
- [ ] Cmd+Tab focus switching during typing

**Clipboard** (P1):
- [ ] Cmd+C copy
- [ ] Cmd+V paste
- [ ] Multi-line paste (bracketed paste mode)
- [ ] Test in bash, zsh, fish

**Scrollback** (P2):
- [ ] Scrollback limit configuration
- [ ] Clear screen behavior
- [ ] vim status line doesn't leak to scrollback
- [ ] Memory usage with large scrollback

### Phase 2 (Tabs, Splits, IPC)

**Splits** (P2):
- [ ] Rapid window resize (1px alignment)
- [ ] Split navigation (spatial, not creation order)
- [ ] Background image across splits

**Window Management** (P2):
- [ ] New window opens on correct monitor
- [ ] Tiling WM compatibility (if applicable)

### Phase 3 (Korean IME, Rich Clipboard)

**IME Deep Testing** (P0):
- [ ] All issues from Phase 1, re-verified
- [ ] JIS keyboard layout special characters
- [ ] Modifier keys during composition (exhaustive)
- [ ] Long composition sessions (stability)

**Clipboard** (P2):
- [ ] Rich text (RTF) copy
- [ ] HTML copy
- [ ] Plain text always available
- [ ] Paste styled text into other apps

### Phase 5 (tmux Compatibility)

**tmux** (P2):
- [ ] Start tmux locally
- [ ] Start tmux over SSH
- [ ] Mouse selection in tmux
- [ ] vim-tmux-navigator plugin
- [ ] Provide sample tmux.conf

---

## Conclusion

Ghostty's development reveals critical lessons for terminal emulator projects, especially for CJK/IME support and macOS Metal rendering. The top priorities for Crux are:

1. **IME correctness** - Pre-edit persistence and candidate window positioning
2. **Cross-architecture testing** - Intel and Apple Silicon
3. **Terminal compatibility** - TERM naming and terminfo distribution
4. **Graceful degradation** - Handle missing fonts, remote servers, etc.

Many of these lessons align with Crux's existing research documents, validating the architecture decisions already documented in `research/platform/ime-clipboard.md`, `research/core/terminfo.md`, and the CLAUDE.md guidelines.

The comprehensive testing checklist above should be integrated into Crux's QA process for each phase.

---

## Sources

- [GitHub - ghostty-org/ghostty](https://github.com/ghostty-org/ghostty)
- [Ghostty Official Documentation](https://ghostty.org/docs)
- [Ghostty Release Notes 1.1.0](https://ghostty.org/docs/install/release-notes/1-1-0)
- [Ghostty Release Notes 1.1.1](https://ghostty.org/docs/install/release-notes/1-1-1)
- [Ghostty Release Notes 1.2.0](https://ghostty.org/docs/install/release-notes/1-2-0)
- [Ghostty Release Notes 1.2.1](https://ghostty.org/docs/install/release-notes/1-2-1)
- [Ghostty GitHub Issues](https://github.com/ghostty-org/ghostty/issues)
- [Ghostty GitHub Discussions](https://github.com/ghostty-org/ghostty/discussions)
- [linux: fcitx5-hangful "한글" input does not work](https://github.com/ghostty-org/ghostty/issues/6772)
- [macOS: Pre-edit text disappears when pressing modifier keys during Japanese IME input](https://github.com/ghostty-org/ghostty/issues/4634)
- [Feature Request: IME cursor position support for CJK input](https://github.com/anthropics/claude-code/issues/19207)
- [Font quirk: Broken "ます" ligature with BIZ UDGothic](https://github.com/ghostty-org/ghostty/issues/5372)
- [CJK characters should be height-constrained relative to Latin characters](https://github.com/ghostty-org/ghostty/issues/8709)
- [Cannot input backslash \\ with Japanese keyboard layout on macOS](https://github.com/ghostty-org/ghostty/discussions/7147)
- [Terminfo - Help](https://ghostty.org/docs/help/terminfo)
- [Error opening terminal: xterm-ghostty](https://github.com/ghostty-org/ghostty/discussions/3161)
- [Fullscreen Ghostty has red/white reverse "E" artifacts on some Intel Mac laptops](https://github.com/ghostty-org/ghostty/discussions/3352)
- [Rendering bug on macos for png image using kitty protocol](https://github.com/ghostty-org/ghostty/discussions/7350)
- [Terminal Text Rendering Issues on Multi-Display Setup](https://github.com/ghostty-org/ghostty/discussions/8295)
- [Upgrading from 1.1.x to 1.2.x changed (or broke?) the font-style rendering](https://github.com/ghostty-org/ghostty/discussions/9435)
- [Nerd fonts glyph width in 1.2.0](https://github.com/ghostty-org/ghostty/discussions/8822)
- [Font not rendering at all](https://github.com/ghostty-org/ghostty/discussions/8367)
- [Wrong font rendering on custom border characters](https://github.com/ghostty-org/ghostty/issues/3415)
- [mouse tracking escape sequences reporting negative numbers when outside of window](https://github.com/ghostty-org/ghostty/discussions/9647)
- [Add `mouse-reporting` configuration to disable all mouse reports](https://github.com/ghostty-org/ghostty/issues/8430)
- [tmux mouse mode doesnt reliably select panes](https://github.com/ghostty-org/ghostty/discussions/5362)
- [macOS: hide mouse while typing bug](https://github.com/ghostty-org/ghostty/issues/2525)
- [Bug: `Ctrl+[` is encoded `^[[91;5u`, and not `^[` as specified](https://github.com/ghostty-org/ghostty/discussions/5071)
- [Kitty keyboard protocol: No text reported for input from Compose key](https://github.com/ghostty-org/ghostty/issues/10049)
- [macOS: toggle_quick_terminal makes hidden window visible again](https://github.com/ghostty-org/ghostty/issues/8414)
- [macOS: new windows/tabs always open on primary monitor, moving existing windows](https://github.com/ghostty-org/ghostty/issues/9310)
- [macOS Tiling Window Managers - Help](https://ghostty.org/docs/help/macos-tiling-wms)
- [Rich text (RTF) copy on macOS](https://github.com/ghostty-org/ghostty/discussions/9798)
- [Clipboard bug in MacOS and better clipboard management in general](https://github.com/ghostty-org/ghostty/discussions/5600)
- [Command+Shift+V doesn't paste what is expected](https://github.com/ghostty-org/ghostty/discussions/9447)
- [macOS paste behavior inconsistent with Terminal.app](https://github.com/ghostty-org/ghostty/discussions/9592)
- [Quick terminal splits get 1px out of sync with aggressive resizing](https://github.com/ghostty-org/ghostty/issues/2944)
- [`goto_split` works differently depending on the order in which splits are created](https://github.com/ghostty-org/ghostty/issues/3408)
- [Ability to define split layouts](https://github.com/ghostty-org/ghostty/discussions/2480)
- [implement "scroll and clear" sequence](https://github.com/ghostty-org/ghostty/issues/905)
- [vim status line leaks into scrollback](https://github.com/ghostty-org/ghostty/issues/7066)
- [scrollback buffer is extremely memory-inefficient?](https://github.com/ghostty-org/ghostty/discussions/9821)
- [Scrollback buffer is limited to `u32::MAX` bytes](https://github.com/ghostty-org/ghostty/discussions/3884)
- [Getting Ghostty to work with Tmux-in-SSH](https://abacusnoir.com/2025/03/07/getting-ghostty-to-work-with-tmux-in-ssh/)
- [Replacing tmux with Ghostty](https://sterba.dev/posts/replacing-tmux/)
- [Tmux & Ghostty](https://mansoorbarri.com/tmux-ghostty/)
- [Certain double-width unicode characters overflow single cells](https://github.com/ghostty-org/ghostty/discussions/5588)
- [Memory grows unboundedly on non-ASCII terminal output (emoji, hyperlinks)](https://github.com/ghostty-org/ghostty/discussions/10244)
- [Some unicode symbols are rendered unexpectedly small](https://github.com/ghostty-org/ghostty/issues/1903)
- [Progressively worse scrolling performance over time on macOS Tahoe Developer Beta](https://github.com/ghostty-org/ghostty/discussions/8616)
- [Terminal Flashes with persistent UI tools](https://github.com/ghostty-org/ghostty/discussions/8162)
- [Configuration](https://ghostty.org/docs/config)
- [Option Reference - Configuration](https://ghostty.org/docs/config/reference)
- [Ghostty Config: Power Up Your Terminal](https://centlinux.com/ghostty-config/)
- [Ghostty terminal colors](https://github.com/ghostty-org/ghostty/discussions/5961)
- [Color Theme - Features](https://ghostty.org/docs/features/theme)
- [Theme color palette changed in 1.2.1](https://github.com/ghostty-org/ghostty/discussions/9063)

---

## Resolved Issues — Root Cause & Fix Analysis

> 아래는 Ghostty에서 **해결된** 주요 버그들의 근본 원인과 수정 방법 분석이다.
> 버전별 수정 이력과 코드 레벨 교훈을 포함한다.

# Ghostty Closed Bug Analysis

Analysis of **closed/resolved** bugs in Ghostty terminal emulator (Zig + Metal). Focus on understanding HOW they were fixed and what lessons Crux (Rust + GPUI/Metal) should learn.

## Executive Summary

Ghostty went through multiple major rewrites and found critical bugs in:
1. **IME handling** (preedit destruction on modifier keys)
2. **Font rendering** (complete rewrite in 1.2.0, 3 patch releases to stabilize)
3. **TERM name** (attempted "ghostty", blocked by Vim hardcoding, reverted to "xterm-ghostty")
4. **Security** (arbitrary code execution via window title sequences)
5. **Metal rendering** (Intel Mac undefined behavior, shader validation)
6. **Keyboard protocol** (Kitty protocol regressions, control character encoding)

**Key lesson**: Even with extensive private beta testing, major subsystem rewrites (font rendering) caused regressions that took months to stabilize.

---

## Critical Bugs Fixed

### 1. IME Preedit Destruction on Modifier Keys

**Issue #4634** (closed in v1.1.0)

#### Bug Description
When typing with Japanese/Korean/CJK IME, pressing ANY modifier key (Shift, Ctrl, Option, Command) would **destroy the preedit text** (composition text). This made IME input nearly unusable.

#### Root Cause
Ghostty was incorrectly handling modifier key events during IME composition. When a modifier key event arrived, it would:
1. Clear the preedit text prematurely
2. Send the key event to the PTY
3. Leave the IME in an inconsistent state

#### Fix (v1.1.0 - January 30, 2025)
- Preedit text no longer disappears when a modifier key is pressed
- Control characters like Ctrl+H now work as expected for IME
- Massive testing with CJK (Chinese, Japanese, Korean), dead keys, emoji, Unicode hex input

**Platform-specific fixes:**
- **macOS**: Tested AquaSKK and macSKK Japanese IME, fixed IME window positioning when `window-padding` is set
- **Linux**: Tested fcitx and ibus under X11/Wayland, found and worked around bug in ibus 1.5.29 causing deadkey input to become "stuck"

#### Related Fixes
- **Issue #5728** (v1.2.0): Fixed pressing backspace with preedit text
- **Issue #7226** (v1.2.0): Key input that clears preedit without text shouldn't encode to PTY

#### Lesson for Crux
**CRITICAL**: Implement IME overlay rendering from day one. NEVER send modifier key events to PTY during composition. NEVER mix preedit text with committed text in the terminal grid.

**Architecture pattern:**
```rust
// Composition text = overlay rendering only, never touches PTY
// Committed text = write to PTY
// Modifier keys during composition = UI feedback only, no PTY write
```

Test with:
- Korean Hangul (2-Set, 3-Set)
- Japanese Hiragana/Katakana
- Chinese Pinyin
- Dead key combinations (macOS ABC - Extended)

**References:**
- [Issue #4634](https://github.com/ghostty-org/ghostty/issues/4634)
- [1.1.0 Release Notes](https://ghostty.org/docs/install/release-notes/1-1-0)
- [1.2.0 Release Notes](https://ghostty.org/docs/install/release-notes/1-2-0)

---

### 2. Font Rendering Complete Rewrite (v1.2.0)

**Major subsystem rewrite - September 15, 2025**

#### Background
Ghostty 1.2.0 contained a **substantial overhaul of the font rendering system**. The renderer backends were reworked so that core logic is shared between OpenGL and Metal.

#### Consequences
Despite extensive testing, the rewrite introduced multiple regressions requiring **3 patch releases** (1.2.1, 1.2.2, 1.2.3) to stabilize.

#### Specific Issues Fixed in 1.2.1 (October 6, 2025)

**Issue: Oversized CJK Characters**
- CJK characters appeared **oversized** when using wide-aspect primary fonts
- **Fix**: IC width (ideographic character width) now upper-bounded by measuring overall bounding box of ASCII characters

**Issue: Nerd Font Icons Misaligned**
- Icons always took 2 cells of space, causing misalignment
- Large gaps around icons
- **Fix**: Icons now larger and better matched in size, icons wider than single cell are **left-aligned** (not centered)

**Issue: FreeType Measurement Inconsistency**
- Glyphs measured with different hinting than rendering
- **Fix**: Ensure glyphs measured with same hinting as rendered

**Issue: Nerd Font Patch Extraction Bug**
- Rules applied to wrong glyphs due to codepoint offset issues
- **Fix**: Correct codepoint offset calculation

#### Continued Fixes in 1.2.3 (October 23, 2025)

Version 1.2.3 "addresses **all currently known font rendering issues**" from the 1.2.0 rewrite:
- Numerous tweaks for edge cases, especially **icon glyphs**
- Fixed issues #9076, #9142, #9160, #9152 (icon rendering edge cases)

#### Lesson for Crux
**DO NOT rewrite font rendering subsystem after launch**. Get it right in Phase 1:
- Test CJK sizing from day one
- Test Nerd Font icons early (powerline, devicons)
- Use consistent measurement/rendering hinting
- Left-align wide icons (Ghostty learned this the hard way)

**Font fallback strategy:**
```rust
// Ghostty's rules (copy these):
1. If bold font not found → use regular font with bold style
2. If italic not available → synthesize by applying slant
3. Measure fallback glyphs with SAME hinting as render
4. Upper-bound IC width by ASCII bounding box
5. Left-align wide icons (>1 cell)
```

**References:**
- [1.2.0 Release Notes](https://ghostty.org/docs/install/release-notes/1-2-0)
- [1.2.1 Release Notes](https://ghostty.org/docs/install/release-notes/1-2-1)
- [1.2.3 Release Notes](https://ghostty.org/docs/install/release-notes/1-2-3)
- [Discussion #8822 - Nerd Fonts glyph width](https://github.com/ghostty-org/ghostty/discussions/8822)
- [Discussion #8651 - Chinese font size too big](https://github.com/ghostty-org/ghostty/discussions/8651)

---

### 3. TERM Name Controversy (xterm-ghostty)

**Attempted in private beta, reverted before 1.0**

#### The Problem
For most of its life, Ghostty advertised `TERM=xterm-ghostty`. The `xterm-` prefix exists because many programs do **string matching** on `$TERM` to determine feature support.

#### The Attempt
For ~1 month in private beta, Ghostty tried to become `TERM=ghostty` (without prefix). They found:
- Many bugs in their own terminfo database
- Upstream issues in other programs
- **Vim 9.0 hardcodes Kitty Keyboard Protocol support** and doesn't respect terminfo

#### The Blocker
Vim 9.0 supports Kitty Keyboard Protocol but **hardcodes the list of terminals** that support it:
```
keyprotocol = "kitty:kitty,foot:kitty,ghostty:kitty,wezterm:kitty,xterm:mok2"
```

Until Vim fixed this bug (patch 9.1.0969) AND downstream distros updated, Ghostty was forced to keep `xterm-ghostty`.

#### Current Status
- **Ghostty's terminfo entry** is in ncurses 6.5-20241228 and above
- `xterm-ghostty` remains the default for compatibility
- Shell integration can now install terminfo automatically via SSH (v1.2.0)

#### Lesson for Crux
**Use `xterm-crux` from day one**. The `xterm-` prefix is critical for compatibility. Ghostty learned this the hard way.

**DO NOT attempt:**
- Pure "crux" TERM name
- "crux-256color" or other creative names
- Assuming terminfo propagation will work

**DO:**
- Ship with `xterm-crux` terminfo
- Submit to ncurses upstream early (6-12 month lag for distros)
- Implement shell integration terminfo installer (Ghostty pattern from 1.2.0)
- Test string-matching apps: Vim, Emacs, tmux, ssh clients

**References:**
- [Terminfo Documentation](https://ghostty.org/docs/help/terminfo)
- [Devlog 004 - TERM name controversy](https://mitchellh.com/writing/ghostty-devlog-004)
- [Discussion #3161 - Error opening terminal: xterm-ghostty](https://github.com/ghostty-org/ghostty/discussions/3161)
- [Vim Issue #16318 - Kitty Keyboard Protocol not used by default](https://github.com/vim/vim/issues/16318)

---

### 4. Security Vulnerability (CVE-2024-56803)

**Fixed in v1.0.1 - December 31, 2024**

#### Vulnerability Details
Ghostty 1.0.0 allowed attackers to execute **arbitrary commands** via window title escape sequences.

**Attack vector:**
1. Attacker sends malicious escape sequence (OSC 21 t - report window title)
2. Title is inserted directly into command line
3. User presses Enter → command executes

**Example:**
```bash
# Attacker sends:
echo -e "\e]0;rm -rf /tmp/*\e\\"
# Terminal reports title via CSI 21 t
# Title text appears on command line
# User presses Enter → disaster
```

#### Classification
- **CVE-2024-56803**
- CWE-94: Improper Control of Generation of Code ('Code Injection')
- CVSS 4.0 Score: 5.1 (Medium severity)
- **Requires user interaction** (pressing Enter)

#### Fix (v1.0.1)
New configuration option `title_report` defaults to **false**:
- Disables title reporting (CSI 21 t) by default
- Can be enabled for compatibility (with security warning in docs)
- Similar vulnerabilities existed in other terminals (iTerm2, etc.)

#### Lesson for Crux
**CRITICAL**: Disable dangerous escape sequences by default.

**High-risk sequences:**
- `CSI 21 t` - Report window title (code injection vector)
- `OSC 52` - Clipboard access (data exfiltration, but useful)
- `OSC 10/11/12` - Query colors (fingerprinting)
- `CSI 18 t` - Report window size (fingerprinting)

**Crux strategy:**
```rust
// Default config (secure):
title_report = false
clipboard_write = "ask"  // Prompt user on first use
color_query = false

// Allow opt-in for power users:
allow_dangerous_sequences = ["title_report", "clipboard"]
```

**Test with:**
- Malicious escape sequences from untrusted sources
- SSH connections to compromised hosts
- Terminal sharing scenarios

**References:**
- [CVE-2024-56803 Advisory](https://github.com/ghostty-org/ghostty/security/advisories/GHSA-5hcq-3j4q-4v6p)
- [1.0.1 Release Notes](https://ghostty.org/docs/install/release-notes/1-0-1)
- [Code Execution Through Ghostty Window Title](https://www.netsecurity.no/en/fagblogg/code-execution-through-ghostty-window-title)

---

### 5. Metal Rendering on Intel Macs

**Issue #3352, fixed in v1.1.1**

#### Bug Description
Red/white reverse "E" artifacts appeared when running Ghostty **fullscreen** on some Intel Mac laptops. Custom shaders broke completely on Intel Macs in v1.2.0.

#### Root Cause
Ghostty was triggering **undefined behavior** on macOS when using **discrete GPUs**:
- All Apple Silicon Macs have integrated GPUs (no issue)
- Intel Macs with discrete GPUs hit Metal API validation failures
- Custom shader compilation failed on Intel (Metal GPU driver bug)

#### Fix (v1.1.1)
Fixed undefined behavior in Metal API calls that only manifested on discrete GPUs.

#### Subsequent Issue (v1.2.0)
Custom shader feature broke again on Intel Macs. Developers considered **disabling custom shaders on Intel Macs entirely** due to Metal GPU driver bugs.

#### Lesson for Crux
**Test on Intel Macs early** (before Apple drops support):
- Integrated GPU (Intel Iris)
- Discrete GPU (AMD Radeon)
- Metal API validation mode (`MTL_DEBUG_LAYER=1`)

**GPUI considerations:**
- GPUI uses Metal on macOS (same risk as Ghostty)
- Verify shader compilation on Intel Macs
- Test with `opt-level = 0` (debug shaders different from release)

**Specific tests:**
- Fullscreen mode (different rendering path)
- Window resize (shader recompilation)
- Multiple displays (GPU switching on laptops)

**Fallback strategy:**
```rust
// Detect Intel Mac + discrete GPU
if is_intel_mac && has_discrete_gpu {
    // Disable advanced shaders
    // OR: Use simplified shader path
    warn!("Intel Mac detected, using compatibility mode");
}
```

**References:**
- [Discussion #3352 - Fullscreen artifacts on Intel Mac](https://github.com/ghostty-org/ghostty/discussions/3352)
- [Discussion #8695 - custom-shader broken on Intel Mac 1.2.0](https://github.com/ghostty-org/ghostty/discussions/8695)
- [1.1.1 Release Notes](https://ghostty.org/docs/install/release-notes/1-1-1)

---

### 6. Kitty Keyboard Protocol Regressions

**Issues in v1.1.1, fixed in v1.1.2**

#### Bug Description (v1.1.1)
Critical regression on macOS: **control-modified keys** stopped working in programs using Kitty Keyboard Protocol (Neovim, Fish 4.0).

**Example:**
- `Ctrl+C` → no response
- `Ctrl+D` → no response
- `Ctrl+[` → encoded as `^[[91;5u` instead of `^[` (violates Kitty spec)

#### Root Cause
Changes to keyboard event handling in 1.1.1 broke the mapping between macOS key events and Kitty protocol encoding.

#### Fix (v1.1.2)
Hotfix release specifically to restore control-modified key functionality.

#### Related Issue: Vim Hardcoding
Vim 9.0 doesn't respect terminfo for Kitty protocol support. It **hardcodes** terminal names:
```
keyprotocol = "kitty:kitty,foot:kitty,ghostty:kitty,wezterm:kitty,xterm:mok2"
```

**Fixed in Vim patch 9.1.0969** to include "ghostty" in the default list.

#### Lesson for Crux
**Kitty Keyboard Protocol is critical for modern editors**:
- Neovim depends on it for key disambiguation
- Fish shell 4.0+ requires it
- Vim requires manual terminfo setup OR hardcoded TERM name

**Implementation checklist:**
- [ ] Implement Kitty progressive enhancement flags
- [ ] Test with Neovim (`:set termguicolors`, key mappings)
- [ ] Test with Fish 4.0+ (completion, key bindings)
- [ ] Test with Kakoune (heavy Kitty protocol user)
- [ ] Verify control character encoding matches spec
- [ ] Test Escape key disambiguation (`Ctrl+[` vs ESC)

**Edge cases from Ghostty:**
- `Ctrl+[` must encode as `^[` (ESC), NOT `^[[91;5u`
- Control characters during IME composition
- Option/Alt key combinations (locale-dependent)

**References:**
- [1.1.1 Release Notes](https://ghostty.org/docs/install/release-notes/1-1-1)
- [1.1.2 Release Notes](https://ghostty.org/docs/install/release-notes/1-1-2)
- [Vim Issue #16318](https://github.com/vim/vim/issues/16318)
- [Discussion #5071 - Ctrl+[ encoding bug](https://github.com/ghostty-org/ghostty/discussions/5071)

---

### 7. Clipboard Fixes (macOS)

**Issue #4956, fixed in v1.1.0**

#### Bug Description
Pasting **multiple files** would separate paths with **newlines** instead of spaces:
```bash
# Before (broken):
/path/to/file1.txt
/path/to/file2.txt

# After (fixed):
/path/to/file1.txt /path/to/file2.txt
```

#### Fix (v1.1.0)
Multiple file paths now separated by **space** instead of newline.

#### Ongoing Issues
Multiple clipboard-related issues remain open:
- **Issue #5838**: `copy-on-select = clipboard` doesn't work (macOS 15.3)
- **Discussion #4898**: Copy-on-select goes to special pasteboard, not system clipboard
- **Discussion #10011**: `copy_to_clipboard` fails on macOS Sequoia 15.7.1

#### Lesson for Crux
**Clipboard is surprisingly complex on macOS**:
- Multiple pasteboards (general, selection, find, drag)
- File URLs vs plain text
- Rich text formats (RTF, HTML)
- Ownership transfer timing

**Phase 3 Crux implementation:**
- Use `NSPasteboard.general` for Copy/Paste
- Consider separate selection pasteboard (X11-style)
- Test file path pasting (Finder drag-and-drop)
- Test with clipboard managers (Alfred, Paste, etc.)

**References:**
- [1.1.0 Release Notes](https://ghostty.org/docs/install/release-notes/1-1-0)
- [Issue #5838 - copy-on-select broken](https://github.com/ghostty-org/ghostty/issues/5838)
- [Discussion #5600 - Clipboard bug in macOS](https://github.com/ghostty-org/ghostty/discussions/5600)

---

### 8. Deadlock in Color Operations

**Fixed in v1.2.3 - October 23, 2025**

#### Bug Description
Programs that emit **many color change or query operations** could cause Ghostty to **hang completely** (deadlock).

**Example scenario:**
- Script repeatedly changes background color (OSC 11)
- Terminal queries current color (OSC 11 ?)
- Deadlock between renderer thread and IO thread

#### Fix (v1.2.3)
Critical deadlock fix. Version 1.2.3 is **highly recommended** for all 1.2.x users.

#### Related Fixes in 1.2.3
- Memory corruption related to scrolling
- Resource leaks
- macOS titlebar tabs improvements (6+ titlebar-related issues)

#### Lesson for Crux
**Color operations are async operations**:
- Renderer thread updates colors
- IO thread processes escape sequences
- PTY thread reads output
- **Potential for deadlock** if locks acquired in wrong order

**GPUI considerations:**
- GPUI has its own threading model
- Background updates vs foreground rendering
- Verify lock ordering with TSan (Thread Sanitizer)

**Test cases:**
```bash
# Rapid color changes:
while true; do
  echo -e "\e]11;#FF0000\e\\"
  echo -e "\e]11;#00FF00\e\\"
done

# Color queries:
while true; do
  echo -e "\e]11;?\e\\"
done
```

**References:**
- [1.2.3 Release Notes](https://ghostty.org/docs/install/release-notes/1-2-3)
- [Issue #9191 - macOS deadlock when selecting text](https://github.com/ghostty-org/ghostty/issues/9191)

---

### 9. SSH Shell Integration

**Feature added in v1.2.0**

#### Problem
Remote hosts don't have `xterm-ghostty` terminfo installed, causing:
- `Error opening terminal: xterm-ghostty`
- Fallback to `TERM=xterm` (loses features)
- Manual terminfo installation required

#### Solution (v1.2.0)
New shell integration features:
- **`ssh-terminfo`**: Automatically copies terminfo to remote machine
- **`ssh-env`**: Sets `TERM=xterm-256color` for SSH sessions (fallback)
- Caches successful installations to avoid repeated attempts

#### Configuration
```
shell-integration-features = ssh-env,ssh-terminfo
```

#### Lesson for Crux
**Phase 5 feature**: Implement automatic terminfo propagation:

```bash
# Shell integration script (injected into .bashrc/.zshrc):
if [[ -n "$SSH_CONNECTION" ]] && [[ "$TERM" == "xterm-crux" ]]; then
  # Check if terminfo exists on remote
  if ! infocmp xterm-crux &>/dev/null; then
    # Copy from local machine via stdin
    cat ~/.terminfo/x/xterm-crux | base64
    # Decode and install on remote
  fi
fi
```

**Cache strategy:**
- Hash of (hostname, username, terminfo content)
- Store in `~/.config/crux/terminfo-cache.json`
- Skip installation if cache hit

**References:**
- [1.2.0 Release Notes](https://ghostty.org/docs/install/release-notes/1-2-0)
- [Shell Integration Documentation](https://ghostty.org/docs/features/shell-integration)
- [Discussion #4156 - Automatic terminfo propagation](https://github.com/ghostty-org/ghostty/discussions/4156)

---

### 10. Float Rounding in Rendering

**From Mitchell Hashimoto's devlogs**

#### Bug Description
Audit of `@intFromFloat` usage found **rounding errors** causing slight rendering artifacts:
- Off-by-one pixel errors
- Misaligned glyph rendering
- Cursor positioning drift

#### Root Cause
Size data structures used **floats** internally, converted to ints for GPU with incorrect rounding.

#### Fix
**Changed all size data structures to use integers**:
- Only convert integers to floats for GPU
- Eliminates float rounding errors entirely
- Described as "hard lesson learned"

#### Lesson for Crux
**GPUI uses floats extensively** (Pixels, Points, PointF32):
- Verify rounding behavior in GPUI coordinate conversions
- Consider integer-based cell grid (rows/cols)
- Convert to float only at final GPU submission

**Audit checklist:**
```rust
// Risky patterns:
let cell_x = (cursor_x as f32 / cell_width) as usize;  // ❌

// Safer patterns:
let cell_x = cursor_x / cell_width;  // ✅ (if both integers)
```

**References:**
- [Ghostty Devlog 001](https://mitchellh.com/writing/ghostty-devlog-001)
- [Ghostty 1.0 Reflection](https://mitchellh.com/writing/ghostty-1-0-reflection)

---

## Additional Notable Fixes

### macOS Titlebar Issues (v1.2.3)
Multiple titlebar-related bugs fixed:
- Title misalignment and clipping in tab titlebar style
- Titlebar coloring in fullscreen mode
- Theme changes causing titlebar to lose styling
- New Tab action unreliable (opened window instead of tab)

**Lesson**: macOS native titlebar integration is complex. Test extensively with:
- Multiple titlebar styles
- Fullscreen mode transitions
- Theme switching (light/dark)
- Window restoration

**Reference**: [1.2.3 Release Notes](https://ghostty.org/docs/install/release-notes/1-2-3)

### Mouse Reporting Edge Cases
- Negative coordinates are **normal** for SGR pixel mouse events (mode 1016)
- Coordinates outside window boundaries can be negative
- XTerm, Ghostty, Foot, Kitty all report negative values

**Lesson**: Don't clamp mouse coordinates to window bounds for SGR pixel mode.

**Reference**: [Discussion #9647 - Mouse tracking negative numbers](https://github.com/ghostty-org/ghostty/discussions/9647)

---

## Top 15 Lessons for Crux

### Critical (Do from Day One)

1. **IME Overlay Rendering** - NEVER mix preedit with PTY. Modifier keys during composition = UI only, no PTY write.

2. **TERM Name = `xterm-crux`** - The `xterm-` prefix is non-negotiable. Ghostty tried to remove it, blocked by Vim hardcoding.

3. **Disable Dangerous Sequences** - `title_report = false` by default. CSI 21 t is a code injection vector.

4. **Font Measurement Consistency** - Measure glyphs with SAME hinting as rendering. Upper-bound IC width by ASCII bbox.

5. **Integer-Based Grid** - Use integers for cell grid, convert to float only for GPU. Avoid float rounding errors.

6. **Test on Intel Macs** - Metal API validation, discrete GPU undefined behavior, shader compilation failures.

7. **Kitty Keyboard Protocol** - Critical for Neovim/Fish. Test control character encoding, especially `Ctrl+[` vs ESC.

### Important (Phase 2-3)

8. **Font Rendering Testing** - Test CJK sizing, Nerd Font icons, italic fallback EARLY. Don't rewrite this subsystem post-launch.

9. **Clipboard Complexity** - macOS has multiple pasteboards. Test file path pasting, rich text, clipboard managers.

10. **Lock Ordering** - Color operations can deadlock. Verify lock ordering with TSan, test rapid OSC 10/11 changes.

11. **Titlebar Integration** - macOS native titlebar is complex. Test fullscreen transitions, theme switching, window restoration.

12. **Mouse Reporting** - SGR pixel mode allows negative coordinates. Don't clamp to window bounds.

### Nice to Have (Phase 5)

13. **SSH Shell Integration** - Auto-install terminfo on remote hosts. Cache installations to avoid repeated attempts.

14. **Terminfo Upstream** - Submit to ncurses early (6-12 month distro lag). Ghostty in ncurses 6.5-20241228.

15. **Private Beta Testing** - Even with extensive beta, major rewrites (font rendering) caused 3 patch releases to stabilize.

---

## Testing Strategy

Based on Ghostty's experience, prioritize:

### Phase 1 (MVP)
- [ ] Korean Hangul IME (2-Set, 3-Set, modifier keys)
- [ ] Japanese Hiragana/Katakana (AquaSKK, macSKK)
- [ ] CJK font sizing (Noto Sans CJK, Source Han Sans)
- [ ] Nerd Font icons (powerline, devicons)
- [ ] TERM=xterm-crux compatibility (Vim, Emacs, tmux)
- [ ] Intel Mac testing (integrated + discrete GPU)

### Phase 2 (Tabs/Panes)
- [ ] Kitty Keyboard Protocol (Neovim, Fish 4.0+)
- [ ] Titlebar integration (fullscreen, theme switching)
- [ ] Window restoration edge cases

### Phase 3 (IME/Clipboard)
- [ ] Clipboard multi-pasteboard handling
- [ ] File path pasting from Finder
- [ ] Clipboard manager compatibility

### Phase 5 (Advanced)
- [ ] SSH shell integration
- [ ] Terminfo auto-propagation
- [ ] Security testing (malicious escape sequences)

---

## Version Timeline

| Version | Date | Key Fixes |
|---------|------|-----------|
| 1.0.0 | Jan 8, 2025 | Public release |
| 1.0.1 | Dec 31, 2024 | **Security**: CVE-2024-56803 (code injection) |
| 1.1.0 | Jan 30, 2025 | **IME**: Preedit modifier key destruction fix |
| 1.1.1 | - | **Metal**: Intel Mac discrete GPU undefined behavior |
| 1.1.2 | - | **Kitty Protocol**: Control-modified keys regression fix |
| 1.2.0 | Sep 15, 2025 | **Font Rewrite**: Shared OpenGL/Metal renderer, SSH integration |
| 1.2.1 | Oct 6, 2025 | **Font Fixes**: CJK oversized, Nerd Font icons, FreeType hinting |
| 1.2.3 | Oct 23, 2025 | **Critical**: Deadlock fix, titlebar improvements, font refinements |

---

## Sources

### Official Documentation
- [Ghostty Release Notes](https://ghostty.org/docs/install/release-notes)
- [Terminfo Documentation](https://ghostty.org/docs/help/terminfo)
- [Shell Integration](https://ghostty.org/docs/features/shell-integration)

### GitHub Issues & Discussions
- [Issue #4634 - Preedit text disappears on modifier keys](https://github.com/ghostty-org/ghostty/issues/4634)
- [Issue #16318 (Vim) - Kitty protocol not used by default](https://github.com/vim/vim/issues/16318)
- [Discussion #3161 - Error opening terminal: xterm-ghostty](https://github.com/ghostty-org/ghostty/discussions/3161)
- [Discussion #3352 - Fullscreen artifacts on Intel Mac](https://github.com/ghostty-org/ghostty/discussions/3352)
- [Discussion #8651 - Chinese font size too big](https://github.com/ghostty-org/ghostty/discussions/8651)
- [Discussion #8822 - Nerd fonts glyph width](https://github.com/ghostty-org/ghostty/discussions/8822)

### Security
- [CVE-2024-56803 Advisory](https://github.com/ghostty-org/ghostty/security/advisories/GHSA-5hcq-3j4q-4v6p)
- [Code Execution Through Ghostty Window Title](https://www.netsecurity.no/en/fagblogg/code-execution-through-ghostty-window-title)

### Blog Posts
- [Ghostty Devlog 001](https://mitchellh.com/writing/ghostty-devlog-001) - Float rounding issues
- [Ghostty Devlog 004](https://mitchellh.com/writing/ghostty-devlog-004) - TERM name controversy
- [Ghostty 1.0 Reflection](https://mitchellh.com/writing/ghostty-1-0-reflection) - Lessons learned

---

## Conclusion

Ghostty's journey from private beta to 1.0 and beyond reveals critical lessons for Crux:

1. **IME is make-or-break for CJK users** - Get overlay rendering right from day one
2. **Font rendering is deceptively complex** - Test CJK, icons, and fallback early
3. **TERM name compatibility matters** - Use `xterm-crux`, don't be clever
4. **Security isn't optional** - Disable dangerous sequences by default
5. **Major rewrites are risky** - Even with extensive testing, font rewrite took 3 patches to stabilize

**Key advantage for Crux**: We can learn from Ghostty's mistakes and implement these fixes from the start, avoiding the same painful regressions.
