---
title: Terminal Bugs & Lessons Learned (iTerm2, Warp, Others)
description: Known bugs and issues (both open and resolved) in iTerm2 and Warp terminal emulators that Crux should learn from and avoid, including CVE security patches and root cause analysis
phase: all
topics: [macos, terminal-emulation, ime, performance, architecture]
related: [ime-clipboard.md, terminal-emulation.md, terminal-architecture.md]
---

# Terminal Bugs & Lessons Learned

**Research Date**: 2026-02-12

This document catalogs known bugs, design mistakes, and lessons learned from existing macOS terminal emulators (primarily iTerm2 and Warp) to inform Crux development and avoid repeating historical mistakes.

## Table of Contents

1. [CJK/IME Issues](#cjkime-issues)
2. [Performance Issues](#performance-issues)
3. [Architecture & Design Issues](#architecture--design-issues)
4. [Security Vulnerabilities](#security-vulnerabilities)
5. [macOS Integration Issues](#macos-integration-issues)
6. [Shell & SSH Compatibility](#shell--ssh-compatibility)
7. [GPU Rendering Pitfalls](#gpu-rendering-pitfalls)
8. [User Experience Anti-Patterns](#user-experience-anti-patterns)
9. [Key Takeaways for Crux](#key-takeaways-for-crux)

---

## CJK/IME Issues

### The Real Cursor vs. Fake Cursor Problem

**Issue**: The most pervasive CJK IME bug across multiple terminals (iTerm2, Warp, Alacritty, Kitty, Claude Code).

**Root Cause**: Terminal applications render a "fake cursor" in their TUI but don't move the **real terminal cursor** to the caret position. macOS IME anchors the preedit/candidate window to the real cursor position (bottom-left or last cursor location), not the visual cursor.

**Symptoms**:
- IME candidate window appears at wrong position (bottom-left corner)
- Preedit text overlay doesn't follow caret movement
- Candidate window doesn't move with left/right caret navigation

**Solution**: Move the real terminal cursor to the caret position after each render while preserving SGR state. This makes IME preedit/candidate windows follow the caret correctly.

**Crux Impact**: CRITICAL. This is documented in `research/platform/ime-clipboard.md` as a must-fix. The fix requires synchronizing real cursor with visual cursor position.

**References**:
- [Claude Code Issue #16372](https://github.com/anthropics/claude-code/issues/16372)
- [Warp Issue #6891](https://github.com/warpdotdev/warp/issues/6891)
- [Alacritty Issue #6942](https://github.com/alacritty/alacritty/issues/6942)

---

### iTerm2 CJK-Specific Bugs

#### 1. CJK Compatibility Ideographs Auto-Conversion

**Bug**: CJK compatibility ideographs automatically convert to non-compatibility counterparts.

**Status**: Long-standing issue.

**Lesson**: Don't normalize Unicode characters without user consent. Preserve exact character codes.

**Reference**: [iTerm2 Issue #5098](https://gitlab.com/gnachman/iterm2/-/issues/5098)

#### 2. Candidate Window in Fullscreen Apps

**Bug**: Input candidate window not visible when hotkey window overlays a fullscreen application.

**Root Cause**: macOS window level conflicts between fullscreen apps and floating windows.

**Reference**: [iTerm2 Issue #7521](https://gitlab.com/gnachman/iterm2/-/issues/7521)

---

### Warp CJK-Specific Bugs

#### 1. Keybindings Don't Work with Non-English IME

**Bug**: Shortcuts like `⌘I`, `⌘P` don't work when Korean/Chinese IME is active.

**Root Cause**: Keybindings rely on current input source. When Korean IME is active, `⌘P` is interpreted as `⌘ㅔ` (different key code).

**Status**: Ongoing issue (reported 2025).

**Lesson**: Keybindings should work at the physical key level, not the character level. Map shortcuts to key codes, not characters.

**Crux Impact**: HIGH. GPUI keyboard event handling must account for this.

**References**:
- [Warp Issue #341](https://github.com/warpdotdev/Warp/issues/341)
- [Warp Issue #8547](https://github.com/warpdotdev/warp/issues/8547)

#### 2. Korean Characters Display Incorrectly

**Bug**: Korean characters not rendered correctly; files with Korean names show incompletely in `ls` output.

**Status**: Reported 2025.

**Reference**: [Warp Issue #428](https://github.com/warpdotdev/warp/issues/428)

#### 3. Japanese Hiragana Preedit Not Visible

**Bug**: When typing Japanese, the hiragana being input temporarily is not displayed before conversion to kanji.

**Root Cause**: Lack of preedit overlay rendering.

**Reference**: [Warp Issue #4529](https://github.com/warpdotdev/warp/issues/4529)

#### 4. Sidebar Cannot Display Chinese Filenames

**Bug**: Warp sidebar (not terminal itself) cannot render Chinese characters in filenames correctly.

**Root Cause**: UI component-specific rendering issue.

**Lesson**: All UI components need proper Unicode/CJK support, not just the terminal canvas.

**Reference**: [Warp Issue #7436](https://github.com/warpdotdev/warp/issues/7436)

---

### Alacritty & Kitty CJK Issues

#### Alacritty: Double Space on Korean Input

**Bug**: When in Korean IME mode, pressing spacebar once results in two spaces.

**Status**: Reported 2024.

**Reference**: [Alacritty Issue #8079](https://github.com/alacritty/alacritty/issues/8079)

#### Kitty: GLFW IME Limitations

**Bug**: IME doesn't work with Kitty due to GLFW limitations (on X11).

**Workaround**: Works on Wayland with certain IME systems (kime).

**Lesson**: Graphics framework choice affects IME support. GLFW has known IME limitations.

**Reference**: [Kitty Issue #462](https://github.com/kovidgoyal/kitty/issues/462)

---

## Performance Issues

### iTerm2 Performance Problems

#### 1. CPU Rendering Bottleneck

**Issue**: iTerm2 is CPU-rendered, fundamentally slower than GPU-accelerated terminals.

**Measured Impact**: macOS Core Text rendering takes **>150ms per frame** on 4K displays on some MacBook models.

**Result**: Visible lag, especially with tmux and vim. Fans spin up on MacBook Pro.

**Lesson**: GPU acceleration is essential for modern terminal performance. CPU rendering doesn't scale to high-DPI displays.

**Crux Impact**: CRITICAL. Validates Crux's Metal rendering choice.

**References**:
- [iTerm2 Issue #7333](https://gitlab.com/gnachman/iterm2/-/issues/7333)
- [Making iTerm2 Render Faster](https://vivi.sh/blog/technical/making-terminal-render-faster/index)
- [Hacker News Discussion](https://news.ycombinator.com/item?id=14800195)

#### 2. Slow Startup Times

**Bug**: iTerm2 can take 5-10 seconds to show prompt on startup.

**Common Causes**:
- Apple system logs (`/private/var/log/asl/*.asl`) accumulation
- Slow shell initialization
- Profile misconfiguration

**Lesson**: Terminal startup should be instant. Profile loading and shell spawning must be optimized.

**References**:
- [iTerm2 Issue #7982](https://gitlab.com/gnachman/iterm2/-/issues/7982)
- [iTerm2 Issue #9872](https://gitlab.com/gnachman/iterm2/-/issues/9872)

#### 3. Memory Leaks

**Bug**: Multiple reports of memory leaks, with iTerm2 consuming 5-9GB RAM.

**Common Cause**: Unlimited scrollback buffer.

**Status**: Ongoing issue across versions.

**Lesson**: Scrollback must have bounded memory usage. Consider ring buffer with configurable limits and memory-mapped storage for large buffers.

**References**:
- [iTerm2 Issue #10766](https://gitlab.com/gnachman/iterm2/-/issues/10766)
- [iTerm2 Issue #9221](https://gitlab.com/gnachman/iterm2/-/issues/9221)
- [iTerm2 Issue #8592](https://gitlab.com/gnachman/iterm2/-/issues/8592)

#### 4. Performance Degradation Over Time

**Bug**: iTerm2 becomes slow over time on macOS High Sierra and later.

**Lesson**: Profile for memory leaks and resource accumulation. Long-running sessions must maintain performance.

**Reference**: [iTerm2 Issue #6939](https://gitlab.com/gnachman/iterm2/-/issues/6939)

---

### Warp Performance Problems

#### 1. Discrete GPU Battery Drain

**Bug**: On dual-GPU MacBooks, Warp forces high-power discrete GPU usage even when idle, rapidly draining battery.

**Root Cause**: Background window re-rendering for cursor blinking.

**Workaround**: Setting "Prefer rendering new windows with integrated GPU" in preferences.

**Lesson**: GPU usage must be power-aware. Idle terminals should use integrated GPU. Cursor blinking should not trigger full window re-renders.

**Crux Impact**: HIGH. GPUI rendering loop must be power-efficient.

**References**:
- [Warp Issue #76](https://github.com/warpdotdev/Warp/issues/76)
- [Warp Issue #2114](https://github.com/warpdotdev/Warp/issues/2114)
- [Warp Issue #2223](https://github.com/warpdotdev/Warp/issues/2223)

#### 2. Excessive CPU/GPU While Idle (Windows)

**Bug**: On Windows, Warp uses excessive CPU/GPU while idle.

**Status**: Reported 2025.

**Lesson**: Rendering should pause or drastically reduce when terminal is idle.

**Reference**: [Warp Issue #7561](https://github.com/warpdotdev/warp/issues/7561)

#### 3. Memory Leak

**Bug**: Warp allocated ~9.5GB virtual memory.

**Status**: Reported 2025.

**Reference**: [Warp Issue #7101](https://github.com/warpdotdev/Warp/issues/7101)

#### 4. Crashes Under Heavy Load

**Bug**: Terminal crashes when heavy processing occurs, especially over networks with multiple tabs running agents.

**Status**: Reported 2025.

**Lesson**: Terminal must be resilient to high I/O throughput. Background threads/async I/O essential.

**Reference**: [Warp Issue #8280](https://github.com/warpdotdev/warp/issues/8280)

---

## Architecture & Design Issues

### Warp: Block Mode Workflow Conflicts

**Issue**: Warp's "block" UI paradigm (treating command output as discrete UI elements) conflicts with traditional terminal workflows.

**User Complaints**:
- Blocks consume excessive screen real estate even in "compact mode"
- Mouse input captured by blocks, requiring "Cmd+U" to refocus
- Cursor position unclear in Rails console and REPLs
- Text deletion doesn't work as expected
- Incompatible with tmux/vim power users

**Lesson**: Don't force new paradigms on terminal users. Traditional continuous character grid model exists for a reason. UI innovations should be **optional**, not mandatory.

**Crux Impact**: MEDIUM. Crux should remain a traditional terminal emulator. Advanced features (IPC, MCP) are **extensions**, not replacements for core terminal behavior.

**Reference**: [Warp Issue #3189](https://github.com/warpdotdev/Warp/issues/3189)

---

### iTerm2: Configuration Complexity

**Issue**: iTerm2 has overwhelming configuration options, including "hidden settings" only accessible via terminal commands.

**User Impact**:
- Difficult to optimize without advanced knowledge
- Breaking changes in profiles can cause iTerm2 to crash on startup
- No clear path from defaults to optimized setup

**Lesson**: Provide sane defaults that work well out-of-the-box. Advanced settings should be discoverable, not hidden.

**Crux Impact**: MEDIUM. Crux config system (Phase 5) should follow Ghostty's approach: single config file, clear documentation, sensible defaults.

**References**:
- [iTerm2 Hidden Settings](https://iterm2.com/documentation-hidden-settings.html)
- [iTerm2 Configuration Guide](https://www.oreateai.com/blog/complete-guide-to-iterm2-configuration-and-optimization/0a929df5ac1b584712330cebe1bef982)

---

### Warp: Proprietary Lock-In & Privacy Backlash

**Issue**: Warp initially required login/account creation to use the terminal, generating massive developer backlash.

**User Concerns**:
- Privacy: Why does a local terminal need cloud authentication?
- Telemetry: What data is being sent to Segment?
- Trust: Terminal handles sensitive commands and credentials
- Philosophy: Goes against open-source ethos

**Warp's Response**: Removed login requirement in November 2024, made telemetry opt-in.

**Lesson**: Developer tools must respect privacy by default. Never require accounts for local-first applications. Telemetry must be opt-in and transparent.

**Crux Impact**: LOW (Crux is open-source, local-first). But: MCP server in Phase 6 must be privacy-conscious.

**References**:
- [Warp: Lifting the Login Requirement](https://www.warp.dev/blog/lifting-login-requirement)
- [Warp Issue #1838](https://github.com/warpdotdev/Warp/issues/1838)
- [Hacker News Discussion](https://news.ycombinator.com/item?id=42247583)

---

## Security Vulnerabilities

### iTerm2: Critical tmux Integration RCE (CVE-2019-9535)

**Severity**: CRITICAL

**Bug**: Remote code execution via tmux integration feature.

**Attack Vector**: Attacker-controlled SSH server or malicious output (e.g., `curl http://attacker.com` or `tail -f /var/log/apache2/referer_log`) can execute commands on user's computer.

**Root Cause**: Insufficient sanitization of escape sequences in tmux integration mode.

**Fix**: Patched in iTerm2 3.3.6 (2019).

**Lesson**: **ALL escape sequence parsing must assume hostile input.** Never trust terminal output, even from "trusted" sources. Implement strict validation and sandboxing.

**Crux Impact**: CRITICAL. VT parser (`alacritty_terminal`) must be audited for injection vulnerabilities. Custom OSC/DCS handlers for IPC (Phase 2) need security review.

**References**:
- [Mozilla Security Blog](https://blog.mozilla.org/security/2019/10/09/iterm2-critical-issue-moss-audit/)
- [CISA Alert](https://www.cisa.gov/news-events/alerts/2019/10/09/iterm2-vulnerability)
- [Hacker News Discussion](https://news.ycombinator.com/item?id=21203564)

---

### iTerm2: OSC 8 URL Handler Exploitation (2024)

**Severity**: HIGH

**Bug**: Vulnerability in OSC 8 URL handling allowing arbitrary application launch via URL schemes.

**Attack Vector**: Terminal output containing crafted OSC 8 sequences can pop Calculator or other apps.

**Additional Bug**: SSH URL scheme argument injection allowing arbitrary file write.

**Lesson**: URL handlers and custom protocol schemes are high-risk attack surfaces. Implement strict allowlists and sanitization.

**Crux Impact**: MEDIUM. Phase 4 includes clickable links. Must use macOS URL validation APIs and allowlist safe schemes (http, https, file).

**Reference**: [Security Researcher Blog](https://vin01.github.io/piptagole/escape-sequences/iterm2/hyper/url-handlers/code-execution/2024/05/21/arbitrary-url-schemes-terminal-emulators.html)

---

## macOS Integration Issues

### iTerm2: Fullscreen & Spaces Management

**Bugs**:
1. Fullscreen window remains on desktop space instead of moving to new fullscreen space
2. Fullscreen side-by-side windows resize incorrectly after wake
3. Hotkey window no longer opens over fullscreen apps (macOS 11.1+)
4. Fullscreen windows get attached to specific spaces

**Root Cause**: macOS window level API changes and Mission Control integration complexity.

**Lesson**: macOS fullscreen behavior is fragile. Test across macOS versions. Provide both native fullscreen (NSWindowCollectionBehaviorFullScreenPrimary) and instant fullscreen options.

**Crux Impact**: MEDIUM. Window management is Phase 2 scope.

**References**:
- [iTerm2 Issue #7432](https://gitlab.com/gnachman/iterm2/-/issues/7432)
- [iTerm2 Issue #9404](https://gitlab.com/gnachman/iterm2/-/issues/9404)
- [iTerm2 Issue #1138](https://gitlab.com/gnachman/iterm2/-/issues/1138)

---

### iTerm2: Shell Integration OSC Compatibility

**Issues**:
1. Proprietary escape codes don't work in tmux/screen
2. Multiline commands not supported (shell-specific PS2 differences)
3. Shell integration appears as literal text in some configurations

**Lesson**: Shell integration via escape sequences is fragile. Document limitations. Provide fallbacks when integration unavailable.

**Crux Impact**: MEDIUM. Phase 2 includes shell integration. Use existing standards (OSC 7, OSC 133) where possible.

**References**:
- [iTerm2 Proprietary Escape Codes](https://iterm2.com/documentation-escape-codes.html)
- [iTerm2 Issue #7138](https://gitlab.com/gnachman/iterm2/-/issues/7138)

---

### iTerm2: tmux Integration Bugs

**Known Issues**:
1. Tab with tmux window cannot contain non-tmux split panes (design limitation)
2. Aggressive-resize option incompatible with tmux integration
3. Panel management bugs when killing sessions with multiple split panes
4. SSH tmux sessions occasionally lose individual windows
5. Compatibility broken with tmux 3.3+

**Lesson**: Deep tmux integration is complex and fragile. Phase 5 tmux compatibility should focus on **standard tmux protocol**, not custom integration layer.

**References**:
- [iTerm2 tmux Integration Docs](https://iterm2.com/documentation-tmux-integration.html)
- [iTerm2 Issue #10435](https://gitlab.com/gnachman/iterm2/-/issues/10435)
- [iTerm2 Issue #7770](https://gitlab.com/gnachman/iterm2/-/issues/7770)

---

### Warp: macOS Local Network Permission

**Bug**: On macOS, SSH fails with "Undefined error: 0" due to missing Local Network permission.

**Root Cause**: macOS 14+ requires explicit Local Network permission for apps to access LAN devices.

**Fix**: System Settings > Privacy & Security > Local Network > Enable Warp.

**Lesson**: Modern macOS requires explicit privacy permissions. Terminal emulators need Local Network entitlement for SSH. Document permission requirements clearly.

**Crux Impact**: LOW. Standard macOS entitlement. Add to distribution checklist (Phase 6).

**Reference**: [Warp Known Issues](https://docs.warp.dev/support-and-billing/known-issues)

---

## Shell & SSH Compatibility

### Warp: SSH Connection Issues (2025-2026)

**Bugs**:
1. Random SSH connection disconnects without user action
2. SSH login hangs (works fine in standard terminals)
3. Port forwarding causes session termination on connection refused
4. TailScale SSH connections loop indefinitely
5. General SSH instability with "SSH Wrapper" for block mode

**Root Cause**: Warp's SSH wrapper for "Blocks over SSH" feature interferes with standard SSH behavior.

**Lesson**: Don't intercept/wrap SSH. It's a complex protocol with many edge cases. Let SSH work transparently.

**Crux Impact**: LOW. Crux is a standard terminal emulator. SSH just works via PTY.

**References**:
- [Warp Issue #8111](https://github.com/warpdotdev/warp/issues/8111)
- [Warp Issue #6183](https://github.com/warpdotdev/warp/issues/6183)
- [Warp Issue #7641](https://github.com/warpdotdev/warp/issues/7641)

---

### Warp: Shell Compatibility Issues

**Issues**:
1. Fish shell crashes on startup
2. Remote fish shell initialization errors (bash/zsh init sent to fish)
3. Aliases don't transfer over SSH
4. Incompatibilities with oh-my-fish, oh-my-bash
5. Multi-line/right-sided prompts only supported in zsh/fish, not bash

**Root Cause**: Warp injects shell-specific initialization code, which breaks when shell type detection fails or custom prompts interfere.

**Lesson**: Shell initialization must be robust and shell-agnostic. Don't assume shell type. Parse prompt strictly or use fallback rendering.

**Crux Impact**: MEDIUM. Phase 2 shell integration should use standard APIs (OSC 133, OSC 7) rather than shell-specific injection.

**References**:
- [Warp Supported Shells](https://docs.warp.dev/getting-started/supported-shells)
- [Warp Issue #5142](https://github.com/warpdotdev/Warp/issues/5142)
- [Warp Issue #2454](https://github.com/warpdotdev/Warp/issues/2454)

---

### iTerm2: Semantic History Bugs

**Issues**:
1. No feedback when semantic history command fails
2. Doesn't work in tmux
3. Network share support causes hangs (slow filesystem blocks entire app)
4. Soft boundaries not respected for prefix/suffix text

**Lesson**: File/URL detection is heuristic and error-prone. Semantic history on network mounts needs background threads to prevent UI hangs.

**Crux Impact**: LOW (Phase 4 feature). Implement async URL/file detection.

**References**:
- [iTerm2 Issue #7915](https://gitlab.com/gnachman/iterm2/-/issues/7915)
- [iTerm2 Issue #7464](https://gitlab.com/gnachman/iterm2/-/issues/7464)
- [iTerm2 Issue #8227](https://gitlab.com/gnachman/iterm2/-/issues/8227)

---

## GPU Rendering Pitfalls

### Warp: Metal API Complexity

**Finding**: Warp chose Metal over OpenGL for macOS.

**Challenge**: Metal is low-level, only allows rendering triangles via shaders or texture sampling. No high-level text/UI APIs.

**Implication**: Custom text rendering, glyph caching, and UI element rendering must be implemented from scratch.

**Benefit**: Better performance potential (400+ fps vs CPU rendering). Better Xcode debugging tools.

**Lesson**: Metal enables high performance but requires significant implementation effort. GPUI abstracts this complexity for Crux.

**Crux Impact**: LOW. GPUI handles Metal text rendering. But: understand performance characteristics and profiling tools.

**Reference**: [Warp: How Warp Works](https://www.warp.dev/blog/how-warp-works)

---

### Ghostty: Metal Rendering Lessons (2025-2026)

**Recent Bugs**:
1. Control-modified keys broken with Kitty Keyboard protocol (Neovim, Fish 4.0)
2. Critical deadlock scenario (all platforms)
3. macOS 26 (Tahoe) tab bar incompatibilities

**Key Lesson**: Tab title override logic needed in core, not per-platform. Implementing separately for macOS and GTK is tedious and error-prone.

**Crux Impact**: MEDIUM. Phase 2 tab implementation should centralize state management in Rust core, not per-platform UI code.

**References**:
- [Ghostty 1.2.3 Release Notes](https://ghostty.org/docs/install/release-notes/1-2-3)
- [Ghostty Issue #10345](https://github.com/ghostty-org/ghostty/issues/10345)

---

### Font Rendering: Ligature Performance

**Issue**: Ligatures add rendering complexity and performance overhead.

**iTerm2**: Ligatures use CoreText (significantly slower than Core Graphics).

**Warp**: Ligatures not supported by GPU renderer, would reduce performance.

**Lesson**: Ligature support is a trade-off. If implemented, needs glyph caching and GPU-friendly rendering path. Consider making ligatures optional.

**Crux Impact**: LOW (not Phase 1). If added later, use GPUI's text shaping APIs carefully.

**References**:
- [iTerm2 Fonts Documentation](https://iterm2.com/documentation-fonts.html)
- [Font Shaping Support Gist](https://gist.github.com/XVilka/070ed8b1c1186097cad65ef49220175a)

---

## User Experience Anti-Patterns

### Clipboard Paste Issues

**iTerm2 Bugs**:
1. Paste doubles clipboard content
2. Large pastes lose characters
3. URL escaping issues on paste

**Lesson**: Clipboard handling needs careful testing. Large paste should be throttled or chunked to prevent PTY buffer overflow.

**Crux Impact**: MEDIUM. Phase 3 clipboard implementation must handle large pastes gracefully.

**References**:
- [iTerm2 Issue #11228](https://gitlab.com/gnachman/iterm2/-/issues/11228)
- [iTerm2 Issue #4789](https://gitlab.com/gnachman/iterm2/-/issues/4789)

---

### Warp: Crashes on Startup

**Bugs**:
1. Crashes if command executed immediately after launch with anaconda auto-activate
2. Terminal breaks on upgrade (commands dump shell code on screen)
3. Gets stuck on "Warping" indefinitely, losing work

**Lesson**: Initialization race conditions are common. Terminal must gracefully handle rapid input during startup.

**Crux Impact**: LOW. Ensure PTY initialization completes before accepting input.

**References**:
- [Warp Issue #7898](https://github.com/warpdotdev/warp/issues/7898)
- [Warp Issue #7090](https://github.com/warpdotdev/warp/issues/7090)
- [Warp Issue #7801](https://github.com/warpdotdev/warp/issues/7801)

---

## Key Takeaways for Crux

### Critical (Phase 1)

1. **Real Cursor Sync for IME**: Move real cursor to visual cursor position after each render. This is THE solution to CJK IME positioning. (See `ime-clipboard.md`)

2. **GPU Acceleration Validated**: iTerm2's CPU rendering proves GPU is essential for modern terminals. Metal via GPUI is the right choice.

3. **Security-First Escape Sequence Parsing**: iTerm2's CVE-2019-9535 shows escape sequences are attack surfaces. Never trust terminal output. Audit `alacritty_terminal` parser for injection risks.

4. **Power-Efficient Rendering**: Warp's discrete GPU battery drain shows importance of idle optimization. Cursor blinking must not trigger full re-renders.

### High Priority (Phase 2-3)

5. **Keybindings at Physical Key Level**: Warp's IME keybinding bugs show shortcuts must map to key codes, not characters. GPUI keyboard events must handle this correctly.

6. **Bounded Scrollback Memory**: iTerm2 memory leaks demonstrate need for ring buffer with configurable limits. Consider memory-mapped storage for large buffers.

7. **Async I/O for High Throughput**: Warp crashes under heavy load show terminal must handle high I/O volume gracefully. Background threads essential.

8. **Standard SSH/tmux Support**: Don't intercept or wrap. Just provide transparent PTY. Complex integration layers (like Warp's SSH wrapper or iTerm2's tmux integration) are fragile.

9. **Clipboard Security**: Phase 3 rich clipboard needs careful sanitization. Large pastes need throttling.

### Medium Priority (Phase 4-5)

10. **URL Handler Security**: Phase 4 clickable links need strict allowlists. OSC 8 is an attack surface.

11. **Shell Integration via Standards**: Use OSC 7 (directory), OSC 133 (command marks), not custom injection. Works across shells.

12. **Fullscreen Testing Across macOS Versions**: macOS window management APIs change between versions. Test on multiple macOS releases.

13. **Configuration Philosophy**: Follow Ghostty model: single config file, sane defaults, clear docs. Avoid iTerm2's "hidden settings" complexity.

### Low Priority (Phase 6)

14. **Ligatures Are Optional**: If added, needs GPU-friendly rendering. Not essential for Phase 1.

15. **Privacy by Default**: MCP server must be local-first. No required accounts. Telemetry opt-in.

16. **macOS Entitlements**: Local Network permission required for SSH. Add to distribution checklist.

---

## Recommendations for PLAN.md Updates

Based on this research, suggest adding to `PLAN.md`:

### Phase 1 Additions

- [ ] Test real cursor synchronization with Korean IME (Hangul 2-Set)
- [ ] Profile GPU memory usage and power consumption (discrete vs integrated)
- [ ] Security audit of `alacritty_terminal` escape sequence parser

### Phase 2 Additions

- [ ] Implement bounded scrollback with memory-mapped overflow
- [ ] Add async PTY I/O handling with backpressure
- [ ] Test tab title state management (centralized in Rust, not per-platform)
- [ ] Verify keybindings work with CJK IME active

### Phase 3 Additions

- [ ] Implement clipboard paste throttling for large content (>1MB)
- [ ] Add unit tests for IME preedit rendering at various cursor positions

### Phase 4 Additions

- [ ] Implement URL scheme allowlist (http, https, file only)
- [ ] Add async file existence checks for semantic history (prevent UI hangs on network mounts)

### Phase 5 Additions

- [ ] Test tmux compatibility without custom integration layer
- [ ] Document shell integration escape sequence support (OSC 7, OSC 133)

### Phase 6 Additions

- [ ] Add Local Network entitlement to code signing
- [ ] Document privacy policy for MCP server (local-first, no telemetry)

---

## Sources

### iTerm2 Issues
- [CJK Compatibility Ideographs](https://gitlab.com/gnachman/iterm2/-/issues/5098)
- [Candidate Window Fullscreen](https://gitlab.com/gnachman/iterm2/-/issues/7521)
- [Performance Issues](https://gitlab.com/gnachman/iterm2/-/issues/7333)
- [Slow Startup](https://gitlab.com/gnachman/iterm2/-/issues/7982)
- [Memory Leak](https://gitlab.com/gnachman/iterm2/-/issues/10766)
- [tmux Integration](https://gitlab.com/gnachman/iterm2/-/issues/10435)
- [Fullscreen Spaces](https://gitlab.com/gnachman/iterm2/-/issues/7432)
- [Semantic History](https://gitlab.com/gnachman/iterm2/-/issues/7915)

### Warp Issues
- [IME Support](https://github.com/warpdotdev/warp/issues/6891)
- [Keybinding IME](https://github.com/warpdotdev/warp/issues/341)
- [Discrete GPU Battery](https://github.com/warpdotdev/warp/issues/76)
- [SSH Issues](https://github.com/warpdotdev/warp/issues/8111)
- [Block Mode Complaints](https://github.com/warpdotdev/warp/issues/3189)
- [Login Requirement](https://github.com/warpdotdev/warp/issues/1838)

### Security
- [Mozilla iTerm2 CVE](https://blog.mozilla.org/security/2019/10/09/iterm2-critical-issue-moss-audit/)
- [CISA iTerm2 Alert](https://www.cisa.gov/news-events/alerts/2019/10/09/iterm2-vulnerability)
- [URL Handler Vulnerability](https://vin01.github.io/piptagole/escape-sequences/iterm2/hyper/url-handlers/code-execution/2024/05/21/arbitrary-url-schemes-terminal-emulators.html)

### Architecture
- [Warp How It Works](https://www.warp.dev/blog/how-warp-works)
- [Claude Code IME Issue](https://github.com/anthropics/claude-code/issues/16372)
- [Making iTerm2 Faster](https://vivi.sh/blog/technical/making-terminal-render-faster/index)

### Ghostty
- [Ghostty 1.2.3 Release](https://ghostty.org/docs/install/release-notes/1-2-3)
- [Ghostty Issue #10345](https://github.com/ghostty-org/ghostty/issues/10345)

### Other Terminals
- [Alacritty Issue #6942](https://github.com/alacritty/alacritty/issues/6942)
- [Kitty Issue #462](https://github.com/kovidgoyal/kitty/issues/462)

---

## Resolved Issues — Root Cause & Fix Analysis

> 아래는 iTerm2와 Warp에서 **해결된** 주요 버그들의 근본 원인과 수정 방법 분석이다.
> CVE 보안 패치와 코드 레벨 교훈을 포함한다.

**Research Date**: 2026-02-12
**Sources**: iTerm2 GitLab, Warp GitHub, CVE databases, Mozilla Security Blog

---

## Table of Contents (Resolved Issues)

1. [Critical Security Vulnerabilities](#critical-security-vulnerabilities-1)
2. [Rendering & GPU Issues](#rendering--gpu-issues-1)
3. [IME & International Input](#ime--international-input-1)
4. [Memory Management](#memory-management-1)
5. [Performance & Threading](#performance--threading-1)
6. [Font Rendering](#font-rendering-1)
7. [Protocol & Integration](#protocol--integration-1)
8. [Lessons for Crux](#lessons-for-crux-1)

---

## Critical Security Vulnerabilities

### 1. CVE-2019-9535: iTerm2 tmux RCE (7 years latent)

**Severity**: CRITICAL (CVSS unspecified, RCE)
**Affected**: iTerm2 ≤3.3.5 (2012-2019)
**Fixed**: iTerm2 3.3.6

#### Bug Description
Attackers could execute arbitrary commands by providing malicious output to the terminal when tmux integration (`tmux -CC`) was enabled (default).

#### Root Cause
Improper neutralization of special elements in output used by downstream components (CWE-94: Code Injection). iTerm2's tmux control mode integration did not sanitize escape sequences before interpreting them as shell commands.

#### Attack Vector
1. User connects to malicious SSH server OR
2. User runs `curl http://attacker.com` OR
3. User runs `tail -f /var/log/apache2/referer_log` on compromised log

Malicious escape sequences in output → tmux integration parses → shell command execution.

#### Fix
- Mozilla-sponsored security audit identified the flaw
- Patch sanitizes escape sequences in tmux control mode
- Commands from tmux output now validated before execution

#### Lesson for Crux
- **CRITICAL**: Sanitize ALL escape sequences before processing, especially in:
  - IPC protocol handlers (`crux-ipc`)
  - tmux integration (Phase 5)
  - Shell integration (Phase 2)
- Never trust terminal output as safe input for command execution
- Security audit before 1.0 release
- Add fuzzing for escape sequence parser

**References**:
- [Mozilla Security Blog](https://blog.mozilla.org/security/2019/10/09/iterm2-critical-issue-moss-audit/)
- [NVD CVE-2019-9535](https://nvd.nist.gov/vuln/detail/CVE-2019-9535)
- [BleepingComputer Coverage](https://www.bleepingcomputer.com/news/security/iterm2-patches-critical-vulnerability-active-for-7-years/)

---

### 2. CVE-2024-38396: iTerm2 Window Title Injection RCE

**Severity**: CRITICAL (CVSS 9.8)
**Affected**: iTerm2 3.5.0–3.5.1 only (regression)
**Fixed**: iTerm2 3.5.2

#### Bug Description
Crafted escape sequence could inject arbitrary code via window title reporting, combined with tmux integration for automatic execution (no Enter required).

#### Root Cause
**Regression bug**: Window title reporting was disabled in <3.5.0 for security reasons, but 3.5.0 re-enabled it without proper sanitization.

Two-step exploit:
1. **Title Setting**: `\033]0;malicious_content\a` sets window title
2. **Title Retrieval**: `CSI Ps 21 t` retrieves title → injects into stdin → executes on Enter

With tmux integration: automatic newlines → no user interaction needed.

#### Technical Details (from researcher Vin01)
```
# Step 1: Set malicious title
echo -e '\033]0;$(whoami > /tmp/pwned)\a'

# Step 2: Trigger title report (normally requires Enter)
echo -e '\033[21t'

# With tmux integration: automatic execution
```

The tmux integration "sneaked in the reported title and also provided a way to send newlines after the title was reported," automating code execution.

#### Fix
Two patches:
- **f1e89f78**: Disabled title reporting by default
- **fc60236a**: Patched tmux integration to prevent automatic newline injection

#### Lesson for Crux
- **CRITICAL**: Never enable window title reporting (OSC 21 t) by default
- If implementing title reporting:
  - Require explicit user opt-in
  - Sanitize retrieved titles before injection into command stream
  - Never auto-execute (no automatic newlines)
- Regressions are dangerous: security features must survive refactors
- Document security-sensitive escape sequences in `research/core/terminal-emulation.md`

**References**:
- [Vin01 Technical Blog (detailed exploit)](https://vin01.github.io/piptagole/escape-sequences/iterm2/rce/2024/06/16/iterm2-rce-window-title-tmux-integration.html)
- [NVD CVE-2024-38396](https://nvd.nist.gov/vuln/detail/CVE-2024-38396)
- [Threat Intelligence Lab Analysis](https://threatintelligencelab.com/blog/cve-2024-38396-a-critical-vulnerability-in-iterm2/)

---

## Rendering & GPU Issues

### 3. iTerm2: Metal Renderer + Transparency = Ghosted Characters

**Severity**: Medium (visual corruption)
**Affected**: iTerm2 3.2+ with Metal + transparency enabled
**Status**: Partially fixed, performance trade-offs remain

#### Bug Description
When Metal GPU rendering + window transparency both enabled: ghosted text appears in background, doesn't scroll/move. Darker semi-transparent rectangles incorrectly overlay inactive panes.

#### Root Cause
Metal renderer uses a more complex blending algorithm when transparency is enabled. The GPU renderer has to composite:
1. Background transparency/blur
2. Text rendering
3. Inactive pane dimming

The blending pipeline doesn't correctly clear previous frame's text from the background layer before compositing the next frame.

#### Fix
- Improved blending algorithm in Metal renderer
- Performance trade-off: transparency + blur = slower (especially Retina)
- Recommendation: disable transparency for best performance

#### Performance Notes
From iTerm2 docs:
- Metal renderer: 60 FPS by default, can reduce to 30 FPS ("Maximize throughput")
- Transparency + blur on Retina: significant performance penalty
- GPU renderer disabled when unplugged from power (battery saving)

#### Lesson for Crux
- **GPUI canvas rendering** in `crux-terminal-view`:
  - Test transparency + Metal compositing early
  - Implement proper frame buffer clearing between draws
  - Use damage tracking to minimize redraws (we already have this from `alacritty_terminal`)
- Performance considerations:
  - Offer "high performance" vs "battery saver" modes
  - Auto-disable GPU effects when on battery (optional)
  - Document performance implications of transparency

**References**:
- [iTerm2 GitLab #7095](https://gitlab.com/gnachman/iterm2/-/issues/7095)
- [iTerm2 Metal Renderer Wiki](https://gitlab.com/gnachman/iterm2/-/wikis/Metal-Renderer)
- [Hacker News Discussion](https://news.ycombinator.com/item?id=17634547)

---

### 4. iTerm2: Metal Renderer Forces Discrete GPU (Battery Drain)

**Severity**: Medium (battery life)
**Affected**: Dual-GPU MacBooks with iTerm2 3.2+
**Fixed**: iTerm2 added "Allow Metal in low power mode" option

#### Bug Description
On MacBooks with discrete + integrated GPUs, Metal renderer forces system to use high-power discrete GPU even when idle, draining battery significantly.

#### Root Cause
Metal API defaults to requesting "high-performance" GPU. iTerm2 didn't specify `MTLCreateSystemDefaultDevice()` preference for integrated GPU.

#### Fix
- Added preference: "Allow Metal rendering when disconnected from power"
- Added "Low power mode" that prefers integrated GPU
- Automatically fall back to CPU rendering when on battery (optional)

#### Lesson for Crux
- **GPUI framework** likely handles this, but verify:
  - Check GPUI's Metal device selection strategy
  - Test battery drain on dual-GPU MacBooks (M1/M2/M3 Pro/Max)
  - Implement preference for integrated vs discrete GPU
- macOS provides `NSProcessInfo.processInfo.isLowPowerModeEnabled`
  - Consider auto-switching rendering strategy

**References**:
- [iTerm2 GitLab #6587](https://gitlab.com/gnachman/iterm2/-/issues/6587)
- [iTerm2 GitLab #10671](https://gitlab.com/gnachman/iterm2/-/issues/10671)

---

### 5. Warp: GPU Battery Drain (Excessive Power Consumption)

**Severity**: High (battery life, user experience)
**Affected**: Warp on macOS (all versions, ongoing)
**Status**: Partial mitigations in 2024, still problematic

#### Bug Description
- Warp forces discrete GPU on dual-GPU MacBooks even when idle
- After disconnecting 4K/5K external display, Warp's `stable` process prevents macOS from switching back to integrated GPU
- Idle Warp consumes excessive power, draining battery rapidly

#### Root Cause
Aggressive GPU rendering pipeline keeps discrete GPU active. Possible causes:
- Continuous background rendering (animations, cursor blink, etc.)
- GPU context not released when idle
- Display mode change not triggering GPU re-evaluation

#### Fix (Partial)
No complete fix in search results. Users report workarounds:
- Quit and relaunch Warp after display disconnect
- Force integrated GPU via third-party tools

#### Lesson for Crux
- **GPUI idle behavior**: Ensure GPU context is released when idle
- Implement proper GPU switching on display configuration changes
- Monitor GPU usage in Activity Monitor during development
- Test specifically:
  - Idle terminal (no output)
  - After external display disconnect
  - Cursor blink animation (should NOT keep discrete GPU active)

**References**:
- [Warp GitHub #76](https://github.com/warpdotdev/Warp/issues/76)
- [Warp GitHub #2223](https://github.com/warpdotdev/Warp/issues/2223)
- [Warp GitHub #4841](https://github.com/warpdotdev/Warp/issues/4841)

---

### 6. Warp: Crashes on Launch (Graphics Drivers)

**Severity**: Critical (unusable)
**Affected**: Nvidia 572.xx, AMD 23.10.x+ drivers
**Fixed**: Workaround with backend switching

#### Bug Description
Warp crashes immediately on launch when using specific Nvidia/AMD driver versions. Users reported "numerous crashes per session" under heavy load with multiple tabs.

#### Root Cause
Graphics driver incompatibility with Warp's GPU rendering pipeline (likely Vulkan or DirectX 12 issues on Windows, Metal issues on macOS).

#### Fix
- Added ability to force graphics backend selection: Vulkan, OpenGL, or DX12
- Improved crash handling for broken link rendering in Agent Mode
- Fixed crash with Unicode characters in file paths (text layout didn't expect BOM marker)

#### Lesson for Crux
- **GPUI is macOS-only (Metal)**, so driver compatibility is simpler
- Still test on:
  - Various macOS versions (13+)
  - Intel vs Apple Silicon
  - Different GPU generations (Intel Iris, M1/M2/M3)
- Implement graceful fallback if Metal initialization fails
- Add diagnostic logging for GPU initialization failures

**References**:
- [Warp GitHub #6099](https://github.com/warpdotdev/Warp/issues/6099)
- [Warp GitHub #7898](https://github.com/warpdotdev/Warp/issues/7898)
- [Warp Docs: Known Issues](https://docs.warp.dev/support-and-billing/known-issues)

---

## IME & International Input

### 7. iTerm2: Emoji Variant Selector (0xFE0F) Console Confusion

**Severity**: Medium (rendering corruption)
**Affected**: iTerm2 (multiple versions)
**Fixed**: Improved emoji variant handling

#### Bug Description
Emojis composed with Unicode variant selector 0xFE0F (emoji presentation) caused console confusion. Examples:
- ⚠️ (U+26A0 + U+FE0F) displayed as single-width when it should be double-width
- Emoji characters had transparent background and overlapped adjacent cells

#### Root Cause
Two conflicting standards:
1. **Unicode Consortium**: Emoji are NOT listed as double-width
2. **Apple rendering**: Emoji are rendered as East Asian Wide (double-width)

iTerm2 defaulted to Unicode 8.0 widths (single-width), but macOS CoreText renders them double-width → visual mismatch → cell overlap.

#### Fix
- Starting Unicode 9.0: emoji should be treated as East Asian Wide
- iTerm2 added escape sequence to switch Unicode version for width calculations
- Later versions default to Unicode 9.0+ behavior

#### Lesson for Crux
- **East Asian Width** is critical for CJK/emoji (Phase 3 priority!)
- Must query actual rendered glyph width from CoreText, not just Unicode data
- Test cases:
  - `⚠️` (warning sign + variant selector) = 2 cells
  - `⚠` (warning sign without selector) = 1 cell
  - Mixed emoji in CJK text
- See `research/platform/ime-clipboard.md` for related IME issues
- Use `unicode-width` crate with emoji support OR query CoreText

**References**:
- [iTerm2 GitLab #5003](https://gitlab.com/gnachman/iterm2/-/issues/5003)
- [iTerm2 GitLab #7938](https://gitlab.com/gnachman/iterm2/-/issues/7938)
- [iTerm2 GitLab #7239](https://gitlab.com/gnachman/iterm2/-/issues/7239)
- [Hacker News: Terminal Emoji Support](https://news.ycombinator.com/item?id=30113521)

---

### 8. Warp: CJK Keybindings Don't Trigger on Non-US Input Sources

**Severity**: High (broken for international users)
**Affected**: Warp (all versions, still open as of 2024)
**Status**: **NOT FIXED** (ongoing issue since Nov 2021)

#### Bug Description
Keyboard shortcuts (Cmd+P, Ctrl+R, etc.) don't work when non-US input source is active (Korean, Japanese, Chinese, French, Ukrainian, etc.). Users must switch to US keyboard layout to use Warp keybindings.

#### Root Cause
Warp's keybinding system relies on physical key codes OR character codes without proper input source mapping. When IME is active, the character codes change but Warp's keybinding matcher doesn't account for this.

#### Fix
**NONE** - Still an open issue (#341, #6891).

#### Lesson for Crux
- **CRITICAL for Phase 3**: This is a MAJOR competitive advantage opportunity
- Use macOS `NSEvent.charactersIgnoringModifiers` for keybindings
- From `research/platform/ime-clipboard.md`:
  - Physical key codes (kVK_*) are layout-independent
  - Character codes change with input source
  - Must map both correctly
- Test with:
  - Korean 2-Set, Japanese Hiragana, Chinese Pinyin
  - Verify Cmd+shortcuts work regardless of active input source
- This is a showstopper for Korean users (Crux's target market!)

**References**:
- [Warp GitHub #341](https://github.com/warpdotdev/Warp/issues/341)
- [Warp GitHub #6891](https://github.com/warpdotdev/warp/issues/6891)
- [Warp Docs: Known Issues](https://docs.warp.dev/support-and-billing/known-issues)

---

## Memory Management

### 9. iTerm2: Memory Leaks in PTYTask and tmux Integration

**Severity**: Medium (memory leak, eventual slowdown)
**Affected**: iTerm2 (multiple versions across history)
**Fixed**: Incremental fixes in various releases

#### Bug Description
- Memory leak in PTYTask (pseudo-terminal task management)
- Leak in tmux integration
- Memory leak tied to keypresses
- Unlimited scrollback could consume all available memory

#### Root Cause
Multiple issues over time:
1. **PTYTask threading**: Main thread held references to PTY data without releasing
2. **tmux integration**: Session objects not deallocated after detach
3. **Scrollback buffer**: No upper limit → unbounded growth

#### Fix
- Used separate thread in PTYSession to process data from PTYTask
- Fixed tmux session cleanup on detach
- Added scrollback limit option (default: limited, optional unlimited)
- Fixed keypress-related retain cycles

#### Lesson for Crux
- **Rust's ownership helps**, but still watch for:
  - `Rc<RefCell<>>` cycles in `crux-terminal` state
  - GPUI `Model<>` references in `crux-terminal-view`
  - PTY thread holding references to terminal grid
- Scrollback limit:
  - Default: 10,000 lines (reasonable)
  - Optional unlimited with warning
  - Implement efficient scrollback pruning (ring buffer)
- Test long-running sessions (days/weeks) with:
  - High-frequency output (build logs, tail -f)
  - Large scrollback buffers
  - Multiple tabs/panes open

**References**:
- [iTerm2 Changelog](https://github.com/jamesarosen/iTerm2/blob/master/Changelog)
- [iTerm2 GitLab #9221](https://gitlab.com/gnachman/iterm2/-/issues/9221)

---

### 10. Warp: Critical Memory Leak (3.6GB+ RAM, System Freeze)

**Severity**: CRITICAL (system-level impact)
**Affected**: Warp (2024-2025, ongoing reports)
**Fixed**: Partial fix for Warpified subshells, still problematic

#### Bug Description
- Warp process consuming 3.6GB+ RAM (90% of 4GB system)
- Opening Warp with no activity: ~2.5GB RAM
- After simple `git log`: 4GB+ RAM
- Unbounded memory growth over time → system swap thrashing → freeze

#### Root Cause
From Warp changelog: "Bug that could cause unbounded memory growth when using Warpified subshells or legacy (non-tmux) SSH Warpify implementation."

Likely causes:
- Block history retained indefinitely (Warp's unique block-based UI)
- Agent output not garbage collected
- Subshell environment state accumulation

#### Fix (Partial)
- Fixed unbounded memory growth in Warpified subshells
- Improved out-of-memory handling for ambient agents
- Users report restarting Warp resolves temporarily

#### Lesson for Crux
- **Avoid block-based UI complexity** (we're using traditional scrollback)
- Implement strict memory limits:
  - Scrollback buffer max size (default 10K lines)
  - Tab/pane closure fully deallocates resources
  - No indefinite history retention
- Monitor memory in development:
  - Instruments Memory Graph Debugger
  - Heap snapshots before/after operations
  - Long-running stress tests (days)

**References**:
- [Warp GitHub #7520](https://github.com/warpdotdev/warp/issues/7520)
- [Warp GitHub #7101](https://github.com/warpdotdev/warp/issues/7101)
- [Warp GitHub #8205](https://github.com/warpdotdev/warp/issues/8205)
- [Warp Changelog](https://docs.warp.dev/getting-started/changelog)

---

## Performance & Threading

### 11. iTerm2: PTYTask Thread Deadlock (VT100Terminal ↔ PTYTextView)

**Severity**: High (terminal hangs)
**Affected**: iTerm2 (early versions)
**Fixed**: Architectural change to separate thread

#### Bug Description
Deadlock between `VT100Terminal` (parser) and `PTYTextView` (renderer) caused terminal to freeze, especially during high-frequency output.

#### Root Cause
**Classic deadlock**: Two threads acquiring locks in opposite order:
- Thread A: PTY read → lock VT100Terminal → update PTYTextView (needs lock)
- Thread B: Render → lock PTYTextView → read VT100Terminal state (needs lock)

#### Fix
Architectural change:
1. Separate thread in `PTYSession` to process data from `PTYTask`
2. PTYTask appends data to `VT100Terminal` stream (lock-free queue)
3. Rendering reads from terminal state without blocking PTY thread

Additional fix: Coprocess file descriptors set to **non-blocking** to avoid deadlock (issue #2576).

#### Lesson for Crux
- **Our architecture** (from `research/core/terminal-architecture.md`):
  ```
  PTY thread → channels → VT parser (alacritty_terminal) → GPUI update
  ```
  - PTY read happens in background thread (via `portable-pty`)
  - `alacritty_terminal::Term` is single-threaded (good!)
  - GPUI updates only on main thread
- Use **async channels** (tokio mpsc) to avoid blocking
- Set PTY to **non-blocking I/O**
- Never hold locks across thread boundaries
- Test deadlock scenarios:
  - High-frequency output (cat large file)
  - Rapid window resize during output
  - Multiple tabs outputting simultaneously

**References**:
- [iTerm2 Changelog](https://github.com/jamesarosen/iTerm2/blob/master/Changelog)

---

### 12. iTerm2: Performance Regression in 3.5.0 (Fixed in 3.5.4beta1)

**Severity**: High (severe slowdown)
**Affected**: iTerm2 3.5.0–3.5.3
**Fixed**: iTerm2 3.5.4beta1

#### Bug Description
Version 3.5.0 had "a bunch of performance issues" causing severe slowdown compared to 3.4.x.

#### Root Cause
Not detailed in search results, but likely related to:
- Metal renderer changes
- New features in 3.5.0 (faster tab creation, tmux flow control)
- Unoptimized code paths

#### Fix
Multiple performance fixes in 3.5.4beta1. Some known improvements:
- Faster tab creation (daemon process redesign)
- Improved background image performance
- Tmux integration flow control (prevents excessive buffering)

#### Lesson for Crux
- **Performance regression testing** is critical
- Benchmark key operations across versions:
  - Terminal output throughput (MB/s)
  - Window resize latency
  - Tab creation time
  - Memory usage over time
- Use `cargo bench` for regression tracking
- Test on real workloads:
  - `cat large_file.txt`
  - `find / -name "*"`
  - `yes | head -n 100000`

**References**:
- [iTerm2 News](https://iterm2.com/news.html)
- [iTerm2 Changelog](https://iterm2.com/appcasts/full_changes.txt)

---

### 13. iTerm2: Ligatures + Underlined Text = Extreme Slowdown

**Severity**: High (severe slowdown)
**Affected**: iTerm2 with ligature fonts (FiraCode, Cascadia Code)
**Status**: Known limitation, performance trade-off

#### Bug Description
When ligatures are enabled (Prefs > Profiles > Text) with fonts like FiraCode, rendering becomes extremely slow. Combining ligatures + underlined text causes extreme performance degradation.

#### Root Cause
Two compounding performance issues:
1. **Ligatures disable GPU renderer**: Must fall back to CPU rendering (CoreText)
2. **CoreText is significantly slower** than Core Graphics/Metal
3. **Underlined text + ligatures**: Complex text layout calculations per cell

From iTerm2 docs: "Makes drawing much slower for two reasons: first, it disables the GPU renderer. Second, it uses a slower API."

#### Fix
**None** - fundamental trade-off. Recommendation: disable ligatures on slow hardware.

#### Lesson for Crux
- **Decide early**: Support ligatures or not?
- If supporting ligatures:
  - Implement in Metal/GPU (complex, but fast)
  - Cache ligature glyph renders
  - Warn users about performance impact
- **GPUI canvas approach** might make this easier than iTerm2's CoreText path
- Test performance with:
  - FiraCode font
  - Code with many ligatures (`=>`, `!=`, `>=`, etc.)
  - Underlined text (error messages, links)

**References**:
- [iTerm2 Fonts Documentation](https://iterm2.com/documentation-fonts.html)
- [iTerm2 GitLab #8105](https://gitlab.com/gnachman/iterm2/-/issues/8105)

---

## Font Rendering

### 14. iTerm2: GPU Rendering Changes Font Weight

**Severity**: Medium (visual quality)
**Affected**: iTerm2 with Metal renderer
**Fixed**: Improved, but some trade-offs remain

#### Bug Description
- GPU rendering makes fonts appear thinner/lighter weight
- Font display weight changes between GPU and CPU rendering
- Glyphs cut off/truncated when using Metal rendering on non-Retina (1x) displays

#### Root Cause
Different rendering pipelines:
- **CPU rendering**: CoreText with subpixel anti-aliasing (heavier weight)
- **Metal rendering**: GPU shader anti-aliasing (lighter weight, no subpixel)

From research: "GPU renderer has to use a more complex blending algorithm and GPU rendering becomes unavailable in concert with transparent windows."

#### Fix
- Improved Metal shader anti-aliasing
- Added "Thin strokes" option (Prefs > Profiles > Text)
- Subpixel anti-aliasing in Metal renderer (complex, see Google Doc)
- Still trade-offs: transparency disables GPU rendering

#### Lesson for Crux
- **GPUI rendering** will face similar challenges
- Options:
  1. Implement subpixel anti-aliasing in Metal shaders (hard)
  2. Use grayscale anti-aliasing + font weight adjustment
  3. Let GPUI handle it (if they already solved this)
- Test on:
  - Retina vs non-Retina displays
  - Various font weights (Light, Regular, Medium, Bold)
  - Dark vs light backgrounds
- Consider "Font smoothing" preference like Terminal.app

**References**:
- [iTerm2 GitLab #7128](https://gitlab.com/gnachman/iterm2/-/issues/7128)
- [iTerm2 GitLab #11267](https://gitlab.com/gnachman/iterm2/-/issues/11267)
- [iTerm2 Subpixel Anti-aliasing Doc](https://docs.google.com/document/d/1vfBq6vg409Zky-IQ7ne-Yy7olPtVCl0dq3PG20E8KDs/edit)

---

## Protocol & Integration

### 15. iTerm2: Scrollback Buffer Corruption in Alternate Screen

**Severity**: Medium (data loss)
**Affected**: iTerm2 with tmux integration
**Fixed**: iTerm2 3.0.7+

#### Bug Description
Cursor position not correctly restored in main screen when attaching to tmux integration session while in alternate screen (e.g., vim, less). Terminal state corrupted on exit from alternate screen programs.

#### Root Cause
State machine error:
1. User runs `vim` (enters alternate screen)
2. User detaches tmux session
3. User reattaches tmux session
4. Vim exits → iTerm2 restores main screen
5. **Bug**: Cursor position from alternate screen incorrectly applied to main screen

#### Fix
- Fixed in iTerm2 3.0.7: Properly save/restore cursor state per screen buffer
- Correctly handle tmux reattach while alternate screen active

#### Lesson for Crux
- **Alternate screen** is in scope (Phase 4 via `alacritty_terminal`)
- `alacritty_terminal::Term` handles this, but test:
  - Save/restore cursor position per buffer
  - Save/restore SGR state (colors, bold, underline)
  - Scrollback buffer not leaked between buffers
- Test cases:
  - `vim` → detach tmux → reattach → `:q`
  - `less` → Ctrl+Z (background) → fg
  - Alternate screen + window resize

**References**:
- [iTerm2 GitLab #4862](https://gitlab.com/gnachman/iterm2/-/issues/4862)
- [iTerm2 GitLab #6273](https://gitlab.com/gnachman/iterm2/-/issues/6273)
- [iTerm2 Changelog (3.0.7)](https://iterm2.com/appcasts/testing_changes_10_8.txt)

---

### 16. Warp: SSH Integration Bugs (Stuck State, Spurious Characters)

**Severity**: High (broken remote sessions)
**Affected**: Warp SSH Wrapper (2024)
**Fixed**: Partial fixes in 2024

#### Bug Description
- Terminal stuck in bad state if SSH connection lost while alternate screen active (tmux, TUI, pagers)
- `00~` and `01~` characters erroneously added to commands after SSH connection lost
- SSH hangs when `/tmp` not writable for Zsh
- SSH returns `0~` and `1~` after executing commands for Zsh 5.0.8 or older

#### Root Cause
Bracketed paste mode not disabled on SSH disconnect. When Warp's SSH wrapper crashes/disconnects:
1. Warp enables bracketed paste mode (`\e[?2004h`)
2. SSH connection lost
3. Warp doesn't send disable sequence (`\e[?2004l`)
4. Terminal left in corrupted state
5. Next paste shows `\e[200~...\e[201~` literally

#### Fix
- Fixed: Terminal no longer stuck after SSH disconnect during alternate screen
- Fixed: Spurious `00~` and `01~` characters removed
- Fixed: SSH no longer hangs when `/tmp` not writable
- Fixed: Old Zsh version compatibility

#### Lesson for Crux
- **Bracketed paste mode** (Phase 2):
  - MUST disable on disconnect/error
  - MUST disable before process exit
  - Add cleanup handler in `crux-terminal`
- Test SSH scenarios (Phase 2/5):
  - Disconnect while in vim
  - Kill SSH process mid-session
  - Network interruption during paste
- Add terminal state reset on error:
  ```rust
  // On disconnect/error:
  term.reset_bracketed_paste();
  term.reset_alternate_screen();
  term.reset_sgr();
  ```

**References**:
- [Warp SSH Legacy Docs](https://docs.warp.dev/terminal/warpify/ssh-legacy)
- [Warp 2024 Year in Review](https://www.warp.dev/blog/2024-in-review)
- [Claude Code GitHub #3134](https://github.com/anthropics/claude-code/issues/3134)

---

### 17. Warp: tmux Integration Not Supported (Open Since 2021)

**Severity**: High (missing critical feature)
**Affected**: Warp (all versions)
**Status**: **NOT IMPLEMENTED** (open issue #42 since Nov 2021)

#### Bug Description
Blocks and input don't work within tmux sessions. Warp commands (Ctrl+R, etc.) don't work inside tmux. Option keys print hex codes instead of working as expected.

#### Root Cause
Warp's block-based UI architecture fundamentally conflicts with tmux's screen multiplexing. Warp would need to implement tmux control mode integration like iTerm2.

#### Proposed Solution
From GitHub discussion: "Most likely way Warp would support tmux is through tmux's control mode feature, like iTerm."

#### Current Status
- Not implemented
- Workaround: Use SSH wrapper (tmux-powered) for remote sessions
- Manual tmux inside Warp = degraded experience

#### Lesson for Crux
- **tmux integration** is Phase 5 priority
- Implement via tmux control mode (`tmux -CC`) like iTerm2
- BUT: Learn from iTerm2's security bugs (CVE-2019-9535, CVE-2024-38396)
- tmux control mode must:
  - Sanitize all escape sequences
  - Never execute commands from tmux output
  - Properly handle pane splits, window management
- This is a MAJOR competitive advantage over Warp if done right

**References**:
- [Warp GitHub #42](https://github.com/warpdotdev/warp/issues/42)
- [Warp GitHub #501](https://github.com/warpdotdev/Warp/discussions/501)
- [Warp GitHub #3737](https://github.com/warpdotdev/Warp/issues/3737)

---

### 18. Warp: URL Click Handling Bugs

**Severity**: Medium (UX annoyance)
**Affected**: Warp (multiple versions)
**Fixed**: Partial improvements

#### Bug Description
- Clickable hyperlinks (OSC 8) don't work with Cmd+Click
- Long URLs cut off, Cmd+Click doesn't work past certain length
- URLs only partially detected on hover
- Links clickable through Warp menu (z-order bug)

#### Root Cause
Multiple issues:
1. **OSC 8 hyperlink support**: Not implemented (iTerm2 supports this)
2. **URL detection regex**: Doesn't handle all URL formats/lengths
3. **Z-order bug**: Clickable regions not properly masked by modal windows

#### Fix (Partial)
- Fixed: Hovering over URLs in blocklist now correctly detects full URL
- Fixed: URL detection improved (but still issues with very long URLs)
- Not fixed: OSC 8 hyperlink support

#### Lesson for Crux
- **URL detection** (Phase 4):
  - Use robust regex (or better: proper parser)
  - Support OSC 8 hyperlinks (terminal standard)
  - Test edge cases:
    - Very long URLs (2048+ chars)
    - URLs with special characters (`%`, `#`, `&`)
    - Multiple URLs per line
- **Click handling**:
  - Cmd+Click should NOT conflict with text selection
  - Right-click context menu for copy URL
  - Proper z-order for modal windows

**References**:
- [Warp GitHub #6393](https://github.com/warpdotdev/Warp/issues/6393)
- [Warp GitHub #5603](https://github.com/warpdotdev/Warp/issues/5603)
- [Warp Docs: Files & Links](https://docs.warp.dev/terminal/more-features/files-and-links)

---

## Lessons for Crux

### Security (CRITICAL)

1. **Escape Sequence Sanitization**
   - Sanitize ALL escape sequences before processing
   - Never trust terminal output as safe input
   - Disable dangerous sequences by default (window title reporting)
   - Security audit before 1.0 release
   - Add fuzzing for VT parser

2. **tmux Integration Security**
   - Learn from iTerm2's CVEs (2019-9535, 2024-38396)
   - Sanitize control mode output before execution
   - Never auto-execute commands from tmux
   - Implement security-first, features second

3. **Bracketed Paste Mode**
   - MUST disable on disconnect/error
   - MUST disable before process exit
   - Test cleanup in error paths

### Rendering & GPU

4. **Metal Renderer Considerations**
   - Test transparency + Metal compositing early
   - Implement frame buffer clearing between draws
   - Monitor battery drain on dual-GPU MacBooks
   - Graceful fallback if Metal fails
   - Test on Intel vs Apple Silicon

5. **Font Rendering**
   - Decide on ligature support early (performance trade-off)
   - Test font weight consistency between rendering paths
   - Implement proper subpixel anti-aliasing OR adjust font weight
   - Test on Retina vs non-Retina displays

### IME & International (Phase 3 PRIORITY)

6. **CJK Keybindings**
   - Use `NSEvent.charactersIgnoringModifiers` for shortcuts
   - Test with Korean, Japanese, Chinese input sources
   - Verify Cmd+shortcuts work regardless of active IME
   - **MAJOR competitive advantage over Warp**

7. **Emoji & East Asian Width**
   - Query actual rendered glyph width from CoreText
   - Test emoji with variant selectors (0xFE0F)
   - Use Unicode 9.0+ width calculations
   - Test mixed emoji + CJK text

### Memory Management

8. **Memory Leaks**
   - Watch for `Rc<RefCell<>>` cycles
   - Monitor GPUI `Model<>` references
   - Implement scrollback limit (default 10K lines)
   - Test long-running sessions (days/weeks)
   - Use Instruments Memory Graph Debugger

### Performance & Threading

9. **Avoid Deadlocks**
   - Use async channels (tokio mpsc)
   - Set PTY to non-blocking I/O
   - Never hold locks across thread boundaries
   - Test high-frequency output scenarios

10. **Performance Regression Testing**
    - Benchmark terminal output throughput
    - Track window resize latency
    - Monitor tab creation time
    - Use `cargo bench` for regression tracking

### Protocol & Integration

11. **Alternate Screen Buffer**
    - Save/restore cursor position per buffer
    - Save/restore SGR state
    - Test tmux reattach scenarios
    - Test with vim, less, htop

12. **URL Detection**
    - Support OSC 8 hyperlinks (standard)
    - Use robust URL parser (not just regex)
    - Test very long URLs (2048+ chars)
    - Implement Cmd+Click correctly

### Testing Strategy

**Critical Test Cases**:
1. High-frequency output (cat large file, find /)
2. Long-running sessions (days/weeks)
3. CJK input with Korean/Japanese IME
4. Keybindings with non-US input sources
5. tmux attach/detach cycles
6. Alternate screen + window resize
7. SSH disconnect during vim session
8. Multiple tabs with simultaneous output
9. Battery drain on dual-GPU MacBooks
10. Memory usage over extended periods

### Competitive Advantages

Based on these bugs, Crux can differentiate by:
1. **Perfect CJK/IME support** (Warp fails here)
2. **Secure tmux integration** (iTerm2 had CVEs)
3. **Efficient memory management** (Warp leaks badly)
4. **Proper Korean input** (target market!)
5. **Security-first architecture** (learn from CVEs)

---

## Summary Table (Resolved Issues)

| Bug | Terminal | Severity | Fixed? | Crux Action |
|-----|----------|----------|--------|-------------|
| CVE-2019-9535 tmux RCE | iTerm2 | CRITICAL | ✅ 3.3.6 | Sanitize escape sequences |
| CVE-2024-38396 Title RCE | iTerm2 | CRITICAL | ✅ 3.5.2 | Disable title reporting |
| Metal + Transparency Ghosts | iTerm2 | Medium | ⚠️ Partial | Test early, proper clearing |
| Metal Forces Discrete GPU | iTerm2 | Medium | ✅ Option added | Monitor battery drain |
| GPU Battery Drain | Warp | High | ❌ Ongoing | Ensure idle GPU release |
| Graphics Driver Crashes | Warp | Critical | ⚠️ Workaround | Test on multiple GPU types |
| Emoji Variant Selector | iTerm2 | Medium | ✅ Improved | Query CoreText width |
| CJK Keybindings Broken | Warp | High | ❌ Open since 2021 | **MAJOR opportunity** |
| PTYTask Memory Leak | iTerm2 | Medium | ✅ Fixed | Watch Rc cycles |
| Memory Leak (3.6GB+) | Warp | CRITICAL | ⚠️ Partial | Strict memory limits |
| PTYTask Deadlock | iTerm2 | High | ✅ Architectural fix | Use async channels |
| Performance Regression 3.5.0 | iTerm2 | High | ✅ 3.5.4beta1 | Regression testing |
| Ligatures + Underline Slow | iTerm2 | High | ⚠️ Known limit | Decide on ligatures early |
| GPU Changes Font Weight | iTerm2 | Medium | ⚠️ Improved | Test rendering quality |
| Alternate Screen Corruption | iTerm2 | Medium | ✅ 3.0.7 | Test state machine |
| SSH Bracketed Paste Bug | Warp | High | ✅ 2024 | Cleanup on disconnect |
| tmux Not Supported | Warp | High | ❌ Open since 2021 | Implement securely |
| URL Click Handling | Warp | Medium | ⚠️ Partial | Support OSC 8 |

**Legend**: ✅ Fixed | ⚠️ Partial/Workaround | ❌ Not Fixed

---

## Additional References (Resolved Issues)

### iTerm2
- [Official Website](https://iterm2.com/)
- [GitLab Repository](https://gitlab.com/gnachman/iterm2)
- [Changelog](https://iterm2.com/appcasts/full_changes.txt)
- [Metal Renderer Wiki](https://gitlab.com/gnachman/iterm2/-/wikis/Metal-Renderer)

### Warp
- [Official Website](https://www.warp.dev/)
- [GitHub Repository](https://github.com/warpdotdev/Warp)
- [Changelog](https://docs.warp.dev/getting-started/changelog)
- [Known Issues](https://docs.warp.dev/support-and-billing/known-issues)

### Security
- [CVE-2019-9535 Details](https://nvd.nist.gov/vuln/detail/CVE-2019-9535)
- [CVE-2024-38396 Details](https://nvd.nist.gov/vuln/detail/CVE-2024-38396)
- [Mozilla Security Blog](https://blog.mozilla.org/security/2019/10/09/iterm2-critical-issue-moss-audit/)
- [Vin01's Technical Blog](https://vin01.github.io/piptagole/escape-sequences/iterm2/rce/2024/06/16/iterm2-rce-window-title-tmux-integration.html)
