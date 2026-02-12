---
title: Terminal Emulator Bugs and Lessons Learned
description: Known bugs and issues from Rio and other terminal emulators that Crux should learn from and avoid
phase: 1
topics:
  - bugs
  - lessons-learned
  - rio-terminal
  - best-practices
  - edge-cases
related:
  - terminal-emulation.md
  - terminal-architecture.md
  - ime-clipboard.md
  - keymapping.md
created: 2026-02-12
---

# Terminal Emulator Bugs and Lessons Learned

Comprehensive research on known bugs and issues from Rio terminal (Rust-based GPU-accelerated terminal) and other modern terminal emulators. This document helps Crux avoid common pitfalls and learn from the community's collective experience.

## Table of Contents

1. [Rio Terminal Specific Issues](#rio-terminal-specific-issues)
2. [CJK and IME Issues](#cjk-and-ime-issues)
3. [Font Rendering Bugs](#font-rendering-bugs)
4. [VT100 Emulation Edge Cases](#vt100-emulation-edge-cases)
5. [wcwidth and Wide Character Issues](#wcwidth-and-wide-character-issues)
6. [tmux Compatibility Issues](#tmux-compatibility-issues)
7. [Performance and Resource Issues](#performance-and-resource-issues)
8. [Graphics Protocol Issues](#graphics-protocol-issues)
9. [CSI Escape Sequence Parsing Bugs](#csi-escape-sequence-parsing-bugs)
10. [Configuration System Gotchas](#configuration-system-gotchas)
11. [Unicode and Emoji Rendering](#unicode-and-emoji-rendering)
12. [PTY and Process Management](#pty-and-process-management)
13. [Mouse Reporting Bugs](#mouse-reporting-bugs)
14. [Universal Terminal Emulator Pitfalls](#universal-terminal-emulator-pitfalls)

---

## Rio Terminal Specific Issues

### Font Rendering Issue (#1381)
**Description**: Visual bugs when rendering text with holes appearing in characters (e.g., "f" in Berkeley Mono font).

**Status**: Reported December 2025, ongoing

**Root Cause**: Likely related to Sugarloaf renderer's glyph rasterization or font metrics handling

**Lesson**: Font rendering requires careful handling of different font families' metrics. Implement comprehensive font testing with various fonts (Berkeley Mono, FiraCode, JetBrains Mono, etc.) before release.

**Reference**: [GitHub Issue #1381](https://github.com/raphamorim/rio/issues/1381)

### Binding Configuration Issues (#1008)
**Description**: Difficulty setting keyboard shortcuts to open floating terminals in Rio.

**Status**: Reported March 2025

**Root Cause**: Unclear binding configuration syntax or conflict with existing bindings

**Lesson**: Provide clear documentation and validation for keybinding configuration. Consider a keybinding tester/validator tool.

**Reference**: [GitHub Issue #1008](https://github.com/raphamorim/rio/issues/1008)

### Configuration File Partial Reading (#870)
**Description**: Some configuration options (shell, fonts) not being applied, while others (colors, log-level) work correctly.

**Status**: Reported, needs investigation

**Root Cause**: Likely TOML parsing order sensitivity or partial config reload logic

**Lesson**:
- Test configuration loading comprehensively
- Implement config validation on load
- Log which config values are applied vs. ignored
- Document config file structure requirements (see TOML gotchas below)

**Reference**: [GitHub Issue #870](https://github.com/raphamorim/rio/issues/870)

---

## CJK and IME Issues

### CJK Line Breaking Display Bug (#1013)
**Description**: Unable to display some CJK characters when breaking the line. If there are odd spaces left in a line, CJK characters (each occupies 2 spaces) between special characters and end of line won't display.

**Status**: Known issue in Rio

**Root Cause**: Width calculation mismatch between CJK character width (2 cells) and available line space

**Lesson**:
- CJK characters must respect 2-cell width consistently
- Line breaking logic must check for sufficient space (2 cells) before placing CJK characters
- Test with Chinese, Japanese, Korean text at various line wrap positions
- Related to wcwidth calculations (see section below)

**Reference**: [GitHub Issue #1013](https://github.com/raphamorim/rio/issues/1013)

### CJK Character Baseline Issues (Fixed in Rio)
**Description**: CJK characters displaying "higher" than Latin characters, creating misaligned baselines.

**Status**: Fixed in Rio

**Root Cause**: Inconsistent font metrics handling between Latin and CJK fonts

**Lesson**:
- Implement consistent baseline adjustment across all scripts
- CJK font metrics require special handling for vertical alignment
- Test mixed Latin/CJK text rendering extensively

**Reference**: [Rio Changelog](https://rioterm.com/changelog)

### IME Preedit and Committed Text Separation (Universal Issue)
**Description**: IME composition text (preedit) incorrectly sent to PTY before commit, or committed text rendered incorrectly.

**Status**: Common bug across terminals (documented in Alacritty, Warp, others)

**Root Cause**: Confusion between preedit (composition) state and committed (final) state

**Lesson**:
- **CRITICAL**: Preedit text MUST be rendered as overlay only, never sent to PTY
- Only committed text goes to PTY write
- Keep composition state separate from terminal grid
- See Crux's `research/platform/ime-clipboard.md` for full IME architecture

**References**:
- [Alacritty Issue #6942](https://github.com/alacritty/alacritty/issues/6942)
- [Alacritty Issue #8079](https://github.com/alacritty/alacritty/issues/8079)
- [OpenCode Issue #8652](https://github.com/anomalyco/opencode/issues/8652)

### Keyboard Shortcuts Don't Work with CJK IME Active (Universal Issue)
**Description**: When Korean/Japanese/Chinese IME is active on macOS, keyboard shortcuts stop working in TUI applications.

**Status**: Affects multiple terminals and TUI apps

**Root Cause**: Key events consumed by IME layer before reaching application's key handler

**Lesson**:
- Handle `marked_text_range()` state correctly in NSTextInputClient
- When IME is in composition mode, some keys should go to IME, others to shortcuts
- Implement proper IME state checking before processing shortcuts
- Test all shortcuts with various IME active states

**Reference**: [OpenCode Issue #8652](https://github.com/anomalyco/opencode/issues/8652)

### IME Cursor Position Support (Feature Request Trend)
**Description**: Need proper IME cursor positioning for CJK input to display composition popups at correct location.

**Status**: Becoming standard feature expectation

**Lesson**:
- Rio implements `ime-cursor-positioning` config
- Must convert terminal cursor position (row/col) to screen coordinates (pixels)
- Update IME cursor rectangle on every cursor movement
- Critical for good CJK user experience

**Reference**:
- [Rio IME Support Docs](https://rioterm.com/docs/features/ime-support)
- [Claude Code Feature Request #19207](https://github.com/anthropics/claude-code/issues/19207)

---

## Font Rendering Bugs

### Variable Fonts Not Supported (#345)
**Description**: Sugarloaf treats variable fonts as static regular fonts. Bold text doesn't render correctly on variable fonts like GitLab Mono.

**Status**: Known limitation in Rio/Sugarloaf

**Root Cause**: Font loader doesn't handle variable font axes (weight, width, slant)

**Lesson**:
- Decide early: support variable fonts or document as limitation
- If supporting variable fonts, need proper font variation axis handling
- Map terminal bold/italic attributes to variable font axes
- Test with popular variable fonts: Inter, Recursive, GitLab Mono

**Reference**: [GitHub Issue #345](https://github.com/raphamorim/rio/issues/345)

### Bold Font Weight Issues (#818)
**Description**: Bold font renderings appear odd depending on configured weight.

**Status**: Known issue

**Root Cause**: Weight calculation or font selection algorithm for bold variants

**Lesson**:
- Bold should consistently select next heavier weight or synthesize boldness
- Test bold rendering across font weights (300, 400, 500, 600, 700, 900)
- Validate that synthesized bold (if used) looks acceptable

**Reference**: [GitHub Issue #818](https://github.com/raphamorim/rio/issues/818)

### Sugarloaf Non-Monospace Icon Handling
**Description**: Sugarloaf considers text font for text but icons may not be monospaced, breaking alignment.

**Status**: Known limitation

**Root Cause**: Font fallback system doesn't enforce monospace requirement

**Lesson**:
- Font fallback chain must enforce monospace for terminal context
- Icons from non-monospace fonts need width normalization
- Consider built-in renderer for common symbols (box drawing, powerline)

**Reference**: [Rio Release 0.0.7](https://rioterm.com/blog/2023/07/07/release-0.0.7)

### Glyph Rendering Edge Cases
**Description**: Symbols like ➜ and ✔ failing to render correctly.

**Status**: Reported in early Rio versions

**Root Cause**: Missing glyphs in font or font fallback failure

**Lesson**:
- Implement robust font fallback chain
- Log missing glyphs for debugging
- Consider built-in rendering for common terminal symbols
- Test with popular shell prompts (oh-my-zsh, starship, powerlevel10k)

**Reference**: [SourceForge Ticket #160](https://sourceforge.net/p/rio/tickets/160/)

### Padding and Rendering Alignment Issues
**Description**: Weird rendering behavior when setting `padding-x` in config. TUI cursor movement control sequences caused rendering problems.

**Status**: Fixed in Rio

**Root Cause**: Coordinate calculation didn't account for padding offset

**Lesson**:
- All coordinate calculations must account for padding/margin offsets
- Test TUI applications with various padding settings
- Verify cursor positioning with padding enabled

**Reference**: [Rio Changelog](https://rioterm.com/changelog)

---

## VT100 Emulation Edge Cases

### Quality Measurement: Borderline Situations
**Description**: Terminal emulator quality is measured by how it handles "unexpected, faulty or weird combinations of sequences."

**Status**: Ongoing challenge for all terminals

**Root Cause**: VT100/xterm specs have ambiguous or undefined behavior for edge cases

**Lesson**:
- Test with escape sequence fuzzers
- Document behavior for undefined cases
- Follow xterm reference implementation for ambiguous cases
- Run vttest suite comprehensively

**Reference**: [State of Terminal Emulation 2025](https://www.jeffquast.com/post/state-of-terminal-emulation-2025/)

### GNOME Terminal TAB Handling Bug
**Description**: "Steaming pile of bugs in various problem areas, some pretty basic (such as TAB handling)."

**Status**: Long-standing GNOME Terminal issue

**Root Cause**: Incorrect TAB character width calculation or cursor movement

**Lesson**:
- TAB handling is deceptively complex (depends on tab stops, current column)
- Default tab stops every 8 columns, but can be customized (HTS, TBC sequences)
- Test TAB at various column positions, especially near right margin

**Reference**: [Linux Terminal Emulator Features](https://babbagefiles.xyz/terminal-emulator-vtt-features-compatibility/)

### VT420+ Horizontal Margins Unsupported
**Description**: Many terminals don't support horizontal margins (VT420 feature), plus various VT420/VT520/ISO-6429 sequences.

**Status**: Common limitation

**Root Cause**: Focus on xterm compatibility rather than full VT420/VT520 emulation

**Lesson**:
- Decide on target emulation level: VT100, VT220, VT420, xterm, or modern xterm+
- Document supported escape sequences explicitly
- For Crux: xterm-level compatibility is sufficient for modern software

**Reference**: [Linux Terminal Emulator Features](https://babbagefiles.xyz/terminal-emulator-vtt-features-compatibility/)

### Split Escape Sequence Writes
**Description**: Arrow keys may fail when escape sequences are split across multiple PTY reads (e.g., ESC followed by [ in separate reads).

**Status**: Must be handled correctly

**Root Cause**: Parser doesn't maintain state across read boundaries

**Lesson**:
- Parser MUST handle incomplete sequences across read boundaries
- Maintain parse state in terminal struct
- alacritty_terminal handles this correctly (use as reference)
- Test by writing escape sequences byte-by-byte

**Reference**: [Microsoft Terminal Issue #4037](https://github.com/microsoft/terminal/issues/4037)

---

## wcwidth and Wide Character Issues

### No Formal Standard for Character Width
**Description**: "No established formal standards exist at present on which Unicode character shall occupy how many cell positions on character terminals."

**Status**: Fundamental limitation of terminal model

**Root Cause**: Unicode was designed after terminal model was established; impedance mismatch

**Lesson**:
- Use established wcwidth implementations (Markus Kuhn's mk_wcwidth() or rust unicode-width crate)
- Stay updated with Unicode version updates
- Test with emojis, CJK, combining characters, zero-width characters

**Reference**:
- [wcwidth PyPI](https://pypi.org/project/wcwidth/)
- [Markus Kuhn's wcwidth.c](https://www.cl.cam.ac.uk/~mgk25/ucs/wcwidth.c)

### Alacritty Early Bug: All Characters Width 1 (#265)
**Description**: In early Alacritty, all characters took up 1 cell instead of respecting Unicode width (0, 1, or 2 cells).

**Status**: Fixed in Alacritty

**Root Cause**: Didn't use wcwidth function or equivalent

**Lesson**:
- MUST use unicode-width crate (Rust) or wcwidth(3) function
- CJK characters (except halfwidth) = 2 cells
- Combining characters = 0 width
- Default = 1 width

**Reference**: [Alacritty Issue #265](https://github.com/alacritty/alacritty/issues/265)

### CJK Ambiguous Width Characters
**Description**: East Asian Ambiguous characters can be width 1 or 2 depending on locale/terminal config.

**Status**: Ongoing compatibility challenge

**Root Cause**: Unicode designates some characters as "ambiguous width" for legacy compatibility

**Lesson**:
- Default to ambiguous=1 (narrow) for Western locales
- For CJK locales: ambiguous=2 (wide)
- Make configurable via environment variable or config
- wcwidth-cjk wrapper exists for this purpose

**References**:
- [wcwidth-cjk GitHub](https://github.com/fumiyas/wcwidth-cjk)
- [Microsoft Terminal Issue #370](https://github.com/microsoft/terminal/issues/370)

### Recent wcwidth Bugfixes (2024-2025)
**Description**: Version 0.5.3 fixed Brahmic Virama conjunct formation bug. Version 0.5.2 fixed category Mc (Spacing Combining Mark) measurement.

**Status**: Actively maintained

**Lesson**:
- Use latest unicode-width crate in Rust
- Keep dependencies updated for Unicode bug fixes
- Complex scripts (Brahmic, Arabic, Thai) need special attention

**Reference**: [wcwidth Documentation](https://wcwidth.readthedocs.io/en/stable/intro.html)

---

## tmux Compatibility Issues

### Feature Redundancy and Keybinding Conflicts (#213)
**Description**: Rio's built-in multiplexing (tabs, splits) creates feature redundancy and conflicting keybindings with tmux/zellij.

**Status**: Rio added "Plain" navigation mode to address this

**Root Cause**: Terminal emulator trying to do too much; multiplexing should be separate layer

**Lesson**:
- **For Crux**: Keep multiplexing separate (Phase 2 feature)
- Support tmux/zellij as first-class citizens
- Provide mode to disable built-in multiplexing
- Default keybindings shouldn't conflict with tmux defaults (Ctrl+B prefix)

**Reference**: [GitHub Issue #213](https://github.com/raphamorim/rio/issues/213)

### tmux Split Separator Misalignment (#740)
**Description**: With Starship prompt's time module, tmux split separator shifts to the right.

**Status**: Reported

**Root Cause**: Width calculation issue with complex prompts or ANSI escape sequences in status line

**Lesson**:
- Test with tmux split panes extensively
- Test with popular prompt frameworks (Starship, oh-my-zsh themes)
- May be related to wcwidth issues or ANSI sequence parsing

**Reference**: [GitHub Issue #740](https://github.com/raphamorim/rio/issues/740)

### Commands Without Prefix Intercepted (#354)
**Description**: Tmux commands without prefix key are intercepted by Rio.

**Status**: Keybinding conflict issue

**Root Cause**: Rio's default keybindings overlap with tmux's

**Lesson**:
- Document keybinding conflicts clearly
- Provide easy way to disable terminal's default bindings
- Consider reserved "safe zones" for multiplexers

**Reference**: [GitHub Issue #354](https://github.com/raphamorim/rio/issues/354)

### Kitty Keyboard Protocol Breaks with tmux (#599)
**Description**: `use-kitty-keyboard-protocol = true` works fine without tmux but stops working with tmux.

**Status**: Known compatibility issue

**Root Cause**: tmux doesn't understand/forward Kitty keyboard protocol

**Lesson**:
- Kitty keyboard protocol requires tmux support (recent tmux versions)
- Document protocol version requirements
- Provide fallback to legacy keyboard encoding
- Test protocol negotiation with tmux passthrough

**Reference**: [GitHub Issue #599](https://github.com/raphamorim/rio/issues/599)

### Wrong Window Size Passed to tmux (#98)
**Description**: Using tmux inside Rio results in incorrect window size.

**Status**: Reported in early Rio versions

**Root Cause**: SIGWINCH signal not sent or incorrect dimensions in ioctl

**Lesson**:
- Send SIGWINCH on window resize
- Use correct ioctl (TIOCSWINSZ) with accurate dimensions
- Account for padding/margins when calculating terminal size
- portable-pty crate should handle this, but verify

**Reference**: [GitHub Issue #98](https://github.com/raphamorim/rio/issues/98)

### tmux 256 Color Configuration
**Description**: Tmux uses 256 colors configuration which needs to be enabled.

**Status**: Common configuration issue

**Root Cause**: TERM environment variable mismatch

**Lesson**:
- Set `TERM=xterm-crux` (or `xterm-256color` for compatibility)
- Ensure terminfo supports 256 colors (colors#256)
- Document tmux configuration: `set -g default-terminal "xterm-crux"`
- Test with `tput colors` inside tmux

**Reference**: [Rio FAQ](https://rioterm.com/docs/frequently-asked-questions)

---

## Performance and Resource Issues

### 83% GPU Memory Reduction Achievement (Rio)
**Description**: Rio achieved 83% reduction in GPU memory usage through optimizations.

**Status**: Major improvement

**Techniques Used**:
- Text run caching (cache shaped text segments, not full lines)
- SIMD implementation for rendering
- Vertex pool system with size-categorized buffers and LRU eviction
- Deferred damage checking and render coalescing

**Lesson**:
- GPU memory can be expensive for terminals (full screen texture + buffers)
- Cache at the right granularity (text runs, not full lines or per-character)
- SIMD helps (GPUI likely uses this already)
- Damage tracking essential for performance
- Consider memory pools for frequently allocated objects

**Reference**: [Rio Changelog](https://rioterm.com/changelog)

### Text Run Caching vs. Line Caching
**Description**: Rio replaced line-based caching with text run caching (words, operators, keywords).

**Status**: Implemented optimization

**Root Cause**: Lines change frequently, but individual text segments are reused across screen

**Lesson**:
- Cache at segment level, not line level
- Shaped text (font rendering) is expensive, cache aggressively
- Identify reusable units (common words in editors: `function`, `const`, `return`)
- Balance cache size vs. memory usage

**Reference**: [Rio Changelog](https://rioterm.com/changelog)

### SIMD Platform-Adaptive Implementation
**Description**: Rio uses platform-adaptive SIMD (AVX2 > SSE2 > NEON > scalar).

**Status**: Implemented optimization

**Lesson**:
- Rust supports SIMD via std::simd (nightly) or platform intrinsics
- Auto-detect CPU features at runtime
- Useful for batch operations (color conversion, alpha blending)
- GPUI might handle this at Metal/GPU level already

**Reference**: [Rio Changelog](https://rioterm.com/changelog)

### Render Coalescing and Batching
**Description**: Rio batches multiple rapid terminal updates into single render passes.

**Status**: Critical optimization

**Root Cause**: PTY can send hundreds of small writes per second; rendering each is wasteful

**Lesson**:
- Implement 4ms window or 4KB data threshold (Zed pattern)
- Use wakeup events to flush batched updates
- Balance latency vs. throughput
- See `crux-terminal-view` damage tracking implementation

**Reference**: [Rio Changelog](https://rioterm.com/changelog)

---

## Graphics Protocol Issues

### Sixel: No True Color (24-bit) Support
**Description**: Sixel format supports RGB/HLS with smaller color space (16-bit), not true 24-bit color. Terminal implementations limited by color register count.

**Status**: Architectural limitation of Sixel protocol

**Root Cause**: Sixel designed in 1980s for limited color hardware

**Lesson**:
- Sixel has inherent color limitations
- Modern alternatives: Kitty graphics protocol, iTerm2 inline images
- If implementing Sixel, document color space limitations
- Consider supporting multiple graphics protocols

**Reference**: [Codeberg Foot Issue #481](https://codeberg.org/dnkl/foot/issues/481)

### Sixel Cell Size and Font Resize Issues
**Description**: Not able to read cell size to position images properly. Images don't resize when font size changes.

**Status**: Common Sixel implementation bug

**Root Cause**: Sixel coordinates in pixels, but terminal layout is cells; mismatch on font change

**Lesson**:
- Track cell dimensions (pixels per cell)
- Recalculate image positions on font size change
- Send proper responses to XTGETTCAP queries
- Test image display with font size changes

**Reference**: [State of Terminal Emulation 2025](https://www.jeffquast.com/post/state-of-terminal-emulation-2025/)

### Text and Image Interaction During Resize
**Description**: Data buffer issues and overlapping content when terminal resizes with images displayed.

**Status**: Challenging edge case

**Root Cause**: Images positioned in pixel coordinates; cells reflow on resize

**Lesson**:
- Clear images on resize, or
- Implement image reflow (complex), or
- Use Kitty protocol's placement ID system for tracking
- Document behavior explicitly

**Reference**: [State of Terminal Emulation 2025](https://www.jeffquast.com/post/state-of-terminal-emulation-2025/)

### Kitty Graphics Protocol: Implementation Differences
**Description**: Differences between implementations, crashes, bugs, documentation ambiguities (e.g., base64 specs).

**Status**: Newer protocol, still maturing

**Lesson**:
- Follow Kitty's reference implementation closely
- Participate in protocol discussion (GitHub issues)
- Test with actual Kitty terminal for compatibility
- Document deviations from spec

**Reference**: [Codeberg Foot Issue #481](https://codeberg.org/dnkl/foot/issues/481)

---

## CSI Escape Sequence Parsing Bugs

### Undefined Behavior for Characters Outside 0x20-0x7E
**Description**: CSI sequence behavior undefined when containing chars outside 0x20-0x7E range (C0 controls, DEL, high bit set).

**Status**: Spec ambiguity

**Possible Responses**:
- Ignore the byte
- Process it immediately
- Abort CSI sequence immediately
- Ignore rest of sequence

**Lesson**:
- Follow xterm behavior for consistency
- alacritty_terminal handles this correctly (use as reference)
- Document chosen behavior
- Test with malformed sequences

**Reference**: [Wez Terminal Escape Sequences](https://wezterm.org/escape-sequences.html)

### Crashes with Large or Negative Parameters
**Description**: Multiple terminals crash with large (INT_MAX) or negative integer parameters in escape sequences.

**Status**: Common vulnerability

**Test Cases**: `2147483647`, `-2147483648`, various hex values

**Lesson**:
- Validate and clamp integer parameters
- Use checked arithmetic or saturating operations
- Negative parameters should be ignored or treated as 0
- Fuzz test with extreme values
- Max reasonable values: 65535 for positions, 256 for colors

**Reference**:
- [OSS Security Terminal Emulators](https://www.openwall.com/lists/oss-security/2017/05/01/13)
- [OSS Security List](https://seclists.org/oss-sec/2017/q2/183)

### PuTTY CVE-2021-33500
**Description**: Escape sequence handling vulnerability fixed in PuTTY 0.75+.

**Status**: Fixed

**Lesson**:
- Escape sequence parsing is security-sensitive
- Validate all inputs
- Follow secure coding practices
- Stay updated on terminal emulator CVEs

**Reference**: [OSS Security List](https://seclists.org/oss-sec/2017/q2/183)

### Modifier Keys Generate Incorrect Sequences (Fixed)
**Description**: Old GNOME Terminal/Terminator: F1-F4 with modifiers generated wrong sequences, colliding with cursor position responses.

**Status**: Fixed in newer versions

**Root Cause**: Incorrect escape sequence generation for modified function keys

**Lesson**:
- Follow xterm's sequences for modified keys
- Test all function keys with Shift/Ctrl/Alt/Meta combinations
- Ensure no collisions with terminal responses (DSR, etc.)
- Document key encoding behavior

**Reference**: [BiDi Terminal Escape Sequences](https://terminal-wg.pages.freedesktop.org/bidi/recommendation/escape-sequences.html)

### SGR Parameter Omission Edge Case
**Description**: Valid to omit code number in SGR sequences. `CSI m` equals `CSI 0 m` (reset attributes).

**Status**: Spec-defined behavior

**Lesson**:
- Parser must handle empty parameters
- Default to 0 for SGR when parameter omitted
- Test with various empty parameter patterns: `CSI m`, `CSI ;m`, `CSI ;;m`
- alacritty_terminal handles this correctly

**Reference**: [ANSI Escape Code Wikipedia](https://en.wikipedia.org/wiki/ANSI_escape_code)

### Security: Escape Sequences Executing Code
**Description**: Historical vulnerabilities where malicious escape sequences could execute arbitrary code.

**Status**: Fixed in modern terminals, but vigilance required

**Lesson**:
- NEVER execute commands based on escape sequences without explicit user consent
- OSC 52 (clipboard) should have size limits
- OSC 8 (hyperlinks) needs validation
- Be cautious with any sequence that interacts with system

**Reference**:
- [Protean Security: Executing Code via Escape Sequences](https://www.proteansec.com/linux/blast-past-executing-code-terminal-emulators-via-escape-sequences/)
- [CyberArk: Abusing ANSI Escape Characters](https://www.cyberark.com/resources/threat-research-blog/dont-trust-this-title-abusing-terminal-emulators-with-ansi-escape-characters)

---

## Configuration System Gotchas

### TOML Parameters Without Headers Must Be First
**Description**: In Rio's TOML config, parameters without a header must be at the beginning of the file, otherwise they're ignored.

**Status**: TOML specification behavior

**Root Cause**: TOML treats everything after a section header `[section]` as part of that section

**Lesson**:
- Document configuration file structure clearly
- Validate configuration and warn about misplaced parameters
- Consider using explicit `[general]` or `[terminal]` section for clarity
- Provide example config with comments

**Reference**: [GitHub Issue #870](https://github.com/raphamorim/rio/issues/870)

### TOML Escape Sequence Notation: \u001b vs \x1b
**Description**: In TOML strings, must use `\u001b` for ESC character. The `\x1b` notation doesn't work.

**Status**: TOML specification

**Root Cause**: TOML supports Unicode escapes `\uXXXX` but not hex escapes `\xXX`

**Lesson**:
- Document escape sequence format for TOML config
- Provide examples with correct notation
- Consider alternative: use named constants (e.g., `<Esc>` instead of literal)
- Validate config and provide helpful error messages

**Reference**: [GitHub Issue #870](https://github.com/raphamorim/rio/issues/870)

### Configuration Override Issues
**Description**: Setup commands can override user configuration files, losing custom settings.

**Status**: Common user complaint across terminals

**Lesson**:
- Never silently overwrite config files
- Prompt before making changes, or
- Create backup before modifying, or
- Use separate config file for auto-generated settings
- Respect existing user configuration

**Reference**: [Claude Code Issue #16066](https://github.com/anthropics/claude-code/issues/16066)

---

## Unicode and Emoji Rendering

### Emoji Width Inconsistency
**Description**: Emojis should display in color and occupy 2 cells (Wide), but implementations vary. Only 7 of 23 tested terminals handle Variation Selector-16 (VS-16) correctly.

**Status**: Widespread compatibility issue

**Root Cause**:
- Unicode version differences
- VS-16 handling complexity
- Font fallback issues

**Lesson**:
- Implement proper VS-16 (U+FE0F) handling
- Wide emoji (with VS-16) = 2 cells
- Text emoji (with VS-15 or no selector) = 1 cell
- Test with emoji test suite
- Keep Unicode data tables updated

**References**:
- [Microsoft Terminal Discussion #13724](https://github.com/microsoft/terminal/discussions/13724)
- [Terminal Emulators Unicode Edition Test Results](https://www.jeffquast.com/post/ucs-detect-test-results/)

### Unicode Version Support Varies
**Description**: Terminals support different Unicode versions. Konsole, iTerm2, kitty support Unicode 15.0.0, while Hyper and VSCode only support 12.1.0.

**Status**: Ongoing challenge

**Lesson**:
- Stay current with Unicode releases (annual updates)
- Use unicode-width crate and keep it updated
- Document supported Unicode version
- Test with recent emoji and characters

**Reference**: [Terminal Emulators Unicode Edition](https://www.jeffquast.com/post/ucs-detect-test-results/)

### Emoji That Should Be Wide (Unicode 9+)
**Description**: Unicode 9+ defined more emojis as wide, but older terminals don't recognize them.

**Status**: Version compatibility issue

**Root Cause**: Unicode data tables not updated

**Lesson**:
- Use maintained Unicode width libraries
- Don't hand-roll wcwidth tables
- For Rust: use unicode-width crate (actively maintained)

**Reference**: [Ubuntu Bug #1665140](https://bugs.launchpad.net/ubuntu/+source/gnome-terminal/+bug/1665140)

### Rio: No Fallback Font Support (#266)
**Description**: Characters like Japanese not recognized when using FiraCode Nerd Font (lacks Japanese glyphs). No font fallback.

**Status**: Known limitation in Rio

**Root Cause**: Font system doesn't implement fallback chain

**Lesson**:
- Implement font fallback chain: primary → secondary → system default
- macOS: use CTFontCreateUIFontForLanguage or Core Text fallback
- Test with fonts lacking specific scripts (Latin-only font with CJK text)
- Log when falling back to alternate font (helps debugging)

**Reference**: [GitHub Issue #266](https://github.com/raphamorim/rio/issues/266)

### Terminal Crashes Rendering Emoji
**Description**: Some terminals crash when rendering specific emojis.

**Status**: Rare but serious bug

**Root Cause**: Unhandled font rendering errors or GPU texture issues

**Lesson**:
- Wrap font rendering in error handling
- Provide fallback for missing glyphs (□ or ?)
- Fuzz test with full emoji range (U+1F300 - U+1FAF8)
- GPUI should handle crashes gracefully, but test thoroughly

**Reference**: [Regolith-st Issue #14](https://github.com/regolith-linux/regolith-st/issues/14)

### Ghostty's Excellent Unicode Support (2025)
**Description**: Ghostty scored highest in Unicode testing, with thoroughly correct implementation.

**Status**: New benchmark for terminal Unicode support

**Lesson**:
- Study Ghostty's Unicode implementation as reference
- Run ucs-detect test suite against Crux
- Aim for Ghostty-level Unicode correctness

**Reference**: [State of Terminal Emulation 2025](https://www.jeffquast.com/post/state-of-terminal-emulation-2025/)

### Text Sizing Protocol (2025)
**Description**: New protocol allowing text to escape monospace constraints for better display of diverse world languages.

**Status**: Emerging standard

**Lesson**:
- Monitor this protocol development
- May be relevant for Phase 4+ features
- Could improve CJK display quality

**Reference**: [State of Terminal Emulation 2025](https://www.jeffquast.com/post/state-of-terminal-emulation-2025/)

---

## PTY and Process Management

### Race Condition: "Cannot resize a pty that has already exited"
**Description**: Error indicates race condition or incorrect state management when resizing PTY.

**Status**: Common bug pattern

**Root Cause**:
- Resize signal sent after PTY closed
- State checking missing before operations
- Event ordering issue

**Lesson**:
- Check PTY alive state before resize operations
- Handle ESRCH/EPIPE errors gracefully
- Use atomic state tracking for PTY lifecycle
- portable-pty crate should help, but validate

**Reference**: [Gemini CLI Issue #12294](https://github.com/google-gemini/gemini-cli/issues/12294)

### Fork/Exec Signal Race Condition
**Description**: Parent sends SIGTERM to child between fork and exec while child still runs parent's code, potentially executing parent's signal handler.

**Status**: Classic UNIX race condition

**Mitigation**:
1. Block all signals before fork
2. In child: reset known signals
3. In child: unblock all signals
4. In child: exec

**Lesson**:
- This is tricky to get right
- portable-pty crate should handle this
- If implementing PTY manually, follow above pattern
- Test with rapid process spawn/kill cycles

**Reference**: [Narkive: fork/exec race condition](https://comp.unix.programmer.narkive.com/AgezE86f/fork-exec-race-condition-with-signals)

### creack/pty v1.1.20 Reverted Due to Race Condition
**Description**: Go's creack/pty library had to revert v1.1.20 due to Linux race condition, causing issues in Argo Workflows.

**Status**: Demonstrates PTY complexity

**Lesson**:
- PTY libraries have subtle bugs
- portable-pty is mature but test thoroughly
- Monitor upstream issues in dependencies
- Have integration tests for PTY lifecycle

**Reference**: [Argo Workflows Issue #12829](https://github.com/argoproj/argo-workflows/issues/12829)

### node-pty Not Thread Safe
**Description**: node-pty library explicitly not thread safe; running across worker threads causes issues.

**Status**: Known limitation

**Lesson**:
- portable-pty in Rust: check thread safety guarantees
- Terminal state is inherently single-threaded (VT state machine)
- Use message passing if multi-threading needed

**Reference**: [Microsoft node-pty](https://github.com/microsoft/node-pty)

### Security: Race Condition in Stream Flushing
**Description**: Race condition in Deno between tcflush() and stdin reading could bypass permission prompts using ANSI escape sequences.

**Status**: Fixed (CVE assigned)

**Lesson**:
- Be cautious with tcflush timing
- Validate all input before security decisions
- ANSI escape sequences can manipulate display

**Reference**: [Deno Security Advisory](https://github.com/denoland/deno/security/advisories/GHSA-95cj-3hr2-7j5j)

---

## Mouse Reporting Bugs

### SGR-Pixels Mode 1016 Support (Emerging)
**Description**: SGR-Pixels (mode 1016) reports mouse position in pixels instead of cells. Added in XTerm-359, being adopted by modern terminals.

**Status**: Feature request in multiple terminals (2025)

**Lesson**:
- Plan for SGR-Pixels support (Phase 4+)
- Provides better precision for high-DPI displays
- Requires tracking both cell and pixel coordinates
- Test with applications using this mode

**References**:
- [Microsoft Terminal Issue #18591](https://github.com/microsoft/terminal/issues/18591)
- [WezTerm Issue #1457](https://github.com/wezterm/wezterm/issues/1457)

### Windows Terminal: SGR Mouse Reports Incorrect (2025)
**Description**: Any-event SGR mouse reports end with lowercase "m" when they should end with uppercase "M".

**Status**: Active bug (March 2025)

**Root Cause**: Incorrect escape sequence generation for mouse button release

**Lesson**:
- Mouse button press: `CSI < ... M`
- Mouse button release: `CSI < ... m`
- Motion events: depend on mode
- Test mouse reporting with actual applications (vim, tmux mouse mode)
- Cross-reference xterm documentation

**Reference**: [Microsoft Terminal Issue #18712](https://github.com/microsoft/terminal/issues/18712)

### Negative Coordinates When Mouse Outside Window
**Description**: XTerm had bug where SGR pixel mouse events reported negative coordinates when mouse outside window. Fixed in patch 404.

**Status**: Fixed in XTerm, Foot matched the fix

**Lesson**:
- Decide on behavior: clamp to 0 or report negative
- Follow XTerm's corrected behavior for compatibility
- Test mouse tracking at window boundaries
- Document behavior explicitly

**Reference**: [Ghostty Discussion #9647](https://github.com/ghostty-org/ghostty/discussions/9647)

---

## Universal Terminal Emulator Pitfalls

### State Corruption in Terminology
**Description**: Terminology produces inconsistent results between executions, suggesting state corruption.

**Status**: Known issue

**Lesson**:
- Terminal state machine must be deterministic
- Reset state properly between tests
- Avoid global mutable state
- Use pure functions where possible

**Reference**: [State of Terminal Emulation 2025](https://www.jeffquast.com/post/state-of-terminal-emulation-2025/)

### Scrollback Buffer Clearing Issues
**Description**: Terminal clear/compact commands accidentally clear scrollback buffer instead of just visible screen. Issue with tmux integration where scrollback reappears on resize.

**Status**: Common UX problem

**Lesson**:
- Distinguish between:
  - Clear visible screen (ED 2)
  - Clear visible screen + scrollback (ED 3)
- Don't clear tmux/screen scrollback buffer
- Respect alternate screen buffer boundaries

**References**:
- [Claude Code Issue #7597](https://github.com/anthropics/claude-code/issues/7597)
- [Claude Code Issue #11260](https://github.com/anthropics/claude-code/issues/11260)

### macOS XProtect Performance Impact (Alacritty)
**Description**: macOS antivirus (XProtect) scans first-run binaries, slowing down bash command execution significantly in Alacritty vs Terminal.app/iTerm2.

**Status**: Platform limitation, but varies by terminal

**Lesson**:
- Consider code signing and notarization to improve first-run experience
- Document potential first-run slowness on macOS
- May require "developer tool" designation in system preferences
- See `research/platform/homebrew-distribution.md` for signing process

**Reference**: [Alacritty Issue #8785](https://github.com/alacritty/alacritty/issues/8785)

### "Can't Find Terminal Definition" (Terminfo Missing)
**Description**: Applications fail with "can't find terminal definition" when terminfo not installed system-wide.

**Status**: Common deployment issue

**Lesson**:
- Ship terminfo with application, or
- Install to system terminfo dirs during setup
- Document manual installation: `tic -x -e xterm-crux,crux,crux-direct crux.terminfo`
- Provide fallback to xterm-256color for compatibility

**Reference**: [NixOS Issue #411867](https://github.com/NixOS/nixpkgs/issues/411867)

### Terminal Name Choice: xterm- Prefix Critical
**Description**: Ghostty learned (the hard way) that `xterm-` prefix is critical for compatibility. Applications check terminal name prefixes.

**Status**: Important naming convention

**Lesson**:
- Crux uses `xterm-crux` (correct)
- Never use plain name like `crux` or `ghostty`
- `xterm-` prefix signals xterm compatibility level
- Applications whitelist based on TERM prefix

**Reference**: [Rio Terminfo Docs](https://github.com/raphamorim/rio/blob/main/docs/docs/install/terminfo.md)

---

## Summary: Top 20 Lessons for Crux

1. **IME Preedit MUST be overlay-only**, never sent to PTY (most critical CJK bug)
2. **CJK characters must respect 2-cell width consistently** in all contexts
3. **Use established wcwidth implementations** (unicode-width crate), don't roll your own
4. **Implement font fallback chain** for missing glyphs (especially CJK)
5. **Parser MUST handle incomplete escape sequences** across read boundaries
6. **Validate and clamp all integer parameters** in escape sequences (security)
7. **VS-16 emoji handling**: wide emoji (VS-16) = 2 cells, text emoji = 1 cell
8. **Damage tracking and render coalescing** essential for performance
9. **TERM name must use xterm- prefix** (xterm-crux) for compatibility
10. **Send SIGWINCH on resize** with correct dimensions accounting for padding
11. **Test extensively with tmux**, provide "plain mode" or keybinding compatibility
12. **Tab handling is complex**: respect custom tab stops, test at all column positions
13. **Never silently overwrite user config files**, prompt or backup first
14. **Check PTY alive state before operations** (resize, write) to avoid race conditions
15. **Block signals before fork, reset in child before exec** (signal race condition)
16. **SGR parameter omission is valid**: `CSI m` = `CSI 0 m`
17. **Run vttest, ucs-detect, and escape sequence fuzzers** regularly
18. **Code sign and notarize for macOS** to avoid XProtect performance issues
19. **Keep Unicode tables updated** (unicode-width crate updates)
20. **Follow xterm behavior for ambiguous/undefined cases** (de facto standard)

---

## Testing Recommendations

### Essential Test Suites

1. **vttest**: Comprehensive VT100/VT220/VT320/xterm testing
2. **ucs-detect**: Unicode width and rendering validation
3. **Escape sequence fuzzers**: Test with malformed/extreme sequences
4. **tmux integration tests**: Split panes, windows, status line
5. **CJK text corpus**: Chinese, Japanese, Korean sample text at various positions
6. **Emoji test suite**: Full emoji range including modifiers, VS-16, ZWJ sequences
7. **Popular applications**: vim, emacs, htop, btop, midnight commander
8. **Shell prompts**: oh-my-zsh themes, Starship, powerlevel10k

### Edge Cases to Test

- Escape sequences split across multiple reads
- Integer parameters: 0, 1, 65535, INT_MAX, negative
- Mouse reporting at window boundaries
- Window resize during: scrolling, image display, line wrapping
- Tab characters at various column positions
- Mixed Latin/CJK/emoji text at line wraps
- Font size changes with images displayed
- Rapid process spawn/kill (PTY lifecycle)
- Configuration file edge cases (empty, malformed, missing sections)

---

## References and Sources

### Rio Terminal
- [GitHub Repository](https://github.com/raphamorim/rio)
- [Official Documentation](https://rioterm.com/docs)
- [Changelog](https://rioterm.com/changelog)

### State of Terminal Emulation (2025)
- [Comprehensive Terminal Emulator Survey](https://www.jeffquast.com/post/state-of-terminal-emulation-2025/)
- [Unicode Edition Test Results](https://www.jeffquast.com/post/ucs-detect-test-results/)

### Alacritty (Reference Implementation)
- [GitHub Issues](https://github.com/alacritty/alacritty/issues)
- Uses same VT parser (alacritty_terminal) as Crux

### wcwidth and Unicode
- [wcwidth Python Library](https://pypi.org/project/wcwidth/)
- [Markus Kuhn's wcwidth.c](https://www.cl.cam.ac.uk/~mgk25/ucs/wcwidth.c)
- [unicode-width Rust crate](https://crates.io/crates/unicode-width)

### Escape Sequences
- [XTerm Control Sequences](https://www.invisible-island.net/xterm/ctlseqs/ctlseqs.html)
- [WezTerm Escape Sequences](https://wezterm.org/escape-sequences.html)

### Security
- [OSS Security: Terminal Emulator Processing](https://www.openwall.com/lists/oss-security/2017/05/01/13)
- [CyberArk: Abusing ANSI Escape Characters](https://www.cyberark.com/resources/threat-research-blog/dont-trust-this-title-abusing-terminal-emulators-with-ansi-escape-characters)

---

## Document Metadata

**Created**: 2026-02-12
**Author**: Research via librarian agent
**Sources**: 50+ GitHub issues, blog posts, documentation, security advisories
**Scope**: Rio terminal, Alacritty, Ghostty, Windows Terminal, and universal terminal emulator issues
**Relevance to Crux**: High - avoids known pitfalls, especially CJK/IME, font rendering, and tmux compatibility

**Related Crux Documents**:
- `research/platform/ime-clipboard.md` - IME architecture details
- `research/core/terminal-emulation.md` - VT emulation fundamentals
- `research/core/keymapping.md` - Keyboard input handling
- `research/core/terminfo.md` - Terminal definition
- `research/integration/ipc-protocol-design.md` - External control protocol

**Next Steps**:
1. Review this document during Phase 1 implementation
2. Add specific test cases based on identified bugs
3. Reference when implementing CJK support (Phase 3)
4. Update with new findings from community
