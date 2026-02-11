---
title: Terminal Mouse Reporting Protocols
description: Complete reference for xterm mouse tracking modes, encoding formats, button/modifier encoding, and implementation patterns
phase: 1-2
topics: [mouse, input, xterm, SGR, alacritty_terminal, GPUI]
related: [terminal-emulation.md, terminal-architecture.md, keymapping.md]
---

# Terminal Mouse Reporting Protocols

## Overview

Terminal mouse reporting allows TUI applications (vim, tmux, htop, etc.) to receive mouse events from the terminal emulator. The terminal intercepts mouse clicks, motion, and scroll events and encodes them as escape sequences written to the application's stdin via the PTY.

The system has two orthogonal dimensions:
1. **Tracking modes** (which events to report): controlled by DEC private modes 9, 1000, 1002, 1003
2. **Encoding formats** (how to encode the reports): controlled by modes 1005, 1006, 1015, 1016

Sources: [XTerm Control Sequences](https://invisible-island.net/xterm/ctlseqs/ctlseqs.html), [XFree86 ctlseqs](https://www.xfree86.org/current/ctlseqs.html)

---

## 1. Mouse Tracking Modes (DEC Private Modes)

Applications enable/disable tracking modes via DECSET/DECRST:
- **Enable**: `CSI ? Ps h` (e.g., `\x1b[?1000h`)
- **Disable**: `CSI ? Ps l` (e.g., `\x1b[?1000l`)

### Mode 9 — X10 Compatibility

| Property | Value |
|----------|-------|
| DECSET | `CSI ? 9 h` |
| DECRST | `CSI ? 9 l` |
| Reports | Button press only (no release, no motion) |
| Modifiers | Not encoded |
| Use case | Legacy X10 compatibility; rarely used today |

### Mode 1000 — Normal Tracking (VT200 Mouse)

| Property | Value |
|----------|-------|
| DECSET | `CSI ? 1000 h` |
| DECRST | `CSI ? 1000 l` |
| Reports | Button press + button release |
| Modifiers | Encoded in Cb byte |
| Use case | Standard mouse support (vim `set mouse=a`) |

### Mode 1001 — Hilite Tracking

Rarely used. Sends highlight events. Not implemented by most modern terminals. Ignore for Crux.

### Mode 1002 — Button-Event Tracking (Cell Motion)

| Property | Value |
|----------|-------|
| DECSET | `CSI ? 1002 h` |
| DECRST | `CSI ? 1002 l` |
| Reports | Press + release + motion **while any button is pressed** (drag) |
| Motion filter | Only reports when mouse moves to a different cell |
| Modifiers | Encoded in Cb byte |
| Use case | Drag-aware TUI apps |

### Mode 1003 — Any-Event Tracking (All Motion)

| Property | Value |
|----------|-------|
| DECSET | `CSI ? 1003 h` |
| DECRST | `CSI ? 1003 l` |
| Reports | Press + release + **all** motion (even without button pressed) |
| Motion filter | Only reports when mouse moves to a different cell |
| Modifiers | Encoded in Cb byte |
| Use case | tmux, applications needing hover/pointer tracking |
| Warning | High event volume; implement cell-change deduplication |

### Mode 1004 — Focus In/Out Events

| Property | Value |
|----------|-------|
| DECSET | `CSI ? 1004 h` |
| DECRST | `CSI ? 1004 l` |
| Focus gained | `CSI I` (`\x1b[I`) |
| Focus lost | `CSI O` (`\x1b[O`) |
| Independent | Works independently of mouse tracking modes |
| Use case | vim/neovim focus-dependent refresh, tmux pane awareness |

### Mode Mutual Exclusivity

Modes 9, 1000, 1001, 1002, 1003 are **mutually exclusive** for tracking behavior. If more than one is enabled, the most recently enabled mode takes precedence and implicitly disables the previous one.

Mode 1004 (focus) operates **independently** and can be combined with any tracking mode.

### alacritty_terminal TermMode Mapping

| DEC Mode | TermMode Flag | Bit |
|----------|---------------|-----|
| 1000 | `MOUSE_REPORT_CLICK` | `1 << 3` |
| 1002 | `MOUSE_DRAG` | `1 << 13` |
| 1003 | `MOUSE_MOTION` | `1 << 6` |
| 1004 | `FOCUS_IN_OUT` | (separate) |
| 1006 | `SGR_MOUSE` | `1 << 5` |
| 1005 | `UTF8_MOUSE` | `1 << 14` |

The composite flag `MOUSE_MODE` = `MOUSE_REPORT_CLICK | MOUSE_DRAG | MOUSE_MOTION`.

Source: [alacritty_terminal TermMode](https://docs.rs/alacritty_terminal/0.25.0/alacritty_terminal/term/struct.TermMode.html)

---

## 2. Mouse Encoding Formats

### Default/Normal Encoding (X10/X11)

**Format**: `CSI M Cb Cx Cy`

Where each of Cb, Cx, Cy is a **single byte** = value + 32 (ASCII space offset).

```
\x1b [ M <button+32> <column+32+1> <row+32+1>
```

- Coordinates are 1-based (upper-left = 1,1)
- Byte value = coordinate + 32 + 1 (so minimum byte = 33 = `!`)
- **Maximum coordinate**: 223 (byte value 255). Cannot report beyond column/row 223.
- Button release: Cb low bits = 3 (cannot distinguish which button was released)

### UTF-8 Mode (1005)

**Enable**: `CSI ? 1005 h`

Same as normal encoding but Cx and Cy use UTF-8 encoding when the coordinate exceeds 95:
- Coordinates 0-94: single byte (value + 32 + 1)
- Coordinates 95-2015: two-byte UTF-8 sequence
  - First byte: `0xC0 + pos / 64`
  - Second byte: `0x80 + (pos & 63)`

**Maximum coordinate**: 2015

**Limitations**:
- Breaks in non-UTF-8 locales
- Ambiguous parsing with non-UTF-8 streams
- **Deprecated in favor of SGR mode (1006)**

### SGR Mode (1006) — **Recommended**

**Enable**: `CSI ? 1006 h`

**Format**:
```
CSI < Pb ; Px ; Py M    (button press / motion)
CSI < Pb ; Px ; Py m    (button release)
```

Where Pb, Px, Py are **decimal integers** separated by semicolons.

```
\x1b [ < <button> ; <column> ; <row> M
\x1b [ < <button> ; <column> ; <row> m
```

**Advantages over normal encoding**:
- No coordinate limit (decimal integers can be arbitrarily large)
- Distinguishes press (`M`) from release (`m`) — knows which button was released
- Button value uses same encoding as Cb but as a decimal number (not +32)
- Coordinates are 1-based decimal

**This is the standard for modern terminals.** All modern terminal emulators support it: Alacritty, Kitty, Ghostty, WezTerm, iTerm2, GNOME Terminal, Windows Terminal, etc.

### urxvt Mode (1015)

**Enable**: `CSI ? 1015 h`

**Format**:
```
CSI Pb ; Px ; Py M
```

Same as X10 button encoding but using decimal parameters. Note: no `<` prefix (unlike SGR).

**Limitations**:
- Cannot distinguish press from release (no `m` final character)
- Largely superseded by SGR mode (1006)
- Supported mainly by rxvt-unicode

### SGR-Pixels Mode (1016)

**Enable**: `CSI ? 1016 h`

Same format as SGR mode (1006) but coordinates are in **pixels** instead of character cells:

```
\x1b [ < <button> ; <pixel_x> ; <pixel_y> M
\x1b [ < <button> ; <pixel_x> ; <pixel_y> m
```

- Added in XTerm-359
- Useful for sub-cell precision (e.g., graphical TUI elements)
- Detectable via DECRQM: `CSI ? 1016 $ p`
- Limited terminal support as of 2025: xterm, foot, some others

---

## 3. Button and Modifier Encoding

### Cb Byte Structure (applies to all formats)

The button byte Cb is a bitmask:

```
Bit layout: 0bMMMMMMBB

BB (bits 0-1): Button identity
Bit 2 (4):    Shift modifier
Bit 3 (8):    Meta/Alt modifier
Bit 4 (16):   Control modifier
Bit 5 (32):   Motion flag (added for drag/motion events)
Bit 6 (64):   Wheel flag (added for scroll buttons)
Bit 7 (128):  Extended button flag (buttons 8-11)
```

### Button Values (bits 0-1)

| Cb & 0x03 | Event |
|-----------|-------|
| 0 | Button 1 (left) press |
| 1 | Button 2 (middle) press |
| 2 | Button 3 (right) press |
| 3 | Button release (normal mode only; SGR uses `m` instead) |

### Modifier Bits (bits 2-4)

| Bit | Value | Modifier |
|-----|-------|----------|
| 2 | 4 | Shift |
| 3 | 8 | Meta (Alt) |
| 4 | 16 | Control |

Note: In xterm, Shift and Control may be unavailable because xterm uses Control for popup menus and Shift for native selection. Modern terminals generally pass all modifiers.

### Motion Flag (bit 5)

| Value | Meaning |
|-------|---------|
| +32 | Motion event (mouse moved while button pressed, or any motion in mode 1003) |

### Wheel Buttons (bit 6)

| Cb Value | Event |
|----------|-------|
| 64 (0x40) | Scroll up (wheel button 4) |
| 65 (0x41) | Scroll down (wheel button 5) |
| 66 (0x42) | Scroll left (wheel button 6) |
| 67 (0x43) | Scroll right (wheel button 7) |

Wheel events are **press-only** — no release events are reported.

### Extended Buttons (bit 7)

| Cb Value | Event |
|----------|-------|
| 128 (0x80) | Button 8 (back/navigate back) |
| 129 (0x81) | Button 9 (forward/navigate forward) |
| 130 (0x82) | Button 10 |
| 131 (0x83) | Button 11 |

### Complete Cb Reference Table

| Cb | Event | Description |
|----|-------|-------------|
| 0 | Left press | Button 1 down |
| 1 | Middle press | Button 2 down |
| 2 | Right press | Button 3 down |
| 3 | Release | Any button up (normal mode) |
| 4-7 | +Shift | Above with Shift |
| 8-11 | +Alt | Above with Alt |
| 16-19 | +Ctrl | Above with Ctrl |
| 32 | Left drag | Motion with button 1 |
| 33 | Middle drag | Motion with button 2 |
| 34 | Right drag | Motion with button 3 |
| 35 | No-button motion | Motion without button (mode 1003) |
| 64 | Scroll up | Wheel button 4 |
| 65 | Scroll down | Wheel button 5 |
| 66 | Scroll left | Wheel button 6 |
| 67 | Scroll right | Wheel button 7 |
| 128 | Button 8 | Back button |
| 129 | Button 9 | Forward button |

Modifiers can be added to any value above (e.g., Shift+scroll up = 64 + 4 = 68).

---

## 4. Application Usage Patterns

### vim / neovim

```vim
set mouse=a        " Enable mouse in all modes
set ttymouse=sgr    " Use SGR encoding (auto-detected in modern vim)
```

- Enables mode 1000 (normal tracking) + mode 1006 (SGR encoding)
- Uses `XM` termcap entry: `\E[?1006;1000%?%p1%{1}%=%th%el%;`
- Mouse clicks position cursor, wheel scrolls, drag selects in visual mode
- Focus events (1004) used for auto-refresh on regain

### tmux

```
set -g mouse on
```

- Enables mode 1003 (any-event tracking) + mode 1006 (SGR encoding)
- Tracks all motion for pane/window border resize handles
- Passes mouse events through to inner applications when appropriate
- Scroll wheel triggers copy mode or is passed to inner application

### htop / btop

- Typically use mode 1000 or 1002 (button tracking, optional drag)
- Mouse clicks select processes, scroll navigates list
- Use ncurses `mousemask()` which handles mode negotiation

### Midnight Commander (mc)

- Uses xterm mouse support when TERM contains "xterm"
- Mode 1000 for click-based navigation
- GPM (General Purpose Mouse) on Linux console as fallback

### less

- With `-R` or `--mouse` flag, captures scroll events
- Uses mode 1000 for wheel scroll
- Less common for mouse interaction

---

## 5. GPUI to Terminal Mouse Mapping

### GPUI Mouse Event Types

| GPUI Event | Fields | Terminal Mapping |
|------------|--------|-----------------|
| `MouseDownEvent` | button, position, modifiers, click_count, first_mouse | Button press report |
| `MouseUpEvent` | button, position, modifiers, click_count | Button release report |
| `MouseMoveEvent` | position, modifiers, pressed_button | Motion report (modes 1002/1003) |
| `ScrollWheelEvent` | position, delta, modifiers, touch_phase | Scroll button 64/65 report |
| `MouseExitEvent` | position, modifiers, pressed_button | No terminal equivalent |

### GPUI MouseButton to Cb Mapping

| GPUI `MouseButton` | Cb Value (press) | Cb Value (motion) |
|---------------------|-------------------|---------------------|
| `Left` | 0 | 32 |
| `Middle` | 1 | 33 |
| `Right` | 2 | 34 |
| `Navigate(Back)` | 128 | N/A (not standard) |
| `Navigate(Forward)` | 129 | N/A (not standard) |
| None (mode 1003) | N/A | 35 |

### GPUI Modifiers to Cb Modifier Bits

| GPUI `Modifiers` field | Cb bit | Value |
|------------------------|--------|-------|
| `shift` | bit 2 | +4 |
| `alt` | bit 3 | +8 |
| `control` | bit 4 | +16 |
| `platform` (Cmd) | — | Not encoded in terminal protocol |
| `function` (Fn) | — | Not encoded in terminal protocol |

### Coordinate Conversion: Pixels to Grid

```rust
// From Zed's terminal implementation:
fn grid_point_and_side(
    pos: Point<Pixels>,
    cur_size: TerminalBounds,
    display_offset: usize,
) -> (AlacPoint, Side) {
    let col = (pos.x / cur_size.cell_width) as usize;
    let line = (pos.y / cur_size.line_height) as i32;
    // Clamp to terminal bounds
    // Adjust for display_offset (scrollback)
    // Determine left/right side of cell for selection
    (AlacPoint::new(line - display_offset, col), side)
}
```

### Scroll Event Conversion

Scroll wheel events generate repeated button 64 (up) or 65 (down) reports:

```rust
// From Zed's terminal implementation:
fn scroll_report(point, scroll_lines, event, mode) -> Option<impl Iterator<Item = Vec<u8>>> {
    // Convert ScrollWheelEvent.delta to direction
    let button = if scroll_up { 64 } else { 65 };
    // Generate N copies of the report for scroll_lines magnitude
    mouse_report(point, button, true, modifiers, format)
        .map(|report| repeat(report).take(scroll_lines))
}
```

### Encoding Format Selection

```rust
// Based on Zed's implementation:
enum MouseFormat {
    Sgr,            // When TermMode::SGR_MOUSE is set
    Normal(bool),   // Normal mode; bool = UTF8_MOUSE flag
}

fn from_mode(mode: TermMode) -> MouseFormat {
    if mode.contains(TermMode::SGR_MOUSE) {
        MouseFormat::Sgr
    } else {
        MouseFormat::Normal(mode.contains(TermMode::UTF8_MOUSE))
    }
}
```

### SGR Report Generation

```rust
fn sgr_mouse_report(point: AlacPoint, button: u8, pressed: bool) -> String {
    let c = if pressed { 'M' } else { 'm' };
    format!("\x1b[<{};{};{}{}", button, point.column + 1, point.line + 1, c)
}
```

### Normal Report Generation

```rust
fn normal_mouse_report(point: AlacPoint, button: u8, utf8: bool) -> Option<Vec<u8>> {
    let max_point = if utf8 { 2015 } else { 223 };
    if line >= max_point || column >= max_point {
        return None;  // Cannot encode — silently drop
    }
    let mut msg = vec![0x1b, b'[', b'M', 32 + button];
    // Encode coordinates with +32+1 offset
    // Use UTF-8 encoding for coords >= 95 when utf8 mode is on
    msg.push(32 + 1 + column as u8);
    msg.push(32 + 1 + line as u8);
    Some(msg)
}
```

Source: [Zed terminal/src/mappings/mouse.rs](https://github.com/zed-industries/zed/blob/main/crates/terminal/src/mappings/mouse.rs)

---

## 6. Selection Interaction and Shift Bypass

### The Core Problem

When a terminal application requests mouse reporting (modes 1000-1003), **all mouse events** are forwarded to the application. This means the terminal's native text selection (click-drag to select, double-click word, triple-click line) stops working.

### The Shift Bypass Convention

The universal convention (originated in xterm, adopted by all terminals):

> **Holding Shift while clicking bypasses mouse reporting and triggers terminal-native selection instead.**

Implementation in Zed/Crux:

```rust
pub fn mouse_mode(&self, shift: bool) -> bool {
    self.last_content.mode.intersects(TermMode::MOUSE_MODE) && !shift
}
```

When `mouse_mode()` returns `false` (Shift held), the terminal handles the mouse event itself for selection rather than encoding and forwarding it.

### Platform Variations

| Terminal | Bypass Modifier | Configurable |
|----------|----------------|--------------|
| xterm | Shift | No |
| Alacritty | Shift | No |
| iTerm2 | Option (Alt) | No |
| WezTerm | Shift (default) | Yes (`bypass_mouse_reporting_modifiers`) |
| Ghostty | Shift (default) | Yes (XTSHIFTESCAPE protocol) |
| Kitty | Shift | No |

### Ghostty's XTSHIFTESCAPE Protocol

Ghostty introduced a protocol for applications to negotiate shift bypass:

- **`CSI > Ps s`** — XTSHIFTESCAPE: application requests shift key passthrough
  - `Ps = 0`: Terminal can override shift (default — terminal uses shift for selection)
  - `Ps = 1`: Application requests shift be sent via mouse protocol

This allows applications like vim to receive Shift+Click events when needed, while still allowing the terminal to use Shift for selection by default.

**Recommendation for Crux**: Start with the standard Shift bypass. Consider XTSHIFTESCAPE support as a future enhancement.

### Vim Visual Mode vs Terminal Selection

When vim has `set mouse=a`, there are two overlapping selection systems:

1. **Vim visual mode**: vim receives mouse events and handles selection internally
2. **Terminal selection**: the terminal highlights text for clipboard copy

Resolution:
- Without Shift: vim handles selection (visual mode)
- With Shift: terminal handles selection (OS clipboard)
- Users expect Shift+drag to select text for system clipboard even inside vim

---

## 7. Edge Cases and Implementation Notes

### Coordinate Overflow

- **Normal encoding**: Silently drop events beyond column/row 223
- **UTF-8 encoding**: Silently drop events beyond column/row 2015
- **SGR encoding**: No limit (decimal integers)
- **Recommendation**: Always prefer SGR mode when the application requests it

### Rapid Scrolling

- macOS trackpad generates smooth scroll events with fractional deltas
- Accumulate fractional scroll deltas until reaching whole-line threshold
- Generate one mouse report per line of scroll
- For `scroll_lines = N`, repeat the report N times (Zed pattern)

### Mode Switching During Drag

- If the application disables mouse mode during a drag, stop sending reports immediately
- If mode switches from 1002 to 1003 mid-drag, begin reporting all motion
- **Race condition prevention**: Set encoding format (1006) before tracking mode (1000/1002/1003), and reset encoding after resetting tracking

### Cell-Change Deduplication

- In modes 1002 and 1003, only report motion when the mouse moves to a **different cell**
- Track the last reported grid position and suppress reports for the same cell
- This is critical for performance in mode 1003 (all motion)

### Alt-Screen Alternate Scroll

When `ALTERNATE_SCROLL` mode is active (mode 1007) and the application is on the alternate screen but mouse mode is NOT active, scroll events should be converted to cursor up/down key sequences instead:

```rust
fn alt_scroll(scroll_lines: i32) -> Vec<u8> {
    let cmd = if scroll_lines > 0 { b'A' } else { b'B' };  // Up or Down
    // Generate ESC O A (up) or ESC O B (down) repeated scroll_lines times
}
```

### Negative Line Coordinates

- Mouse events above the visible area (scrollback) should be clamped to line 0
- Mouse events below the visible area should be clamped to the last line
- `alacritty_terminal::AlacPoint` uses signed integers for line; clamp before encoding

### First Mouse / Focus Click

- GPUI provides `first_mouse: bool` on `MouseDownEvent`
- When true, this is the click that focuses the window — consider whether to forward to application
- Convention: some terminals eat the focus-click, others pass it through
- **Recommendation**: Make configurable, default to passing through

---

## 8. Implementation Checklist for Crux

### Phase 1 (MVP)
- [ ] Parse DECSET/DECRST for modes 1000, 1002, 1003, 1004, 1006
- [ ] Implement SGR mouse encoding (the only format needed for modern apps)
- [ ] Handle `MouseDownEvent` -> button press report
- [ ] Handle `MouseUpEvent` -> button release report
- [ ] Handle `ScrollWheelEvent` -> scroll button reports (64/65)
- [ ] Implement Shift bypass for terminal selection
- [ ] Implement `mouse_mode()` check against TermMode flags
- [ ] Coordinate conversion: GPUI pixels -> grid position
- [ ] Cell-change deduplication for motion events

### Phase 2 (Full Support)
- [ ] Handle `MouseMoveEvent` -> motion reports (modes 1002/1003)
- [ ] Normal (X10/X11) encoding for legacy applications
- [ ] UTF-8 encoding (mode 1005) for intermediate compatibility
- [ ] Focus in/out events (mode 1004)
- [ ] Alt-screen alternate scroll (mode 1007)
- [ ] Modifier encoding (Shift, Alt, Ctrl bits in Cb)

### Phase 3 (Advanced)
- [ ] SGR-Pixels mode (1016)
- [ ] urxvt mode (1015) for rxvt compatibility
- [ ] XTSHIFTESCAPE protocol (Ghostty-style shift negotiation)
- [ ] Extended buttons 8-11 (navigate back/forward)
- [ ] Configurable bypass modifier (Shift vs Option)
- [ ] Configurable first-mouse / focus-click behavior

---

## 9. Testing Strategy

### Manual Testing

1. **vim**: `set mouse=a` — click should position cursor, scroll should work
2. **tmux**: `set -g mouse on` — pane resize handles, scroll in copy mode
3. **htop**: Click process list, scroll
4. **less --mouse**: Scroll with wheel
5. **cat -v + mouse**: Verify raw escape sequences are correct

### Programmatic Testing

```bash
# Print mouse escape sequences to verify encoding:
printf '\e[?1000h\e[?1006h'  # Enable normal tracking + SGR
# Click/scroll in terminal, observe raw output
printf '\e[?1006l\e[?1000l'  # Disable
```

### Verification Points

- SGR format: `\x1b[<0;10;5M` for left click at column 10, row 5
- SGR release: `\x1b[<0;10;5m` (lowercase m)
- Scroll up: `\x1b[<64;10;5M`
- Scroll down: `\x1b[<65;10;5M`
- Shift+click: should NOT generate any mouse report (bypass to selection)
- Mode 1003 motion: `\x1b[<35;10;5M` for no-button motion at (10,5)
