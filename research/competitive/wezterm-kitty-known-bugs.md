---
title: "Terminal Emulator Bugs & Issues — Lessons from WezTerm and Kitty"
description: "Known bugs (open and resolved), design pitfalls, and anti-patterns in WezTerm and Kitty that Crux should avoid. Based on GitHub issues, user reports, and community discussions (2020-2026)."
phase: "all"
topics:
  - competitive-analysis
  - bug-patterns
  - wezterm
  - kitty
  - reliability
  - performance
  - ime
  - clipboard
  - rendering
related:
  - competitive/ghostty-warp-analysis.md
  - competitive/terminal-structures.md
  - core/terminal-emulation.md
  - core/performance.md
  - platform/ime-clipboard.md
  - gpui/framework.md
---

# Terminal Emulator Bugs & Issues — Lessons from WezTerm and Kitty

> **Purpose**: Document known bugs, design pitfalls, and common failure modes in popular terminal emulators (WezTerm and Kitty) to inform Crux development and avoid repeating these mistakes.
>
> **Research Date**: 2026-02-12
>
> **Scope**: macOS-specific issues, CJK/IME, performance, rendering, multiplexing, clipboard, configuration, and platform integration bugs.

---

## Table of Contents

1. [Executive Summary](#executive-summary)
2. [WezTerm Issues](#wezterm-issues)
   - [macOS Platform Issues](#wezterm-macos-platform-issues)
   - [CJK and IME Problems](#wezterm-cjk-and-ime-problems)
   - [Performance and Rendering](#wezterm-performance-and-rendering)
   - [Font and Ligature Bugs](#wezterm-font-and-ligature-bugs)
   - [Multiplexer and SSH Issues](#wezterm-multiplexer-and-ssh-issues)
   - [Memory Leaks](#wezterm-memory-leaks)
   - [Configuration Pitfalls](#wezterm-configuration-pitfalls)
3. [Kitty Issues](#kitty-issues)
   - [macOS Platform Issues](#kitty-macos-platform-issues)
   - [CJK and IME Problems](#kitty-cjk-and-ime-problems)
   - [Keyboard Protocol Adoption Barriers](#kitty-keyboard-protocol-adoption-barriers)
   - [Graphics Protocol Bugs](#kitty-graphics-protocol-bugs)
   - [tmux Compatibility](#kitty-tmux-compatibility)
   - [Unicode and Emoji Rendering](#kitty-unicode-and-emoji-rendering)
   - [Configuration Gotchas](#kitty-configuration-gotchas)
   - [Shell Integration Issues](#kitty-shell-integration-issues)
   - [Wayland vs X11](#kitty-wayland-vs-x11)
4. [Cross-Terminal Issues](#cross-terminal-issues)
   - [Clipboard and Selection](#clipboard-and-selection)
   - [TERM Variable and Terminfo](#term-variable-and-terminfo)
   - [Escape Sequence Security](#escape-sequence-security)
   - [Color Scheme Rendering](#color-scheme-rendering)
   - [Scrollback Buffer](#scrollback-buffer)
   - [Font Rendering and Antialiasing](#font-rendering-and-antialiasing)
   - [Battery Drain and GPU Efficiency](#battery-drain-and-gpu-efficiency)
   - [IME Preedit Common Mistakes](#ime-preedit-common-mistakes)
5. [Lessons for Crux](#lessons-for-crux)
6. [References](#references)

---

## Executive Summary

### Key Findings

After analyzing 100+ GitHub issues, community discussions, and bug reports from WezTerm and Kitty (2020-2026), we identified **12 critical anti-patterns** Crux must avoid:

| Anti-Pattern | WezTerm | Kitty | Severity | Crux Mitigation |
|--------------|---------|-------|----------|-----------------|
| **IME preedit mixing with PTY** | ✓ | ✓ | CRITICAL | Never send composition text to PTY (see `research/platform/ime-clipboard.md`) |
| **Memory leaks in window lifecycle** | ✓ | — | HIGH | Test spawn/close cycles, use Rust RAII |
| **Custom TERM without xterm- prefix** | — | ✓ | HIGH | Use `xterm-crux` (Ghostty learned this) |
| **No terminfo fallback over SSH** | ✓ | ✓ | HIGH | Shell integration auto-install (like Ghostty v1.2.0) |
| **Font ligature performance regression** | ✓ | — | MEDIUM | Separate ligature enable/disable from font shaping |
| **Scrollback in main RAM without limits** | ✓ | ✓ | MEDIUM | Configurable limit, warn on >1M lines |
| **Status bar updates blocking rendering** | ✓ | — | MEDIUM | Decouple status bar refresh from main loop |
| **Graphics protocol without tmux support** | — | ✓ | MEDIUM | Design for multiplexer compatibility from day 1 |
| **Clipboard PRIMARY/CLIPBOARD confusion** | ✓ | ✓ | MEDIUM | Clear macOS NSPasteboard semantics |
| **Lua config side effects on reload** | ✓ | — | LOW | Use TOML (static), not scripting |
| **Retro tab bar mouse hover lag** | ✓ | — | LOW | Use native macOS tabs, not custom rendering |
| **High GPU usage with background blur** | ✓ | — | LOW | Warn users about macOS compositor costs |

### WezTerm Top 5 Issues
1. **macOS Freezes** (Issue #6833): 100% CPU usage on macOS 15.4
2. **Memory Leaks** (Issue #3815): 1.4GB RSS growth in 18 hours
3. **IME Ctrl+H Bug** (Issue #7234): Deletes confirmed text instead of preedit
4. **Performance**: Scrolling slower than competitors, high CPU
5. **SSH Multiplexer**: Connection failures, version mismatches

### Kitty Top 5 Issues
1. **macOS Tahoe Crashes** (Issue #8983): Fullscreen + external monitor
2. **IME Dependency on GLFW**: Limited Linux IME support
3. **tmux Incompatibility**: Advanced features don't work
4. **Emoji Rendering**: Variation selectors, width calculation bugs
5. **Shell Integration**: TMUX incompatibility, env var issues

---

## WezTerm Issues

### WezTerm: macOS Platform Issues

#### 1. Freezes on Launch (macOS 15.4)
- **Issue**: [#6833](https://github.com/wezterm/wezterm/issues/6833)
- **Symptom**: WezTerm freezes immediately on launch with 100% CPU usage
- **Affected Version**: macOS 15.4 (March 2025)
- **Root Cause**: Unknown (still open)
- **Lesson**: Test on all macOS versions in CI, especially new releases

#### 2. Window Resize on Drag (macOS Tahoe)
- **Issue**: [#7492](https://github.com/wezterm/wezterm/issues/7492)
- **Symptom**: Dragging window by title bar unexpectedly resizes to near-maximized
- **Platform**: macOS Tahoe (January 2026)
- **Root Cause**: macOS window management API regression
- **Lesson**: Validate window size/position after every macOS API call

#### 3. High GPU Usage on macOS Tahoe
- **Issue**: [#7271](https://github.com/wezterm/wezterm/issues/7271)
- **Symptom**: 50%+ GPU usage per window on M3 Macs
- **Correlation**: Usage scales with window size, not window count
- **Root Cause**: Metal/OpenGL translation inefficiency
- **Lesson**: Profile GPU usage per frame, use Instruments

#### 4. Discrete GPU Activation
- **Issue**: [#2138](https://github.com/wezterm/wezterm/issues/2138)
- **Symptom**: Finder launches use integrated GPU, CLI launches use discrete GPU
- **Impact**: Battery drain on dual-GPU MacBooks
- **Lesson**: Set `NSSupportsAutomaticGraphicsSwitching` in Info.plist

#### 5. Background Blur GPU Cost
- **Issue**: [#5555](https://github.com/wezterm/wezterm/issues/5555)
- **Symptom**: `macos_window_background_blur > 0` causes 20-40% GPU usage
- **Root Cause**: macOS compositor cost, not WezTerm
- **Lesson**: Document performance cost of blur, provide toggle

---

### WezTerm: CJK and IME Problems

#### 1. Wide CJK Character Splits
- **Issue**: [#614](https://github.com/wezterm/wezterm/issues/614)
- **Symptom**: Random 1px vertical lines in middle of wide characters
- **Root Cause**: Floating-point error in cell coordinate calculation
- **Fixed**: Yes
- **Lesson**: Use integer cell coordinates, avoid float rounding

#### 2. Katakana Conversion Shortcut (Ctrl+K)
- **Issue**: [#5533](https://github.com/wezterm/wezterm/issues/5533)
- **Symptom**: Ctrl+K doesn't convert Hiragana → Katakana with Kotoeri IME
- **Root Cause**: Shortcut conflicts with terminal keybindings
- **Lesson**: Provide `macos_forward_to_ime_modifier_mask` config

#### 3. Ctrl+H Deletes Confirmed Text (Japanese IME)
- **Issue**: [#7234](https://github.com/wezterm/wezterm/issues/7234)
- **Symptom**: Ctrl+H deletes confirmed characters instead of preedit
- **Platform**: macOS Japanese IME
- **Root Cause**: Incorrect preedit state tracking
- **Lesson**: **NEVER mix confirmed vs preedit text lifecycle**

#### 4. Search Overlay IME Input
- **Issue**: [#5333](https://github.com/wezterm/wezterm/issues/5333)
- **Symptom**: Chinese IME input goes to command line instead of search
- **Fixed**: Yes (recent versions)
- **Lesson**: Each overlay needs separate IME context

#### 5. Preedit on All Panes
- **Issue**: [#2569](https://github.com/wezterm/wezterm/issues/2569)
- **Symptom**: IME preedit renders on ALL pane cursors (Windows)
- **Root Cause**: Global preedit state instead of per-pane
- **Lesson**: Preedit state must be scoped to active pane

---

### WezTerm: Performance and Rendering

#### 1. High CPU on Scrolling
- **Issue**: [#5400](https://github.com/wezterm/wezterm/discussions/5400)
- **Symptom**: CPU usage much higher than Windows Terminal when scrolling
- **Impact**: "Feels like molasses" on 4K monitors
- **Root Cause**: Window size affects framerate (larger = slower)
- **Lesson**: Optimize rendering for large window sizes

#### 2. Low Framerate on macOS 11
- **Issue**: [#790](https://github.com/wezterm/wezterm/issues/790)
- **Symptom**: Slow framerate compared to Alacritty/iTerm2
- **Root Cause**: OpenGL → Metal translation overhead
- **Mitigation**: `front_end = "WebGpu"` for direct Metal
- **Lesson**: Use native Metal on macOS, not OpenGL

#### 3. Complex Status Bar Lag
- **Issue**: [#4788](https://github.com/wezterm/wezterm/issues/4788)
- **Symptom**: Input lag and choppy scrolling with `update-status` event
- **Root Cause**: Status bar updates block main rendering loop
- **Lesson**: Decouple status bar refresh from frame rendering

#### 4. Retro Tab Bar Mouse Hover Hang
- **Issue**: [#5054](https://github.com/wezterm/wezterm/issues/5054)
- **Symptom**: Sweeping mouse over tabs causes lag and hangs
- **Scope**: Only with `format-tab-title` + retro style
- **Lesson**: Avoid custom tab rendering, use native UI

#### 5. Cache Blowout on Scrollback
- **Discussion**: [#751](https://github.com/wezterm/wezterm/discussions/751)
- **Symptom**: Scrolling rapidly through history is slow
- **Root Cause**: Font glyph cache misses
- **Tradeoff**: Larger cache = more memory
- **Lesson**: Tune cache size vs memory usage

---

### WezTerm: Font and Ligature Bugs

#### 1. Inconsistent Ligature Rendering
- **Issue**: [#4874](https://github.com/wezterm/wezterm/issues/4874)
- **Symptom**: Ligatures don't take up same space, "stair step" effect
- **Example**: `=>` sometimes misaligned
- **Root Cause**: Font shaping inconsistency
- **Lesson**: Test ligature spacing with multiple fonts

#### 2. Cursor Darkens Whole Ligature
- **Issue**: [#478](https://github.com/wezterm/wezterm/issues/478)
- **Symptom**: Cursor on one end of ligature makes entire ligature black
- **Scope**: Font-dependent
- **Lesson**: Render cursor overlay, don't modify glyph color

#### 3. Color Change Splits Ligatures
- **Symptom**: `>=` ligature breaks if `>` and `=` have different colors
- **Root Cause**: Ligature formation happens before colorization
- **Lesson**: This is expected behavior, document it

#### 4. Ligatures Rendered Left of Cell
- **Issue**: [#2888](https://github.com/wezterm/wezterm/issues/2888)
- **Symptom**: Ligatures appear one cell to the left
- **Root Cause**: Cell coordinate calculation bug
- **Lesson**: Validate glyph origin against cell grid

#### 5. Gaps at Small Font Sizes
- **Issue**: [#6931](https://github.com/wezterm/wezterm/issues/6931)
- **Symptom**: Gaps between cells with ligatures at small sizes
- **Related to**: Antialiasing
- **Lesson**: Test font rendering at 8pt, 10pt, 12pt

#### 6. Ligature Performance Regression
- **Issue**: [#5280](https://github.com/wezterm/wezterm/issues/5280)
- **Symptom**: Enabling ligature-capable fonts causes lag
- **Impact**: Decreased responsiveness even with ligatures disabled
- **Lesson**: Separate ligature enable/disable from font selection

---

### WezTerm: Multiplexer and SSH Issues

#### 1. Lines Flushed Incorrectly
- **Issue**: [#3558](https://github.com/wezterm/wezterm/issues/3558)
- **Symptom**: SSH multiplexing renders lines incorrectly in Neovim
- **Root Cause**: Rendering corruption in interactive UI
- **Lesson**: Test with complex TUI apps (nvim, htop, etc.)

#### 2. SSH Connection Stops After Hours
- **Issue**: [#7014](https://github.com/wezterm/wezterm/issues/7014)
- **Symptom**: Can't split/open tabs after a few hours
- **Error**: "Channel opening failure: channel 64 error (2)"
- **Lesson**: Implement connection keepalive, reconnect logic

#### 3. Can't Connect After Server Restart
- **Issue**: [#6452](https://github.com/wezterm/wezterm/issues/6452)
- **Symptom**: `wezterm connect` fails after server freezes/restarts
- **Impact**: Regular SSH unaffected, only mux
- **Lesson**: Provide manual reconnect command, auto-cleanup stale sockets

#### 4. SSH Agent with Mux Disables Key Auth
- **Issue**: [#5817](https://github.com/wezterm/wezterm/issues/5817)
- **Symptom**: `mux_enable_ssh_agent = true` causes "Permission denied (publickey)"
- **Root Cause**: Private key authentication disabled
- **Lesson**: SSH agent should augment, not replace key auth

#### 5. Version Mismatch Errors
- **Common Error**: "Please install the same version of wezterm on both the client and server!"
- **Root Cause**: Client/server version incompatibility
- **Lesson**: Version check should be semver-compatible, not exact match

#### 6. Pane Size Adjustment Skipped
- **Issue**: [#6844](https://github.com/wezterm/wezterm/issues/6844)
- **Symptom**: Multiple `AdjustPaneSize` commands skipped in SSH mux
- **Lesson**: Batch resize commands, acknowledge completion

---

### WezTerm: Memory Leaks

#### 1. Kitty GIF Memory Leak
- **Issue**: [#7400](https://github.com/wezterm/wezterm/issues/7400)
- **Symptom**: Memory steadily increases, CPU 100%, GUI freezes
- **Trigger**: Kitty graphics protocol GIF animations
- **Lesson**: Implement image cache eviction, max memory limit

#### 2. Excessive Memory Growth
- **Issue**: [#2626](https://github.com/wezterm/wezterm/issues/2626)
- **Symptom**: 1.4GB RSS after 18 hours with single window
- **Root Cause**: Memory not freed until ALL windows closed
- **Lesson**: Implement per-window memory cleanup

#### 3. Hashmap Memory Leaks
- **Issue**: [#3815](https://github.com/wezterm/wezterm/issues/3815)
- **Source**: `hashbrown` hashmap usage
- **Symptom**: Leaks at startup and during usage
- **Lesson**: Use Rust's built-in HashMap, profile with Valgrind

#### 4. Thread Leak on Spawn/Close
- **Issue**: [#6116](https://github.com/wezterm/wezterm/issues/6116)
- **Symptom**: +5MB memory and +2 threads per spawn/close cycle
- **Root Cause**: Threads not cleaned up
- **Lesson**: Use `Arc` + `Weak` for cyclic refs, join threads on drop

---

### WezTerm: Configuration Pitfalls

#### 1. Side Effects in Config Flow
- **Source**: [Official docs](https://wezterm.org/config/files.html)
- **Anti-pattern**: Launching background processes in config
- **Impact**: Many processes spawned on every config reload
- **Lesson**: Config should be pure data, no side effects

#### 2. Invalid Lua Table Syntax
- **Issue**: [#7896](https://github.com/anthropics/claude-code/issues/7896)
- **Common Mistake**: Missing closing braces, incorrect action syntax
- **Example**: `wezterm.action{SendString="\x1b\r"}` → `act.SendString("\x1b\r")`
- **Lesson**: Use `wezterm.config_builder()` for validation

#### 3. Config Reload Doesn't Update Status Bar
- **Issue**: [#4892](https://github.com/wezterm/wezterm/issues/4892)
- **Symptom**: Tab bar strftime frozen after updating to 20240127
- **Lesson**: Force re-render after config reload

---

## Kitty Issues

### Kitty: macOS Platform Issues

#### 1. Crashes on Tahoe Fullscreen + External Monitor
- **Issue**: [#8983](https://github.com/kovidgoyal/kitty/issues/8983)
- **Symptom**: Kitty crashes when fullscreen and reconnecting MacBook to monitor
- **Platform**: macOS 26 Tahoe (September 2025)
- **Fixed**: Workaround in v0.44
- **Lesson**: Test multi-monitor dock/undock scenarios

#### 2. Random Crashes on M2
- **Issue**: [#6997](https://github.com/kovidgoyal/kitty/issues/6997)
- **Symptom**: Random crashes on M2 Macs
- **Root Cause**: Unknown
- **Lesson**: Crash reporting with detailed macOS version + CPU info

#### 3. Quick Access Terminal Crashes
- **Fixed in**: v0.44 (2025)
- **Lesson**: Test macOS-specific UI features (Quick Access, etc.)

#### 4. File Drop Handling Issues
- **Fixed in**: v0.44 (2025)
- **Lesson**: Test drag-and-drop from Finder, other apps

---

### Kitty: CJK and IME Problems

#### 1. IME Doesn't Work by Default (Linux)
- **Issue**: [#469](https://github.com/kovidgoyal/kitty/issues/469), [Debian Bug #990316](https://bugs.debian.org/cgi-bin/bugreport.cgi?bug=990316)
- **Root Cause**: GLFW limitation, intentionally disabled by author
- **Workaround**: `GLFW_IM_MODULE=ibus` (works for ibus and fcitx5)
- **Developer Rationale**: "Efficiency issues and bugs"
- **Lesson**: **Don't rely on GLFW for IME**, use native APIs

#### 2. CJK Input Fails on Wayland + Sway
- **Issue**: [Debian Forums](https://forums.debian.net/viewtopic.php?t=155899)
- **Workaround**: `linux_display_server x11` to force xwayland
- **Lesson**: Test Wayland IME separately from X11

#### 3. Japanese → English Switching Broken
- **Issue**: [#8131](https://github.com/kovidgoyal/kitty/issues/8131)
- **Symptom**: English → Japanese works, reverse doesn't
- **Lesson**: Test bidirectional IME switching

#### 4. macOS IME Compatibility
- **Issue**: [#910](https://github.com/kovidgoyal/kitty/issues/910), [#4219](https://github.com/kovidgoyal/kitty/issues/4219)
- **Symptom**: Can't type Chinese/Japanese on macOS
- **Root Cause**: IME integration incomplete
- **Lesson**: Use `NSTextInputClient` directly, not GLFW

---

### Kitty: Keyboard Protocol Adoption Barriers

#### 1. No Terminal Detection
- **Issue**: [foot #1642](https://codeberg.org/dnkl/foot/issues/1642), [RFC #3248](https://github.com/kovidgoyal/kitty/issues/3248)
- **Problem**: No way for terminal to tell app which level it supports
- **Impact**: Apps must implement all levels or none
- **Lesson**: Provide capability query escape sequence

#### 2. Not Regulated by Standards Body
- **Source**: [Suckless discussion](https://dev.suckless.narkive.com/wctsTGzs/st-thoughts-on-kitty-keyboard-protocol)
- **Criticism**: "Just a proposal used by one terminal emulator"
- **Adoption Risk**: Early rollout bugs, hard sell for TUI users
- **Lesson**: Wait for wider adoption before implementing

#### 3. Ambiguity with Traditional Sequences
- **Problem**: No way to reliably distinguish Esc key from escape sequence start
- **Impact**: Fragile timing-related hacks
- **Lesson**: Implement timeout-based detection, make configurable

---

### Kitty: Graphics Protocol Bugs

#### 1. Image Deletion Deletes All Images
- **Issue**: [#5081](https://github.com/kovidgoyal/kitty/issues/5081)
- **Symptom**: Delete action removes all images, including scrolled-past
- **Root Cause**: Incorrect scope of deletion command
- **Lesson**: Track image lifecycle per screen region

#### 2. Image ID Replacement Incompatibility
- **Issue**: [Ghostty #6711](https://github.com/ghostty-org/ghostty/issues/6711)
- **Symptom**: Kitty rejects re-sending image data with `a=t` for existing ID
- **Ghostty Behavior**: Supports re-send for animation updates
- **Lesson**: Protocol ambiguity → document explicitly

#### 3. Doesn't Work in tmux
- **Symptom**: Kitty graphics protocol completely fails in tmux
- **Root Cause**: tmux doesn't pass through protocol
- **Lesson**: **Design graphics for multiplexer compatibility**

#### 4. Unicode Placeholder Wrapping
- **Issue**: [#3163](https://github.com/kovidgoyal/kitty/issues/3163)
- **Symptom**: Images wrap at screen edge instead of truncating
- **Impact**: Image appears interleaved with blank lines
- **Lesson**: Clip images at viewport boundary

---

### Kitty: tmux Compatibility

#### 1. Multiple TERM Variables Break tmux
- **Source**: [FAQ](https://sw.kovidgoyal.net/kitty/faq/)
- **Problem**: Starting tmux in one terminal, switching to another with different TERM
- **Impact**: tmux doesn't support multiple terminfo definitions
- **Lesson**: tmux + custom TERM = pain, document clearly

#### 2. Missing Terminfo on Remote
- **Issue**: [#1241](https://github.com/kovidgoyal/kitty/issues/1241)
- **Symptom**: "open terminal failed: missing or unsuitable terminal: xterm-kitty"
- **Root Cause**: Remote server doesn't have kitty terminfo
- **Lesson**: Ship terminfo installer, auto-deploy over SSH

#### 3. Advanced Features Don't Work
- **Affected**: Styled underlines, notifications, variable-sized text, extended keyboard
- **Quote**: "May or may not work depending on tmux version and whims of maintainer"
- **Lesson**: Don't rely on tmux for advanced features

#### 4. Ancient tmux Versions
- **Issue**: [#877](https://github.com/kovidgoyal/kitty/issues/877)
- **Symptom**: Gibberish on screen with tmux 1.8
- **Lesson**: Document minimum tmux version (3.2+)

---

### Kitty: Unicode and Emoji Rendering

#### 1. Emoji Not Rendered Correctly
- **Issue**: [#2821](https://github.com/kovidgoyal/kitty/issues/2821), [#4871](https://github.com/kovidgoyal/kitty/issues/4871)
- **Symptom**: Some emoji don't display at all or appear wrong
- **Root Cause**: Font fallback order varies between instances
- **Lesson**: Deterministic fallback chain, log which font was chosen

#### 2. Unicode 7.0 Emoji Too Wide
- **Issue**: [#3312](https://github.com/kovidgoyal/kitty/issues/3312)
- **Symptom**: Emoji with `Default_Emoji_Style` text not rendered
- **Lesson**: Check East Asian Width property, not just emoji property

#### 3. Variation Selector Changes Width
- **Issue**: [#3998](https://github.com/kovidgoyal/kitty/issues/3998)
- **Symptom**: U+26A0 (⚠︎) = 1 column, U+26A0 U+FE0F (⚠️) = 2 columns
- **Expected**: VS-16 changes presentation, should affect width
- **Lesson**: Width calculation must account for variation selectors

#### 4. Private Use Area Width
- **Issue**: Unicode standard sets PUA width to 1, but many are wide
- **Impact**: Symbols render smaller or truncated
- **Lesson**: Provide config override for PUA char widths

---

### Kitty: Configuration Gotchas

#### 1. Environment Variables Not Loaded
- **Issue**: [FAQ](https://sw.kovidgoyal.net/kitty/faq/)
- **Problem**: LANG, LC_*, PATH not set correctly
- **Solution**: `env read_from_shell=PATH LANG LC_* XDG_* EDITOR VISUAL`
- **Lesson**: Document env var loading explicitly

#### 2. Remote Control Security Risk
- **Warning**: [FAQ](https://sw.kovidgoyal.net/kitty/faq/)
- **Risk**: Other programs can control all aspects of kitty, even over SSH
- **Impact**: Send text, open/close windows, read content
- **Lesson**: Disable remote control by default, require explicit opt-in

#### 3. TERM Variable Pitfall
- **Warning**: "Changing this can break many terminal programs"
- **Source**: Stack Overflow advice to change TERM
- **Lesson**: Never suggest changing TERM, fix terminfo instead

#### 4. Scrollback Line Balance
- **Tradeoff**: Too small = lose history, too large = RAM hit
- **Lesson**: Default 10,000 lines, warn if > 100,000

---

### Kitty: Shell Integration Issues

#### 1. Escape Codes Visible
- **Issue**: [#4765](https://github.com/kovidgoyal/kitty/issues/4765)
- **Symptom**: OSC 133;A and 133;C appear as visible characters
- **Root Cause**: Shell integration not recognized
- **Lesson**: Test shell integration on bash, zsh, fish

#### 2. Keybinding Conflicts with Fish
- **Issue**: [#5906](https://github.com/kovidgoyal/kitty/issues/5906)
- **Symptom**: Ctrl+Alt+J/K work when integration disabled, fail when enabled
- **Lesson**: Document keybinding conflicts, provide overrides

#### 3. KITTY_SHELL_INTEGRATION Not Set
- **Issue**: [#6783](https://github.com/kovidgoyal/kitty/issues/6783), [#7809](https://github.com/kovidgoyal/kitty/issues/7809)
- **Symptom**: Env var not set with `shell_integration = no-rc`
- **Platform**: macOS especially
- **Lesson**: Test env var propagation on all platforms

#### 4. Bash History File Changes
- **Issue**: [#5534](https://github.com/kovidgoyal/kitty/issues/5534)
- **Symptom**: `.bash_history` → `.sh_history` when integration enabled
- **Lesson**: Don't change shell defaults without user consent

#### 5. DEBUG Warnings in Bash
- **Symptom**: `command_builtin` DEBUG warnings on new terminal
- **Lesson**: Suppress internal debugging in shipped integration

#### 6. TMUX Incompatibility
- **Issue**: [#4599](https://github.com/kovidgoyal/kitty/issues/4599)
- **Symptom**: Shell integration doesn't work inside tmux
- **Root Cause**: tmux filters escape sequences
- **Lesson**: Document tmux limitations clearly

---

### Kitty: Wayland vs X11

#### 1. Wayland Backend Lag
- **Issue**: [#9026](https://github.com/kovidgoyal/kitty/issues/9026)
- **Symptom**: Delayed rendering, lower FPS on Wayland vs X11
- **Platform**: Hyprland compositor
- **Lesson**: Optimize Wayland rendering path separately

#### 2. Force xwayland Backend
- **Issue**: [#2648](https://github.com/kovidgoyal/kitty/issues/2648)
- **Config**: `linux_display_server x11`
- **Reason**: X11 currently more stable for some workflows
- **Lesson**: Provide display server override

---

## Cross-Terminal Issues

### Clipboard and Selection

#### 1. Selection Automatically Copies
- **Issue**: [Terminator #242](https://github.com/gnome-terminator/terminator/issues/242)
- **Problem**: Selecting text to delete/paste over copies to clipboard
- **Impact**: Overwrites previously copied text
- **Lesson**: Provide "copy-on-select" toggle (default off)

#### 2. Tmux Pane Borders in Selection
- **Source**: [seanh.cc](https://www.seanh.cc/2020/12/27/copy-and-paste-in-tmux/)
- **Problem**: Copying text includes pane borders and other pane content
- **Lesson**: Teach users about tmux copy mode

#### 3. PRIMARY vs CLIPBOARD Confusion
- **Issue**: [ArchWiki](https://wiki.archlinux.org/title/Copying_text_from_a_terminal)
- **Problem**: Some emulators copy to PRIMARY, not CLIPBOARD
- **Impact**: System clipboard managers don't see it
- **Lesson**: macOS: always use NSPasteboard general, Linux: copy to both

#### 4. Ubuntu 24.04 Clipboard Regression
- **Issue**: [Terminator #905](https://github.com/gnome-terminator/terminator/issues/905)
- **Symptom**: Select-to-copy works within terminal, not with other apps
- **Lesson**: Test clipboard integration on each OS release

#### 5. Copy-on-Select Fails in tmux + Kitty
- **Issue**: [opencode #9942](https://github.com/anomalyco/opencode/issues/9942)
- **Platform**: Linux
- **Lesson**: tmux mouse mode disables terminal clipboard

---

### TERM Variable and Terminfo

#### 1. The xterm-256color Lie
- **Source**: [State of the Terminal](https://gpanders.com/blog/state-of-the-terminal/)
- **Problem**: Many terminals claim to be "xterm-256color" but aren't
- **Impact**: "Kinda sorta xterm-256color" intersection of features
- **Lesson**: Use `xterm-` prefix for compatibility, but custom terminfo

#### 2. Outdated ncurses on macOS
- **Problem**: macOS ncurses didn't include `tmux-256color` for years
- **Impact**: tmux users had to install custom terminfo
- **Lesson**: Ship terminfo installer with app

#### 3. Missing Terminfo on SSH
- **Error**: "open terminal failed: missing or unsuitable terminal: xterm-ghostty"
- **Solution**: [Ghostty v1.2.0](https://vninja.net/2024/12/28/ghostty-workaround-for-missing-or-unsuitable-terminal-xterm-ghostty/) auto-installs via shell integration
- **Lesson**: Auto-deploy terminfo over SSH

#### 4. Multiple TERM Values Break tmux
- **Source**: [Kitty FAQ](https://sw.kovidgoyal.net/kitty/faq/)
- **Problem**: Starting tmux in one terminal, attaching from another
- **Impact**: tmux doesn't support multiple terminfo definitions
- **Lesson**: Document tmux + custom TERM limitations

#### 5. Terminfo Bugs
- **Source**: [Text-Terminal-HOWTO](https://tldp.org/HOWTO/Text-Terminal-HOWTO-16.html)
- **Issues**: Incomplete files, features not defined
- **Lesson**: Validate terminfo with `infocmp`, test with `vttest`

#### 6. Query vs Terminfo
- **Source**: [State of the Terminal](https://gpanders.com/blog/state-of-the-terminal/)
- **Advantage**: Querying solves $TERM, SSH, and outdated database problems
- **Lesson**: Support DA1, DA2, DA3 queries for capability detection

---

### Escape Sequence Security

#### 1. Command Injection via ANSI
- **Source**: [CyberArk](https://www.cyberark.com/resources/threat-research-blog/dont-trust-this-title-abusing-terminal-emulators-with-ansi-escape-characters), [Protean Security](https://www.proteansec.com/linux/blast-past-executing-code-terminal-emulators-via-escape-sequences/)
- **Attack**: Inject commands via escape characters in malicious text
- **Example**: Changing terminal color, then executing injected commands
- **Lesson**: Sanitize escape sequences in untrusted input

#### 2. Buffer Overflow from Large Parameters
- **Testing**: [HD Moore](https://hdm.io/writing/termulation.txt)
- **Symptom**: rxvt crashes, screen 100% CPU denial of service
- **Root Cause**: Large/negative integer parameters
- **Lesson**: Validate all numeric parameters, set max limits

#### 3. Sixel Format Overflow
- **Found in**: xterm, libsixel
- **Root Cause**: Memory-unsafe parsing
- **Lesson**: Use Rust, fuzz escape sequence parser

#### 4. OSC 8 Hyperlink Escaping
- **Issue**: [less pager](https://dgl.cx/2023/09/ansi-terminal-security)
- **Problem**: Didn't correctly handle escape sequences
- **Impact**: Raw escape sequences sent to terminal
- **Lesson**: Validate OSC 8 URLs, sandbox hyperlink handling

#### 5. Inconsistent Error Handling
- **Source**: [ANSI X3.64-1979](https://dgl.cx/2023/09/ansi-terminal-security)
- **Problem**: Standard doesn't specify error handling
- **Impact**: Terminals handle malformed sequences differently
- **Lesson**: Define error handling policy, fuzz test

---

### Color Scheme Rendering

#### 1. Color Mismatch Between Terminals
- **Issue**: [WezTerm #2287](https://github.com/wezterm/wezterm/issues/2287), [#4680](https://github.com/wezterm/wezterm/discussions/4680)
- **Symptom**: Same color scheme looks different in Kitty vs WezTerm
- **Example**: #c4746e looks red in Kitty, orange in WezTerm
- **Root Cause**: Different color calibration
- **Lesson**: Document color profile (sRGB, Display P3)

#### 2. Foreground = Background
- **Issue**: [WezTerm #2287](https://github.com/wezterm/wezterm/issues/2287)
- **Symptom**: iTerm2/Kitty scheme works, WezTerm shows invisible text
- **Root Cause**: Incorrect theme import
- **Lesson**: Validate fg/bg contrast on theme load

#### 3. bold_brightens_ansi_colors Difference
- **Observation**: Kitty doesn't have this option, behaves like `false`
- **Impact**: Bold text looks different between terminals
- **Lesson**: Document bold rendering behavior

#### 4. Image Blending
- **Issue**: [WezTerm #7222](https://github.com/wezterm/wezterm/issues/7222)
- **Symptom**: Text doesn't blend with kitty images in WezTerm
- **Lesson**: Test image + text overlay rendering

---

### Scrollback Buffer

#### 1. Memory Pressure from Large Scrollback
- **Source**: [DediRock](https://dedirock.com/blog/increasing-the-scrollback-buffer-size-in-linux-terminal-emulators-a-step-by-step-guide/)
- **Problem**: Larger values require more RAM, especially with many tabs
- **Lesson**: Default 10,000 lines, warn if > 100,000

#### 2. Performance Degradation
- **Issue**: [Claude Code #11260](https://github.com/anthropics/claude-code/issues/11260)
- **Symptom**: Repeated clearing doesn't remove from scrollback
- **Impact**: Scroll jumping, slow rendering
- **Lesson**: Implement scrollback compaction on clear

#### 3. Hard Limits
- **Example**: VS Code debug console = 10,000 lines
- **Reason**: Performance issues with unlimited scrollback
- **Lesson**: Enforce configurable max

---

### Font Rendering and Antialiasing

#### 1. macOS Mojave Subpixel Antialiasing
- **Issue**: [ahmadawais.com](https://ahmadawais.com/fix-macos-mojave-font-rendering-issue/)
- **Change**: Apple disabled subpixel AA in Mojave
- **Impact**: Fonts look strange on non-Retina displays
- **Lesson**: Let macOS handle antialiasing, don't override

#### 2. Linux Subpixel Antialiasing
- **Issue**: [Kitty #214](https://github.com/kovidgoyal/kitty/issues/214)
- **Problem**: Kitty uses grey AA instead of respecting fontconfig
- **Lesson**: Read fontconfig settings, allow override

#### 3. Greyscale vs Subpixel
- **WezTerm Config**: `font_antialias = "Greyscale"` (default)
- **Options**: None, Greyscale, Subpixel
- **Lesson**: Provide config option, default to platform preference

#### 4. Warp Blur on Linux
- **Issue**: [Warp #4304](https://github.com/warpdotdev/Warp/issues/4304)
- **Symptom**: Blurrier text than GNOME Terminal
- **Suspected**: Lack of subpixel rendering
- **Lesson**: Match system font rendering settings

---

### Battery Drain and GPU Efficiency

#### 1. Warp Forces Discrete GPU
- **Issue**: [Warp #76](https://github.com/warpdotdev/Warp/issues/76)
- **Impact**: High-power GPU active even when Warp not in use
- **Platform**: Dual-GPU MacBooks
- **Lesson**: Set `NSSupportsAutomaticGraphicsSwitching` in Info.plist

#### 2. GPU Acceleration Tradeoffs
- **Source**: [Hacker News](https://news.ycombinator.com/item?id=29528343)
- **Benefit**: Lower CPU heat, offload to GPU
- **Cost**: Overall power consumption increases with high-end GPU
- **Lesson**: Profile total system power, not just CPU

#### 3. Alacritty Efficiency
- **Claim**: ~500 FPS with OpenGL, only draws when state changes
- **Impact**: Battery-friendly
- **Lesson**: Skip frames when no changes, use damage tracking

#### 4. High GPU Usage from Blur
- **Issue**: [WezTerm #5555](https://github.com/wezterm/wezterm/issues/5555)
- **Symptom**: 20-40% GPU usage with `macos_window_background_blur > 0`
- **Lesson**: Document compositor costs, recommend disabling

---

### IME Preedit Common Mistakes

#### 1. Sending Preedit to PTY
- **Issue**: [Terminal #13681](https://github.com/microsoft/terminal/issues/13681)
- **Symptom**: Composition text sent to shell before confirmation
- **Root Cause**: Mixing preedit and confirmed text lifecycle
- **Lesson**: **NEVER send preedit to PTY, only confirmed text**

#### 2. Preedit Not Displayed
- **Issue**: [Neovide #1931](https://github.com/neovide/neovide/issues/1931)
- **Root Cause**: IME UI disabled or not rendered
- **Lesson**: Overlay preedit on canvas, don't mix with grid

#### 3. Blank or Disappearing Preedit
- **Issue**: [Codex #4870](https://github.com/openai/codex/issues/4870)
- **Symptom**: Korean IME shows blanks or disappears
- **Platform**: Windows
- **Lesson**: Render preedit immediately on `insertText` callback

#### 4. IME UI Doesn't Follow Cursor
- **Issue**: [Terminal #459](https://github.com/microsoft/terminal/issues/459)
- **Symptom**: IME popup in upper-left corner, not at cursor
- **Root Cause**: Cursor position not updated to IME
- **Lesson**: Call `setMarkedTextSelectedRange` on every cursor move

#### 5. Multiple Cursor Location Calls
- **Issue**: [Scintilla #2135](https://sourceforge.net/p/scintilla/bugs/2135/)
- **Symptom**: Cursor location function called multiple times
- **Impact**: Flickering IME popup
- **Lesson**: Debounce cursor position updates

#### 6. Can't Clear Preedit with Backspace
- **Issue**: [Alacritty #6313](https://github.com/alacritty/alacritty/issues/6313)
- **Symptom**: Holding backspace disappears IME popup, first char remains
- **Lesson**: Track preedit length, clear overlay on empty preedit

#### 7. Insufficient Synchronization
- **Root Cause**: TUI rerendering vs IME composition events race
- **Lesson**: Render preedit after grid render, in separate pass

---

## Lessons for Crux

### Critical Anti-Patterns to Avoid

| # | Anti-Pattern | WezTerm | Kitty | Mitigation |
|---|--------------|---------|-------|------------|
| 1 | **IME preedit sent to PTY** | ✓ (#7234) | ✓ (Linux) | Overlay preedit on canvas, NEVER write to grid/PTY |
| 2 | **Memory not freed per-window** | ✓ (#2626) | — | RAII, test spawn/close cycles, profile with Instruments |
| 3 | **Custom TERM without xterm- prefix** | — | ✓ | Use `xterm-crux`, learn from Ghostty's mistake |
| 4 | **No terminfo auto-install over SSH** | ✓ | ✓ (#1241) | Shell integration auto-deploys terminfo (Ghostty v1.2.0) |
| 5 | **Font ligatures degrade performance** | ✓ (#5280) | — | Separate ligature on/off from font selection |
| 6 | **Status bar blocks rendering loop** | ✓ (#4788) | — | Async status updates, max 10 FPS for status bar |
| 7 | **Graphics protocol doesn't work in tmux** | — | ✓ | Design for multiplexer pass-through from day 1 |
| 8 | **Clipboard PRIMARY/CLIPBOARD confusion** | ✓ | ✓ | macOS: NSPasteboard general only, Linux: both |
| 9 | **Config reload side effects** | ✓ (Lua) | — | Use TOML (static data), not scripting language |
| 10 | **Scrollback unlimited RAM growth** | ✓ | ✓ | Default 10k, warn >100k, enforce max |
| 11 | **Escape sequence parameter overflow** | ✓ | ✓ | Validate all numeric params, max 65535 |
| 12 | **GPU blur without performance warning** | ✓ (#5555) | — | Document macOS compositor costs |

### Crux-Specific Recommendations

#### Phase 1: Basic Terminal MVP
- [x] **IME**: Use `NSTextInputClient` directly, NOT GLFW (Kitty's mistake)
- [x] **TERM**: Use `xterm-crux` with `xterm-` prefix for compatibility
- [x] **Terminfo**: Ship installer, validate with `infocmp` and `vttest`
- [x] **Rendering**: Integer cell coordinates, avoid float rounding (WezTerm #614)
- [x] **Font**: Test ligatures at 8pt/10pt/12pt, validate glyph origin

#### Phase 2: Tabs, Panes, IPC
- [ ] **Status Bar**: Decouple from main rendering loop, max 10 FPS
- [ ] **Tab Bar**: Use native macOS tabs, not custom rendering (WezTerm retro lag)
- [ ] **Memory**: Test spawn/close cycles, profile with Instruments
- [ ] **IPC**: Semver-compatible version check, not exact match

#### Phase 3: Korean/CJK IME, Rich Clipboard
- [x] **IME Preedit**: Overlay on canvas, NEVER send to PTY until confirmed
- [x] **IME Cursor**: Update position on every cursor move via `setMarkedTextSelectedRange`
- [x] **Clipboard**: macOS NSPasteboard general only, test drag-and-drop
- [ ] **CJK Width**: Account for variation selectors (VS-15, VS-16)

#### Phase 4: Markdown Preview, Graphics, Kitty Protocol
- [ ] **Graphics Protocol**: Design for tmux pass-through (Kitty's mistake)
- [ ] **Image Cache**: Max memory limit, eviction policy
- [ ] **Hyperlinks**: Validate OSC 8 URLs, sandbox handling

#### Phase 5: tmux, Claude Code Integration
- [ ] **tmux**: Document custom TERM limitations, auto-deploy terminfo
- [ ] **Shell Integration**: Test on bash/zsh/fish, auto-inject over SSH
- [ ] **Config**: TOML only, no side effects, validate on load

#### Phase 6: Homebrew Distribution
- [ ] **Code Signing**: Set `NSSupportsAutomaticGraphicsSwitching` in Info.plist
- [ ] **Gatekeeper**: Pass macOS notarization (Alacritty failing as of 2026)

### Testing Checklist

| Category | Test Case | WezTerm Bug | Kitty Bug |
|----------|-----------|-------------|-----------|
| **IME** | Japanese Ctrl+H should delete preedit only | #7234 | — |
| **IME** | Preedit should render on active pane only | #2569 | — |
| **IME** | Search overlay needs separate IME context | #5333 | — |
| **Memory** | Spawn/close 100 windows, check RSS | #6116 | — |
| **Memory** | Open 24 hours, should not grow >500MB | #2626 | — |
| **Font** | Ligatures at 8pt/10pt/12pt, no gaps | #6931 | — |
| **Font** | Cursor on ligature shouldn't darken whole glyph | #478 | — |
| **Rendering** | Wide CJK chars, no 1px splits | #614 | — |
| **Clipboard** | Select-to-copy toggle (default off) | — | — |
| **Clipboard** | Test macOS NSPasteboard + drag-and-drop | — | v0.44 |
| **Graphics** | Image deletion should be scoped to region | — | #5081 |
| **Graphics** | Should work in tmux (if possible) | — | ✗ |
| **Emoji** | Variation selector changes width correctly | — | #3998 |
| **Emoji** | Fallback chain deterministic | — | #2821 |
| **Scrollback** | Default 10k, warn >100k | — | — |
| **Scrollback** | Clear should compact buffer | — | — |
| **SSH** | Auto-deploy terminfo on connect | — | #1241 |
| **tmux** | Document TERM limitations | — | FAQ |
| **Config** | Reload should not spawn processes | Docs | — |
| **Config** | Validate on load, provide errors | #7896 | — |
| **Security** | Fuzz escape sequence parser | — | — |
| **Security** | Validate numeric params, max 65535 | — | — |
| **macOS** | Multi-monitor dock/undock | — | #8983 |
| **macOS** | Test on all macOS versions in CI | #6833 | — |
| **macOS** | Auto GPU switching (Info.plist) | #2138 | — |

---

## References

### WezTerm Issues
- [#6833: Freezes on Launch (macOS 15.4)](https://github.com/wezterm/wezterm/issues/6833)
- [#7492: Window Resize on Drag (macOS Tahoe)](https://github.com/wezterm/wezterm/issues/7492)
- [#7271: High GPU Usage (macOS Tahoe)](https://github.com/wezterm/wezterm/issues/7271)
- [#614: Wide CJK Character Splits](https://github.com/wezterm/wezterm/issues/614)
- [#7234: Ctrl+H Japanese IME Bug](https://github.com/wezterm/wezterm/issues/7234)
- [#5333: Search Overlay IME](https://github.com/wezterm/wezterm/issues/5333)
- [#2569: Preedit on All Panes](https://github.com/wezterm/wezterm/issues/2569)
- [#5400: Performance (Windows 11)](https://github.com/wezterm/wezterm/discussions/5400)
- [#790: Low Framerate (macOS 11)](https://github.com/wezterm/wezterm/issues/790)
- [#4788: Status Bar Lag](https://github.com/wezterm/wezterm/issues/4788)
- [#4874: Ligature Rendering](https://github.com/wezterm/wezterm/issues/4874)
- [#478: Cursor Darkens Ligature](https://github.com/wezterm/wezterm/issues/478)
- [#5280: Ligature Performance](https://github.com/wezterm/wezterm/issues/5280)
- [#3558: SSH Rendering](https://github.com/wezterm/wezterm/issues/3558)
- [#7014: SSH Connection Stops](https://github.com/wezterm/wezterm/issues/7014)
- [#7400: Kitty GIF Memory Leak](https://github.com/wezterm/wezterm/issues/7400)
- [#2626: Excessive Memory Growth](https://github.com/wezterm/wezterm/issues/2626)
- [#3815: Hashmap Memory Leaks](https://github.com/wezterm/wezterm/issues/3815)
- [#6116: Thread Leak](https://github.com/wezterm/wezterm/issues/6116)
- [Configuration Files](https://wezterm.org/config/files.html)
- [#7896: Invalid Lua Table Syntax (Claude Code)](https://github.com/anthropics/claude-code/issues/7896)

### Kitty Issues
- [#8983: Crashes on Tahoe Fullscreen](https://github.com/kovidgoyal/kitty/issues/8983)
- [#6997: Random Crashes (M2)](https://github.com/kovidgoyal/kitty/issues/6997)
- [#469: IME Kitten Request](https://github.com/kovidgoyal/kitty/issues/469)
- [Debian Bug #990316: IME Support](https://bugs.debian.org/cgi-bin/bugreport.cgi?bug=990316)
- [#8131: Japanese IME Switching](https://github.com/kovidgoyal/kitty/issues/8131)
- [#910: macOS IME](https://github.com/kovidgoyal/kitty/issues/910)
- [Kitty Keyboard Protocol](https://sw.kovidgoyal.net/kitty/keyboard-protocol/)
- [#5081: Image Deletion](https://github.com/kovidgoyal/kitty/issues/5081)
- [Ghostty #6711: Image ID Replacement](https://github.com/ghostty-org/ghostty/issues/6711)
- [#3163: Graphics Protocol Deletion](https://github.com/kovidgoyal/kitty/issues/3163)
- [#1241: Tmux over SSH](https://github.com/kovidgoyal/kitty/issues/1241)
- [Kitty FAQ](https://sw.kovidgoyal.net/kitty/faq/)
- [#2821: Unicode Emoji](https://github.com/kovidgoyal/kitty/issues/2821)
- [#3998: Variation Selectors](https://github.com/kovidgoyal/kitty/issues/3998)
- [#4765: Shell Integration Escape Codes](https://github.com/kovidgoyal/kitty/issues/4765)
- [#6783: KITTY_SHELL_INTEGRATION](https://github.com/kovidgoyal/kitty/issues/6783)
- [#4599: Shell Integration TMUX](https://github.com/kovidgoyal/kitty/issues/4599)
- [#9026: Wayland Backend Lag](https://github.com/kovidgoyal/kitty/issues/9026)

### Cross-Terminal Issues
- [Copy and Paste in tmux (seanh.cc)](https://www.seanh.cc/2020/12/27/copy-and-paste-in-tmux/)
- [State of the Terminal (gpanders.com)](https://gpanders.com/blog/state-of-the-terminal/)
- [Ghostty Terminfo Workaround (vninja.net)](https://vninja.net/2024/12/28/ghostty-workaround-for-missing-or-unsuitable-terminal-xterm-ghostty/)
- [Text-Terminal-HOWTO: Terminfo](https://tldp.org/HOWTO/Text-Terminal-HOWTO-16.html)
- [ANSI Terminal Security (dgl.cx)](https://dgl.cx/2023/09/ansi-terminal-security)
- [Terminal Emulation Security (hdm.io)](https://hdm.io/writing/termulation.txt)
- [WezTerm #2287: Color Mismatch](https://github.com/wezterm/wezterm/issues/2287)
- [Kitty #214: Subpixel Antialiasing](https://github.com/kovidgoyal/kitty/issues/214)
- [macOS Font Rendering (ahmadawais.com)](https://ahmadawais.com/fix-macos-mojave-font-rendering-issue/)
- [Warp #76: Discrete GPU](https://github.com/warpdotdev/Warp/issues/76)
- [Terminal #13681: IME Preedit](https://github.com/microsoft/terminal/issues/13681)
- [Alacritty #6313: Clear Preedit](https://github.com/alacritty/alacritty/issues/6313)

---

## Resolved Issues — Root Cause & Fix Analysis

> 아래는 WezTerm, Kitty, Rio에서 **해결된** 주요 버그들의 근본 원인과 수정 방법 분석이다.
> 수정 PR과 코드 레벨 교훈을 포함한다.

---

## WezTerm Bugs

### 1. macOS Glyph Panic on Code-Signing (#6883)

**Bug Description**: WezTerm crashes with `NSInternalInconsistencyException` when displaying placeholder glyphs for missing fonts on macOS.

**Root Cause**:
- Commit 9abf85c migrated macOS toast notifications from deprecated notification center to `UNUserNotificationCenter`
- `UNUserNotificationCenter` requires proper code-signing to function
- Unsigned builds (including Nixpkgs) attempt to access the notification center when displaying missing-glyph warnings
- Stack trace shows: `bundleProxyForCurrentProcess is nil: mainBundle.bundleURL`

**Fix**:
- Identified as duplicate of #6731
- Problem was in notification center implementation, not glyph-handling code
- Requires proper Apple Developer account code-signing for distribution builds

**Lesson for Crux**:
- **Code-signing is mandatory for macOS system APIs**: Don't use `UNUserNotificationCenter` in unsigned builds
- **Graceful degradation**: Fall back to alternative notification methods (NSUserNotification) for unsigned/dev builds
- **Warning visibility**: Consider in-terminal warnings instead of system notifications to avoid this class of bugs
- **Test unsigned builds**: Nixpkgs and homebrew formulas often build unsigned; test this scenario

**References**:
- [Issue #6883](https://github.com/wezterm/wezterm/issues/6883)
- [Issue #6731](https://github.com/wezterm/wezterm/issues/6731)

---

### 2. Thread and Memory Leak on Window Spawn/Terminate (#6116)

**Bug Description**: Each window spawn/terminate cycle leaks ~5 MB memory and 2 threads when reusing a single GUI instance.

**Reproduction**:
```bash
for i in $(seq 1 200); do wezterm start -- sh -c 'exit 0'; done
# RSS grows from ~300 MB to ~1.2 GB
# Threads increase from 17 to 417
```

**Root Cause**:
- Thread names indicate graphics driver threads, not WezTerm threads
- Wayland surface cleanup issues: "queue destroyed while proxies still attached"
- Graphics driver interaction with Wayland protocol handling
- Potential connection to shader compilation in Glium graphics library

**Fix**:
- Issue closed after reporter disabled Wayland (`enable_wayland = false`)
- Problem stemmed from dependency layers (graphics drivers, Wayland) not WezTerm core

**Lesson for Crux**:
- **Metal-only simplifies**: macOS Metal backend avoids multi-backend complexity that leads to leaks
- **Window cleanup**: Ensure GPUI window disposal properly releases GPU resources
- **Profile with instruments**: Use Xcode Instruments to detect thread/memory leaks in GPU code
- **Test spawn/close cycles**: Automate testing of rapid window creation/destruction
- **GPU context lifecycle**: Verify Metal device/command queue cleanup on window close

**References**:
- [Issue #6116](https://github.com/wezterm/wezterm/issues/6116)

---

### 3. Japanese IME Swallows Ctrl-O on macOS (#2725)

**Bug Description**: Ctrl-O keystrokes consumed by Japanese IME, never reaching the application (e.g., Neovim jump list navigation).

**Observations**:
- WezTerm logs show `key: Char('o'), modifiers: CTRL` detected
- Logs show "send to pane UP key=Char('o') mods=CTRL"
- Despite logging the send, character never reaches PTY
- Works correctly in iTerm2 and macOS Terminal

**Root Cause**:
- IME interaction with specific keyboard layouts on macOS
- NSTextInputClient protocol implementation issue where IME intercepts control keys

**Fix Status**:
- Issue marked CLOSED/COMPLETED
- Specific fix details not documented in visible discussion
- Maintainer investigated keyboard layout configuration

**Lesson for Crux**:
- **IME must not intercept control keys**: Explicitly bypass IME for Ctrl/Cmd combinations (see `research/platform/ime-clipboard.md`)
- **NSTextInputClient protocol**: Override `hasMarkedText` to return false during control key handling
- **Test matrix**: Test IME with Ctrl combinations across Japanese, Korean, Chinese input methods
- **Log vs reality**: Logging "sent to PTY" doesn't prove bytes reached the PTY; verify with PTY read
- **Bypass IME for shortcuts**: Use `interpretKeyEvents:` selectively, not for all keystrokes

**References**:
- [Issue #2725](https://github.com/wezterm/wezterm/issues/2725)

---

### 4. Memory "Leaks" from HashMap Caching (#3815)

**Bug Description**: Profiler reports ~2000 memory "leaks" within 22 seconds of launch on macOS.

**Root Cause Analysis**:
- NOT actual leaks, but legitimate long-lived allocations
- `hashbrown` HashMap allocations for cached data retained throughout process lifetime
- Intentional caching for performance optimization
- Modern fonts with bitmap data occupy significant RAM
- Real problem: Memory growth from repeated spawn/close (separate from initial allocations)

**Developer Response**:
- Maintainer (Wez Furlong) declined prioritization
- Explained trade-off: caching reduces CPU overhead but increases memory
- Suggested users explore cache tuning options
- Closed as "not planned" due to lack of sponsorship funding

**Lesson for Crux**:
- **Distinguish leaks from retention**: Use Instruments to identify true leaks vs intentional caching
- **Bounded caches**: Implement LRU eviction for font/glyph caches with configurable max size
- **Memory budgets**: Define acceptable memory usage per window/tab
- **Profiler interpretation**: macOS leaks tool flags retained memory; verify actual leak with heap growth over time
- **Trade-offs documentation**: Document memory vs CPU performance trade-offs for users

**References**:
- [Issue #3815](https://github.com/wezterm/wezterm/issues/3815)
- [Issue #2626](https://github.com/wezterm/wezterm/issues/2626)
- [Issue #2453](https://github.com/wezterm/wezterm/issues/2453)

---

### 5. Font Shaping Performance Regression (#5280)

**Bug Description**: Significant unresponsiveness and lag when using fonts with ligatures, even when ligatures disabled.

**Profiling Findings**:
- 84.3% CPU time in `wezterm_font::shaper::harfbuzz::HarfbuzzShaper::do_shape` with JetBrainsMono
- Call chain: `do_shape → call_metric → FT_Load_Glyph → tt_glyph_load → load_truetype_glyph → TT_Hint_Glyph → TT_Runins`
- Hack Nerd Font (no ligatures): only 24.1% CPU in same function
- Problem: Unicode symbols (U+22C5) not in primary font triggered fallback to Menlo

**Root Cause**:
- Font fallback glyphs underwent repeated reshaping without caching
- Aggressive font unloading mechanism removed "unused" fallback fonts prematurely
- Caused repeated font reloading and reshaping cycles

**Fix**:
- Disabled premature font disposal for fallback fonts
- Prevented font unloading mechanism from evicting actively-used fallbacks
- Substantially improved performance by caching fallback font shapes

**Lesson for Crux**:
- **Cache fallback shapes**: Never re-shape the same glyph; cache by (font_id, codepoint, size)
- **LRU for fallbacks**: Keep recently-used fallback fonts in memory
- **Profile with realistic data**: Test with Unicode symbols requiring fallback (emoji, math symbols, CJK)
- **FreeType hinting cost**: Hinting (TT_Hint_Glyph) is expensive; consider unhinted rendering for fallbacks
- **Ligature independence**: Ensure fallback performance doesn't degrade even when primary font has ligatures
- **GPUI font system**: Verify GPUI's font API caches shaped runs; supplement if needed

**References**:
- [Issue #5280](https://github.com/wezterm/wezterm/issues/5280)

---

### 6. IME and Dead Key Interaction (#1372)

**Bug Description**: Bad interaction between `send_composed_key_when_left_alt_is_pressed`, IME, and dead keys on macOS.

**Observations**:
- Dead keys (e.g., Spanish ISO, U.S. International: ´ + a → á) inserted wrong character
- Option key with IME active produced unexpected characters
- Dead key expansion failed when `use_ime = false`

**Fix**:
- Marked as fixed in nightly builds
- WezTerm now performs dead-key expansion when `use_ime = false`
- Dead keys treated as composition effects
- Configuration options: `send_composed_key_when_left_alt_is_pressed` and `use_ime`

**Lesson for Crux**:
- **Dead key handling**: Implement explicit dead key state machine separate from IME
- **Option key routing**: macOS Option generates different keycodes; handle separately from IME composition
- **Test international layouts**: Spanish, French, German, U.S. International keyboards for dead key coverage
- **NSTextInputClient**: Override `validAttributesForMarkedText` to control IME vs dead key routing
- **Configuration surface**: Let users choose IME vs dead key precedence for Option key

**References**:
- [Issue #1372](https://github.com/wezterm/wezterm/issues/1372)
- [Issue #300](https://github.com/wezterm/wezterm/issues/300)
- [Issue #5468](https://github.com/wezterm/wezterm/issues/5468)

---

### 7. Unicode Normalization Ignores Config (#3732)

**Bug Description**: WezTerm always normalizes strings to NFC form regardless of `normalize_output_to_unicode_nfc = false` setting.

**Impact**:
- Editors like Neovim need strings displayed as-is
- Cannot preserve NFD (decomposed) strings
- Particularly affects CJK and Hangul characters in Korean filenames
- macOS uses NFD for filesystem, breaking filename display

**Root Cause**:
- Configuration option documented but not actually preventing normalization
- Normalization happens unconditionally in output pipeline

**Lesson for Crux**:
- **Preserve byte fidelity**: Don't normalize VT output unless explicitly configured
- **macOS filename display**: macOS filesystem returns NFD; preserve or normalize consistently
- **Configuration testing**: Verify config options actually change behavior (unit test each flag)
- **Unicode test suite**: Test NFC, NFD, NFKC, NFKD with Korean/Japanese filenames
- **Display vs filesystem**: Document when normalization happens (display vs filename comparison)

**References**:
- [Issue #3732](https://github.com/wezterm/wezterm/issues/3732)
- [normalize_output_to_unicode_nfc config](https://wezterm.org/config/lua/config/normalize_output_to_unicode_nfc.html)

---

### 8. Scrollback Performance with Large Buffers (#1342, #6003)

**Bug Description**: Large scrollback buffers (hundreds of thousands of lines) cause memory allocation failures and performance degradation.

**Improvements**:
- Internal scrollback datastructure improvements reduced per-cell overhead by up to 40x
- Depends on line composition (varied attributes/image attachments = more overhead)

**Limits**:
- Upper bound: 999,999,999 for `scrollback_lines`
- Larger values pressure system RAM with many long-lived tabs

**Lesson for Crux**:
- **Bounded scrollback**: Default to reasonable limit (10k-50k lines), allow configuration
- **Compact representation**: Use dense storage for cells (bit-packing attributes, string deduplication)
- **Lazy loading**: Consider disk-backed scrollback for very large buffers (à la tmux)
- **Memory monitoring**: Warn user if scrollback exceeds memory budget
- **Damage tracking**: Only render visible portion of scrollback, not entire buffer
- **GPUI canvas**: Verify efficient rendering of large scrollback regions with virtualization

**References**:
- [Issue #1342](https://github.com/wezterm/wezterm/issues/1342)
- [Issue #6003](https://github.com/wezterm/wezterm/issues/6003)
- [Issue #3879](https://github.com/wezterm/wezterm/issues/3879)
- [Scrollback documentation](https://wezterm.org/scrollback.html)

---

### 9. Multiplexer Domain Lockup on Large Output (#2048)

**Bug Description**: When any buffer in a WezTerm multiplexer session receives large output, entire domain/session slows or fully locks with no recovery except force-kill.

**Root Cause**:
- Large stdout from PTY overwhelms multiplexer protocol
- Backpressure not properly handled between mux server and clients
- Domain-wide lock contention when one pane floods

**Lesson for Crux**:
- **Per-pane buffering**: Isolate pane buffers; one pane's flood shouldn't block others
- **Backpressure**: Implement flow control between PTY read and renderer
- **IPC batching**: Batch updates over Unix socket to avoid per-line overhead (see `research/integration/ipc-protocol-design.md`)
- **Rate limiting**: Throttle VT parser when output exceeds threshold (e.g., `cat large.log`)
- **Async rendering**: Decouple PTY reads from rendering; queue updates in background
- **GPUI async**: Use GPUI's async task spawning to handle PTY I/O without blocking UI thread

**References**:
- [Issue #2048](https://github.com/wezterm/wezterm/issues/2048)
- [Multiplexing documentation](https://wezterm.org/multiplexing.html)

---

### 10. Closing Panes Stops Input (#2304)

**Bug Description**: Rapidly closing panes on macOS causes WezTerm to stop accepting keyboard input permanently.

**Reproduction**:
- Particularly in workflows like Kakoune editor with frequent pane creation/destruction
- Input completely frozen; requires force-quit

**Root Cause**:
- macOS event loop interaction with pane cleanup
- Focus management bug when removing NSView from hierarchy
- Potential race between pane destruction and input routing

**Lesson for Crux**:
- **Focus restoration**: When closing pane, explicitly transfer focus to remaining pane
- **Event loop safety**: Defer NSView removal until after event processing completes
- **Input routing**: Verify keyboard responder chain after pane removal
- **GPUI focus management**: Test GPUI's focus handling when removing views dynamically
- **Async cleanup**: Delay resource cleanup until after current event cycle
- **Test rapid operations**: Automate pane open/close cycles to catch race conditions

**References**:
- [Issue #2304](https://github.com/wezterm/wezterm/issues/2304)

---

### 11. macOS Window Freeze on Launch (#6833, #452)

**Bug Description**: WezTerm freezes on launch with 100% CPU usage on macOS 15.4. Also: hangs for several seconds after consecutively opening and closing.

**Observations**:
- Completely unusable on some macOS versions
- 100% CPU usage during freeze
- Rapid open/close cycles trigger multi-second hangs

**Root Cause**:
- macOS version-specific interaction with window manager
- Possible Metal/GPU initialization deadlock
- Event loop blocking during window setup

**Lesson for Crux**:
- **Async initialization**: Perform GPU/Metal setup off main thread if possible
- **Timeout guards**: Detect and recover from initialization hangs
- **macOS version testing**: Test on beta macOS versions early (15.x betas available now)
- **Metal device creation**: Profile Metal device/queue creation time; defer if expensive
- **GPUI initialization**: Verify GPUI's window creation doesn't block on macOS 15+
- **Launch watchdog**: Implement timeout for window appearance; alert user if stuck

**References**:
- [Issue #6833](https://github.com/wezterm/wezterm/issues/6833)
- [Issue #452](https://github.com/wezterm/wezterm/issues/452)
- [Issue #7051](https://github.com/wezterm/wezterm/issues/7051)

---

## Kitty Bugs

### 12. tmux Ctrl-B Triggers CSI u on Old tmux (#3722)

**Bug Description**: SSH to CentOS 7 with old tmux (1.8), pressing Ctrl-B shows `8;5u` instead of entering tmux control mode.

**Root Cause**:
- tmux 1.8 has buggy implementation that emits CSI u escape sequence
- Kitty 0.20.0 introduced extended keyboard protocol (CSI u) support
- Old tmux inadvertently activates Kitty's extended protocol through its own escape bug
- Creates conflict where Ctrl-B is interpreted as keyboard protocol instead of tmux prefix

**Solutions**:
- **Modern tmux**: Upgrade to tmux 2.7+ (no longer emits buggy CSI u)
- **Workaround for legacy**: `printf '\x1b[<u\x1b[<u'` to reset keyboard protocol
- **Within tmux**: `printf '\ePtmux;\e\e[<u\e\\'` (wraps escape for tmux pass-through)
- **Auto-fix**: Add to `.bashrc` in tmux sessions

**Lesson for Crux**:
- **Protocol opt-in**: Extended keyboard protocol should be explicit opt-in (CSI >1u), not auto-activated
- **Escape sequence audit**: Test with old tmux, screen, legacy apps to avoid accidental protocol activation
- **Protocol reset**: Provide escape sequence to reset to legacy mode (CSI <u)
- **Compatibility matrix**: Document compatibility with tmux 1.8, 2.x, 3.x versions
- **In-band protocol**: Careful with in-band protocol negotiation; prefer out-of-band (terminfo)

**References**:
- [Issue #3722](https://github.com/kovidgoyal/kitty/issues/3722)
- [Keyboard Protocol documentation](https://sw.kovidgoyal.net/kitty/keyboard-protocol/)

---

### 13. OSC 52 Clipboard Appends Instead of Replacing (#995)

**Bug Description**: Running OSC 52 escape sequence twice results in duplicated clipboard content ("testtest" instead of "test").

**Reproduction**:
```bash
printf "\033]52;c;$(printf "%s" "test" | base64)\a"
printf "\033]52;c;$(printf "%s" "test" | base64)\a"
# Clipboard: "testtest" (WRONG)
```

**Root Cause**:
- `window.py` line 403 used compound assignment (`+=`) to append clipboard data
- Should have used simple assignment (`=`) to replace

**Fix**:
- Initially: Changed `+=` to `=` for standard behavior
- Later: Added "no-append" option to `clipboard_control` config
- Maintains backward compatibility for users who preferred appending

**Lesson for Crux**:
- **OSC 52 semantics**: Each OSC 52 should SET clipboard, not APPEND (per spec)
- **Test escape sequences**: Verify each OSC 52 invocation replaces clipboard
- **macOS NSPasteboard**: Ensure `setPasteboardItems:` replaces, not appends
- **Configuration**: Consider config for append behavior, but default to spec-compliant replace
- **Security**: OSC 52 write-only by default; prompt for read to prevent clipboard snooping

**References**:
- [Issue #995](https://github.com/kovidgoyal/kitty/issues/995)
- [Clipboard documentation](https://sw.kovidgoyal.net/kitty/clipboard/)

---

### 14. Cascadia Code Ligature Rendering Artifacts (#3504)

**Bug Description**: Cascadia Code v2102.03+ ligatures render with missing characters, extra spacing, truncation, and occlusion.

**Example**: `>>--->` shows gaps; repeated `#` symbols display with visible breaks.

**Root Cause**:
- Cascadia Code changed from "backwards looking" ligatures (negative left side bearing on final char) to "forwards looking" (first char contains ligature)
- Kitty's glyph positioning logic didn't handle this shift
- Ligatures rendered shorter than constituent characters (which include inter-char spacing)
- Created mismatch: ligature visual width < sum of cell widths

**Fix**:
- Multiple commits to adjust glyph positioning calculations
- Handle variable-width ligature substitutions properly
- Resolved by version 0.21.2

**Lesson for Crux**:
- **Ligature testing**: Test with multiple fonts (Cascadia, JetBrains, Fira Code, Iosevka)
- **Glyph metrics**: Use actual glyph advance width, not assumed cell-width multiplication
- **HarfBuzz shaping**: Trust HarfBuzz cluster mapping for glyph positioning
- **Font design evolution**: Fonts change bearing models; don't hardcode assumptions
- **Visual testing**: Automated screenshot comparison for ligature rendering (prevent regressions)
- **GPUI text layout**: Verify GPUI's `ShapedRun` correctly positions multi-cell ligatures

**References**:
- [Issue #3504](https://github.com/kovidgoyal/kitty/issues/3504)
- [Issue #3313](https://github.com/kovidgoyal/kitty/issues/3313)

---

### 15. Emoji Variation Selector (VS16) Rendering Regression (#6774, #8318)

**Bug Description**: Emoji with U+FE0F variation selector failed to render on macOS. Also: regression in 0.40.0 broke VS16 handling when character flows to next line.

**Root Cause**:
- macOS CoreText handles variation selectors specially
- U+FE0F (VS16) hints renderer to use emoji variant
- CoreText internally switches to Color Emoji font when detecting VS16
- Kitty's font-matching logic conflicted with CoreText's internal switch
- Line-wrapping code didn't account for VS16 as zero-width modifier

**Fix**:
- Version 0.38.1 fixed VS16 rendering
- Version 0.40.0 introduced line-wrap regression (later fixed)
- Proper handling of variation selectors as combining characters

**Lesson for Crux**:
- **CoreText emoji**: Let CoreText handle emoji font fallback automatically
- **Zero-width modifiers**: VS16, skin tone modifiers (U+1F3FB-U+1F3FF) don't advance cursor
- **Grapheme clusters**: Treat emoji + VS16 + skin tone as single grapheme for line breaking
- **Test suite**: Include emoji with/without VS16, skin tone sequences, ZWJ sequences
- **Unicode segmentation**: Use unicode-segmentation crate for grapheme cluster boundaries
- **GPUI text shaping**: Verify GPUI's text layout handles variation selectors correctly

**References**:
- [Issue #6774](https://github.com/kovidgoyal/kitty/issues/6774)
- [Issue #8318](https://github.com/kovidgoyal/kitty/issues/8318)
- [Issue #2139](https://github.com/kovidgoyal/kitty/issues/2139)
- [Changelog](https://sw.kovidgoyal.net/kitty/changelog/)

---

### 16. CJK Ambiguous Width Characters (#6560, #8265)

**Bug Description**: Incorrect handling of CJK ambiguous width characters (UAX #11 East Asian Width property).

**Root Cause**:
- Unicode defines some characters as "Ambiguous" width (1 or 2 cells depending on context)
- Legacy East Asian encodings render them as fullwidth
- Modern Unicode context renders them as halfwidth
- Lack of definitive method to determine correct width

**Solution**:
- Kitty version 0.40+ introduced text-sizing protocol
- Allows programs to control character cell width explicitly
- Solves long-standing ambiguous width problem robustly

**Lesson for Crux**:
- **Ambiguous width**: Default ambiguous characters to 1 cell (halfwidth) unless in CJK locale
- **Configuration**: Provide `wcwidth` configuration for ambiguous characters
- **Text-sizing protocol**: Consider implementing Kitty's text-sizing protocol for programmatic control
- **Locale detection**: Use `LANG`/`LC_CTYPE` environment variables to infer CJK context
- **Test cases**: Test with box-drawing, Greek, Cyrillic characters marked ambiguous
- **wcwidth source**: Use up-to-date Unicode data; consider `unicode-width` crate

**References**:
- [Issue #6560](https://github.com/kovidgoyal/kitty/issues/6560)
- [Issue #8265](https://github.com/kovidgoyal/kitty/issues/8265)
- [Text-sizing protocol](https://sw.kovidgoyal.net/kitty/text-sizing-protocol/)

---

### 17. Image Protocol Background Flashing (#changelog)

**Bug Description**: Background image flashes when closing a tab in Kitty.

**Additional Issue**: Background opacity <1.0 causes flicker on startup with Wayland compositors supporting single pixel buffers.

**Fix**:
- Fixed background image flashing on tab close
- Improved Wayland support for transparency to prevent startup flicker

**Lesson for Crux**:
- **GPU resource cleanup**: Ensure textures/framebuffers released before window/tab destruction
- **Transparency compositing**: macOS compositor handles alpha differently than Wayland
- **Metal layer opacity**: Set `CAMetalLayer.isOpaque` correctly based on background alpha
- **GPUI transparency**: Verify GPUI's window transparency doesn't cause flicker
- **Frame synchronization**: Use `CAMetalLayer` `presentsWithTransaction` to prevent tearing
- **Testing**: Test tab close with images displayed to catch resource cleanup bugs

**References**:
- [Kitty changelog](https://sw.kovidgoyal.net/kitty/changelog/)
- [Issue #7987](https://github.com/kovidgoyal/kitty/issues/7987)

---

### 18. Keyboard Protocol Modifier Key Disambiguation (#8211)

**Bug Description**: Kitty keyboard protocol's "disambiguate escape codes" feature disabled VINTR/VSTOP signals, breaking Ctrl-C cancellation in some contexts.

**Root Cause**:
- Disambiguate flag means Ctrl-C delivered as CSI u escape instead of generating SIGINT
- Applications expecting signal-based cancellation broke
- Trade-off between disambiguation and traditional signal handling

**Impact**:
- fish shell reported Ctrl-C no longer interrupting commands
- Programs relying on SIGINT signal handling affected

**Lesson for Crux**:
- **Protocol opt-in**: Extended keyboard protocol must be application opt-in, not default
- **Signal preservation**: Ctrl-C, Ctrl-Z should generate signals by default
- **Escape sequences**: Only send CSI u for Ctrl-C when app explicitly requests protocol
- **Termios respect**: Honor termios VINTR, VSUSP, VQUIT character mappings
- **Testing**: Test Ctrl-C in vim, emacs, bash, zsh, fish to verify signal delivery
- **PTY semantics**: Don't bypass PTY driver's built-in signal generation

**References**:
- [Issue #8211](https://github.com/kovidgoyal/kitty/issues/8211)
- [fish-shell issue #10864](https://github.com/fish-shell/fish-shell/issues/10864)
- [Keyboard protocol spec](https://sw.kovidgoyal.net/kitty/keyboard-protocol/)

---

## Rio Bugs

### 19. CJK Characters Not Displayed on Line Break (#1013)

**Bug Description**: CJK characters between special characters and end-of-line don't display when odd spaces remain.

**Example**: Terminal with odd number of cells remaining → CJK character (2 cells wide) doesn't fit → fails to display instead of wrapping.

**Root Cause**:
- Character width calculation mismatch at line boundaries
- CJK characters occupy 2 display columns
- Odd number of remaining spaces (e.g., 1 cell) can't fit 2-cell character
- Renderer failed to wrap; instead dropped character

**Status**: Marked "In Progress" for Rio 0.2.x

**Lesson for Crux**:
- **Automatic wrapping**: When 2-cell CJK character doesn't fit, insert padding and wrap to next line
- **wcwidth checking**: Before rendering, verify cell width fits remaining line space
- **Padding cells**: Insert empty cell to fill odd space, then wrap CJK character
- **Alacritty compatibility**: Check how `alacritty_terminal` handles this (Rio uses same VT parser)
- **Test cases**: Lines ending with: `」` (2-cell) + space, emoji (2-cell), Korean Hangul
- **GPUI line breaking**: Verify GPUI's text layout wraps wide characters correctly

**References**:
- [Issue #1013](https://github.com/raphamorim/rio/issues/1013)

---

### 20. Fallback Font for Unicode Characters (#266)

**Bug Description**: FiraCode Nerd Font lacks Japanese character support → Japanese text (こんにちは、源氏です) renders as unrecognized glyphs.

**Platform**: Windows 10 with WSL2 (couldn't reproduce on macOS, suggesting platform-specific font handling).

**Solution**:
- Added configuration-based fallback fonts: `[fonts] extra = ["font1", "font2"]`
- Implemented system-level fallback font support for Windows
- Before/after comparison showed Japanese rendering correctly after fix

**Status**: Fixed in main branch

**Lesson for Crux**:
- **Fallback font chain**: Primary font → user-configured extras → system fallbacks
- **Platform differences**: macOS CoreText handles fallbacks automatically; Windows/Linux need explicit list
- **Configuration**: Allow `extra_fonts = ["Noto Sans CJK JP", "Apple Color Emoji"]`
- **Font matching**: Use CoreText `CTFontCreateForString` on macOS for automatic fallback
- **GPUI font system**: Verify GPUI's font API supports fallback chains
- **Testing**: Test with emoji, Japanese, Korean, Arabic, Hebrew to verify fallback coverage
- **Performance**: Cache fallback font lookups; don't re-query per character

**References**:
- [Issue #266](https://github.com/raphamorim/rio/issues/266)

---

## Cross-Cutting Lessons

### Memory Management

1. **Distinguish leaks from retention**: Use Instruments to identify true leaks vs intentional caching
2. **Bounded caches**: Implement LRU eviction for font/glyph caches
3. **Profile spawn/close cycles**: Automate testing of window creation/destruction
4. **GPU resource cleanup**: Ensure Metal textures/buffers released properly

### macOS Platform Integration

5. **Code-signing mandatory**: System APIs like `UNUserNotificationCenter` require signing
6. **CoreText integration**: Let CoreText handle emoji and fallback fonts automatically
7. **NSTextInputClient**: Carefully implement IME protocol; bypass for control keys
8. **Metal initialization**: Profile device/queue creation; consider async initialization

### Font Rendering

9. **Cache fallback shapes**: Never re-shape same glyph; cache by (font, codepoint, size)
10. **LRU for fallbacks**: Keep recently-used fallback fonts in memory
11. **Ligature positioning**: Trust HarfBuzz cluster mapping; test multiple fonts
12. **Variation selectors**: Handle VS16 and skin tone modifiers as zero-width

### IME and Keyboard

13. **IME must not intercept control keys**: Explicitly bypass IME for Ctrl/Cmd combinations
14. **Dead key state machine**: Implement separate from IME (Option key, international layouts)
15. **Test matrix**: Japanese, Korean, Chinese IME with Ctrl combinations
16. **Signal preservation**: Ctrl-C should generate SIGINT by default, not escape sequence

### Unicode and Text

17. **Preserve byte fidelity**: Don't normalize VT output unless configured
18. **Ambiguous width**: Default to 1 cell; provide configuration for CJK context
19. **Grapheme clusters**: Use unicode-segmentation for proper emoji/CJK boundaries
20. **wcwidth accuracy**: Use up-to-date Unicode data for character width

### Performance

21. **Backpressure**: Implement flow control between PTY read and renderer
22. **Damage tracking**: Only render changed cells, not entire buffer
23. **Async rendering**: Decouple PTY I/O from UI thread with GPUI async tasks
24. **Scrollback bounds**: Default to reasonable limit (10k-50k lines)

### Protocol Compatibility

25. **Protocol opt-in**: Extended features should be explicit opt-in, not auto-activated
26. **Escape sequence audit**: Test with old tmux, screen, legacy apps
27. **OSC 52 semantics**: Each invocation should SET, not APPEND (per spec)
28. **terminfo correctness**: `xterm-crux` prefix critical for compatibility

### Testing Strategy

29. **Platform coverage**: Test on macOS 13, 14, 15 (including betas)
30. **Visual regression**: Automated screenshot comparison for ligatures, emoji
31. **International keyboards**: Spanish, French, German, U.S. International, Korean, Japanese
32. **Rapid operations**: Automate window/pane/tab open/close cycles
33. **Memory monitoring**: Long-running sessions with Instruments to detect leaks

### GPUI-Specific

34. **Window lifecycle**: Verify GPUI window disposal releases GPU resources
35. **Focus management**: Test GPUI's focus handling when removing views
36. **Text layout**: Verify ShapedRun correctly positions multi-cell ligatures
37. **Transparency**: Test GPUI window transparency doesn't cause flicker
38. **Async tasks**: Use GPUI's task spawning for PTY I/O without blocking UI

---

## Additional Resources

### WezTerm
- [WezTerm Issues](https://github.com/wezterm/wezterm/issues)
- [WezTerm Changelog](https://wezterm.org/changelog.html)
- [Font Rendering](https://github.com/wezterm/wezterm/issues/5280)
- [Memory Leaks](https://github.com/wezterm/wezterm/issues/3815)
- [IME Issues](https://github.com/wezterm/wezterm/issues/2725)

### Kitty
- [Kitty Issues](https://github.com/kovidgoyal/kitty/issues)
- [Kitty Changelog](https://sw.kovidgoyal.net/kitty/changelog/)
- [Keyboard Protocol](https://sw.kovidgoyal.net/kitty/keyboard-protocol/)
- [Graphics Protocol](https://sw.kovidgoyal.net/kitty/graphics-protocol/)
- [tmux Compatibility](https://github.com/kovidgoyal/kitty/issues/3722)

### Rio
- [Rio Issues](https://github.com/raphamorim/rio/issues)
- [Rio Changelog](https://rioterm.com/changelog)
- [CJK Rendering](https://github.com/raphamorim/rio/issues/1013)
- [Font Fallback](https://github.com/raphamorim/rio/issues/266)

---

## Summary Statistics

**Total Issues Analyzed**: 20
- **WezTerm**: 11 issues (memory leaks, font shaping, IME, scrollback, multiplexer)
- **Kitty**: 7 issues (keyboard protocol, clipboard, ligatures, emoji, CJK width)
- **Rio**: 2 issues (CJK rendering, font fallback)

**Categories**:
- **Font Rendering**: 6 issues
- **Memory/Performance**: 4 issues
- **IME/Keyboard**: 5 issues
- **Platform (macOS)**: 4 issues
- **Unicode/Text**: 3 issues
- **Protocol/Compatibility**: 3 issues

**Key Takeaway for Crux**: The most critical lessons are:
1. **macOS IME**: Never let IME intercept control keys (research/platform/ime-clipboard.md)
2. **Font fallback caching**: Biggest performance win for mixed content
3. **Code-signing**: Required for system APIs; test unsigned builds
4. **Unicode handling**: Preserve byte fidelity, support variation selectors
5. **GPUI integration**: Verify focus, text layout, and GPU resource cleanup

---

**END OF DOCUMENT**
