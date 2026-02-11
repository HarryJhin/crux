---
title: "OSC 8 Hyperlinks and URL Detection"
description: "OSC 8 explicit hyperlinks spec, id parameter for multi-line links, URI scheme whitelist, implicit URL detection regex, hover/click behavior, keyboard hints mode, security considerations, alacritty_terminal built-in support"
date: 2026-02-12
phase: [4]
topics: [hyperlinks, osc-8, url-detection, security]
status: final
related:
  - terminal-emulation.md
---

# OSC 8 Hyperlinks and URL Detection

> 작성일: 2026-02-12
> 목적: Crux 터미널에서 하이퍼링크를 지원하기 위한 기술 조사 — OSC 8 명시적 링크, 암시적 URL 감지, 보안 고려사항

---

## 목차

1. [개요](#1-개요)
2. [OSC 8 — Explicit Hyperlinks](#2-osc-8--explicit-hyperlinks)
3. [alacritty_terminal Built-in Support](#3-alacritty_terminal-built-in-support)
4. [Implicit URL Detection](#4-implicit-url-detection)
5. [Hover and Click Behavior](#5-hover-and-click-behavior)
6. [Keyboard Hints Mode](#6-keyboard-hints-mode)
7. [Security Considerations](#7-security-considerations)
8. [Tools Emitting OSC 8](#8-tools-emitting-osc-8)
9. [Crux Implementation Recommendations](#9-crux-implementation-recommendations)

---

## 1. 개요

Terminal hyperlinks come in two forms:

1. **Explicit (OSC 8)**: Applications emit escape sequences to mark text as a hyperlink with a specific URI. The terminal renders the text with underline/color and makes it clickable.
2. **Implicit (URL detection)**: The terminal scans output for patterns that look like URLs (https://..., file://...) and makes them clickable automatically.

Both are essential for a modern terminal experience. Tools like `cargo`, `ripgrep`, `gcc`, and `ls` now emit OSC 8 hyperlinks, while implicit detection catches URLs in logs, error messages, and chat.

Sources: [Egmont Koblinger's OSC 8 Spec](https://gist.github.com/egmontkob/eb114294efbcd5adb1944c9f3cb5feda), [XTerm Control Sequences](https://invisible-island.net/xterm/ctlseqs/ctlseqs.html)

---

## 2. OSC 8 — Explicit Hyperlinks

### Escape Sequence Format

```
ESC ] 8 ; params ; URI ST       ← Open hyperlink
ESC ] 8 ; ; ST                  ← Close hyperlink (empty URI)
```

Where:
- `ESC ]` is OSC (Operating System Command)
- `8` is the hyperlink function number
- `params` is a colon-separated list of `key=value` pairs (or empty)
- `URI` is the hyperlink target
- `ST` is String Terminator: `ESC \` (`\x1b\x5c`) or BEL (`\x07`)

### Example

```
\x1b]8;;https://example.com\x1b\\Click here\x1b]8;;\x1b\\
```

Renders as: [Click here](https://example.com) (underlined, clickable)

### The `id` Parameter

The `id` parameter groups non-contiguous text into a single hyperlink. This is critical for **multi-line links** and **wrapped links**:

```
\x1b]8;id=link1;https://example.com\x1b\\This is a long hyperlink that
wraps to the next line\x1b]8;;\x1b\\
```

Without `id`, each line segment would be treated as a separate link. With `id=link1`, the terminal knows both segments are the same link and can:
- Highlight both segments on hover over either one
- Open the same URL when clicking either segment

### Parameter Format

Parameters are semicolon-separated `key=value` pairs before the URI:

```
ESC ] 8 ; id=mylink:other=value ; https://example.com ST
```

Currently only `id` is standardized. Other parameters should be preserved but can be ignored.

### Multi-Cell Rendering

A hyperlinked region can span multiple cells:

```
Cell 0: 'C'  hyperlink=Some("https://example.com", id="link1")
Cell 1: 'l'  hyperlink=Some("https://example.com", id="link1")
Cell 2: 'i'  hyperlink=Some("https://example.com", id="link1")
Cell 3: 'c'  hyperlink=Some("https://example.com", id="link1")
Cell 4: 'k'  hyperlink=Some("https://example.com", id="link1")
Cell 5: ' '  hyperlink=None
Cell 6: 'h'  hyperlink=None
```

### Terminal Adoption

| Terminal | OSC 8 Support | Since |
|----------|--------------|-------|
| iTerm2 | Yes | 2017 |
| GNOME Terminal (VTE) | Yes | 2017 |
| WezTerm | Yes | Early |
| Kitty | Yes | 0.19 |
| Ghostty | Yes | Launch |
| Alacritty | Yes (via alacritty_terminal) | 0.14 |
| Windows Terminal | Yes | 2021 |
| foot | Yes | Early |
| macOS Terminal.app | No | — |

---

## 3. alacritty_terminal Built-in Support

### Cell Hyperlink API

`alacritty_terminal` already parses OSC 8 and stores hyperlink data on each cell:

```rust
use alacritty_terminal::term::cell::Cell;

// Each cell has an optional hyperlink
impl Cell {
    pub fn hyperlink(&self) -> Option<&Hyperlink> { ... }
}

// Hyperlink contains the URI and optional id
pub struct Hyperlink {
    inner: HyperlinkInner,
}

impl Hyperlink {
    pub fn uri(&self) -> &str { ... }
    pub fn id(&self) -> Option<&str> { ... }
}
```

### Using in Crux Rendering

```rust
// In CruxTerminalElement::paint()
fn paint_cell(&self, cx: &mut WindowContext, cell: &RenderableCell, origin: Point<Pixels>) {
    // Normal text rendering
    self.paint_text(cx, cell, origin);

    // If cell has a hyperlink, add underline decoration
    if let Some(hyperlink) = cell.hyperlink() {
        let is_hovered = self.hovered_hyperlink.as_ref()
            .map(|h| h.uri() == hyperlink.uri())
            .unwrap_or(false);

        if is_hovered {
            // Draw underline on hover
            self.paint_underline(cx, origin, cell_width, cell.fg);
            // Change cursor to pointer
            cx.set_cursor_style(CursorStyle::PointingHand);
        }
    }
}
```

### What Crux Gets for Free

Since `alacritty_terminal` handles the parsing, Crux automatically gets:
- OSC 8 sequence parsing and state management
- Hyperlink data stored per cell
- `id` parameter grouping
- Proper hyperlink lifecycle (open → text → close)

Crux only needs to implement the **rendering** (underline, color) and **interaction** (hover highlight, click to open).

---

## 4. Implicit URL Detection

### Overview

Implicit URL detection scans terminal content for text that looks like URLs and makes them clickable without requiring OSC 8 sequences. This catches URLs in:
- Compiler error output
- Log files
- Chat messages
- README content
- `git log` output

### URL Detection Regex

A robust URL detection regex for terminals:

```rust
use regex::Regex;
use once_cell::sync::Lazy;

static URL_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(concat!(
        // Explicit scheme URLs
        r"(?:",
            r"https?://",                          // http:// or https://
            r"|ftp://",                             // ftp://
            r"|file://",                            // file://
            r"|mailto:",                            // mailto:
        r")",
        r"[^\s<>\"'`\x00-\x1f\x7f-\x9f]*",       // URL body (no whitespace, no control chars)
        r"[^\s<>\"'`\x00-\x1f\x7f-\x9f.,;:!?\)\]\}]", // Don't end with punctuation
    )).unwrap()
});
```

### Trailing Punctuation Problem

A common issue: URLs in prose often end with punctuation that's not part of the URL:

```
Visit https://example.com/path. ← Period is NOT part of URL
See https://example.com/path(info) ← Closing paren IS part of URL
```

Strategies:
- **Bracket matching**: Count open/close brackets within the URL; only strip unmatched trailing brackets
- **Trailing punctuation strip**: Remove `.,;:!?` from end unless they appear earlier in the URL
- **Path heuristic**: `/path.` strips `.`, `/path.html` does not

### Bracket Matching

```rust
fn trim_url_trailing(url: &str) -> &str {
    let mut result = url;

    // Trim trailing punctuation that's unlikely part of the URL
    while let Some(last) = result.chars().last() {
        match last {
            '.' | ',' | ';' | ':' | '!' | '?' => {
                result = &result[..result.len() - last.len_utf8()];
            }
            ')' => {
                // Only trim if unmatched
                let opens = result.chars().filter(|&c| c == '(').count();
                let closes = result.chars().filter(|&c| c == ')').count();
                if closes > opens {
                    result = &result[..result.len() - 1];
                } else {
                    break;
                }
            }
            ']' => {
                let opens = result.chars().filter(|&c| c == '[').count();
                let closes = result.chars().filter(|&c| c == ']').count();
                if closes > opens {
                    result = &result[..result.len() - 1];
                } else {
                    break;
                }
            }
            _ => break,
        }
    }

    result
}
```

### When to Run Detection

- **On render**: Scan visible lines only (not entire scrollback)
- **Cache results**: Re-detect only when line content changes (use damage tracking)
- **Background thread**: For large scroll regions, detect in background

```rust
struct ImplicitUrlCache {
    /// Map from line index to detected URLs
    line_urls: HashMap<usize, Vec<DetectedUrl>>,
    /// Version counter for cache invalidation
    version: u64,
}

struct DetectedUrl {
    start_col: usize,
    end_col: usize,
    url: String,
}
```

---

## 5. Hover and Click Behavior

### Hover

When the mouse hovers over a hyperlinked cell:

1. **Underline**: Draw underline under the entire link span
2. **Color change**: Optionally change text color to a "link" color
3. **Cursor**: Change mouse cursor to pointing hand
4. **Tooltip**: Show the actual URL in a tooltip (important for security — see Section 7)
5. **Multi-segment**: If the link spans multiple lines (same `id`), highlight all segments

### Click

| Action | Behavior |
|--------|----------|
| Cmd+Click | Open URL in default browser (standard macOS convention) |
| Click (no modifier) | Normal terminal interaction (selection, mouse reporting) |
| Right-click | Context menu: Open, Copy URL, Copy Text |

**Why Cmd+Click**: In mouse-reporting mode, plain clicks are forwarded to applications. Cmd+Click is the universal convention (iTerm2, Kitty, Ghostty, WezTerm) for "I want to interact with the terminal chrome, not the application."

### Opening URLs

```rust
use std::process::Command;

fn open_url(url: &str) -> Result<(), std::io::Error> {
    // Validate URL before opening
    if !is_safe_url(url) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Unsafe URL scheme",
        ));
    }

    // macOS: use `open` command
    Command::new("open")
        .arg(url)
        .spawn()?;

    Ok(())
}
```

---

## 6. Keyboard Hints Mode

### Overview

Keyboard hints mode (popularized by Kitty and Ghostty) lets users interact with URLs without a mouse:

1. User presses a keyboard shortcut (e.g., Cmd+Shift+U)
2. Terminal scans visible content for URLs
3. Each URL gets a short label (a, b, c, ..., aa, ab, ...)
4. User types the label → URL is opened
5. Press Escape to cancel

### Visual Overlay

```
user@host:~$ cat error.log
Error at https://example.com/api/v2/endpoint  [a]
See https://docs.example.com/troubleshooting   [b]
Report: https://github.com/org/repo/issues/42  [c]

Type hint label or press Escape to cancel
```

### Implementation

```rust
struct HintsMode {
    active: bool,
    urls: Vec<HintedUrl>,
    input_buffer: String,
}

struct HintedUrl {
    label: String,     // "a", "b", ..., "aa", "ab"
    url: String,
    start: GridPosition,
    end: GridPosition,
}

impl HintsMode {
    fn activate(&mut self, terminal: &Term) {
        self.urls = find_all_urls_in_visible(terminal);
        self.assign_labels();
        self.active = true;
    }

    fn handle_key(&mut self, key: char) -> HintsAction {
        self.input_buffer.push(key);

        // Check for exact match
        if let Some(url) = self.urls.iter().find(|u| u.label == self.input_buffer) {
            return HintsAction::Open(url.url.clone());
        }

        // Check if any label starts with current input
        let has_prefix = self.urls.iter().any(|u| u.label.starts_with(&self.input_buffer));
        if !has_prefix {
            return HintsAction::Cancel;
        }

        HintsAction::Continue
    }

    fn assign_labels(&mut self) {
        let chars: Vec<char> = "asdfjklghweruio".chars().collect(); // Home row first
        for (i, url) in self.urls.iter_mut().enumerate() {
            if i < chars.len() {
                url.label = chars[i].to_string();
            } else {
                // Two-character labels for overflow
                let first = chars[i / chars.len() - 1];
                let second = chars[i % chars.len()];
                url.label = format!("{}{}", first, second);
            }
        }
    }
}
```

---

## 7. Security Considerations

### URL Spoofing

A malicious application can use OSC 8 to make text look like one URL while linking to another:

```
\x1b]8;;https://evil.com\x1b\\https://google.com\x1b]8;;\x1b\\
```

This displays "https://google.com" but actually links to "https://evil.com".

**Mitigation**: Always show the **actual URL** in a tooltip on hover. This is the single most important security measure.

### CVE-2023-46321 and CVE-2023-46322 (iTerm2)

iTerm2 had critical vulnerabilities in its URL handling:

- **CVE-2023-46321**: `x-man-page://` URI scheme could execute arbitrary commands via `man` page rendering
- **CVE-2023-46322**: `ssh://` URI scheme could be crafted to execute commands via SSH escape characters

Both were caused by insufficient URI scheme validation.

### URI Scheme Whitelist

Only open URIs with known-safe schemes:

```rust
const SAFE_SCHEMES: &[&str] = &[
    "http",
    "https",
    "ftp",
    "ftps",
    "mailto",
    "file",     // Only if hostname matches local machine
    "ssh",      // Only open in SSH client, never execute directly
];

fn is_safe_url(url: &str) -> bool {
    if let Ok(parsed) = url::Url::parse(url) {
        let scheme = parsed.scheme().to_lowercase();

        // Whitelist check
        if !SAFE_SCHEMES.contains(&scheme.as_str()) {
            return false;
        }

        // file:// special handling: must be local
        if scheme == "file" {
            match parsed.host_str() {
                None | Some("") | Some("localhost") => {},
                Some(host) => {
                    // Verify it's the local machine
                    if !is_local_hostname(host) {
                        return false;
                    }
                }
            }
        }

        true
    } else {
        false
    }
}
```

### Additional Security Measures

| Measure | Description |
|---------|-------------|
| Tooltip on hover | Always show actual URI, not display text |
| Cmd+Click required | Prevent accidental clicks in mouse-reporting mode |
| Scheme whitelist | Block `javascript:`, `data:`, `x-man-page:`, custom schemes |
| `file://` hostname check | Prevent opening remote files as local |
| URI length limit | Reject URIs > 2048 characters |
| Confirmation for `file://` | Show confirmation dialog before opening local files |
| Log URL opens | Audit trail: log all URLs opened by the terminal |

### Hostname Validation for `file://`

```rust
fn is_local_hostname(host: &str) -> bool {
    if host.is_empty() || host == "localhost" {
        return true;
    }
    hostname::get()
        .map(|h| h.to_string_lossy().eq_ignore_ascii_case(host))
        .unwrap_or(false)
}
```

---

## 8. Tools Emitting OSC 8

### Ecosystem Adoption

Many common developer tools now emit OSC 8 hyperlinks:

| Tool | Since Version | Link Target |
|------|--------------|-------------|
| `cargo` | 1.75 (2023-12) | Source file paths in compiler errors |
| `rustc` | 1.75 (2023-12) | Error code documentation URLs |
| `ripgrep` | 14.0 (2023-11) | `--hyperlink-format` flag for file:// links |
| `fd` | 10.2 (2024) | File paths as file:// links |
| `ls` | GNU coreutils 8.32+ | `--hyperlink=auto` for file:// links |
| `gcc` | 10+ | Error documentation URLs |
| `clang` | 16+ | Diagnostic URLs |
| `bat` | 0.24+ | File paths and line numbers |
| `delta` | 0.16+ | Git diff file paths |
| `systemd` | 239+ | Journal URLs |
| `python` | 3.13+ | Traceback file paths |

### TERM_FEATURES Detection

Tools detect hyperlink support via:

1. **`TERM_FEATURES` env var**: Not yet standardized
2. **`COLORTERM` env var**: Indirect signal (if `truecolor`, likely modern terminal)
3. **Terminal identification**: `TERM_PROGRAM=crux` → tools can whitelist
4. **DECRQM query**: `CSI ? 2031 $ p` (proposed but not widely adopted)

### Crux Should Set

```rust
cmd.env("TERM_PROGRAM", "crux");
cmd.env("TERM_PROGRAM_VERSION", env!("CARGO_PKG_VERSION"));
// Consider: set COLORTERM=truecolor to signal modern terminal
cmd.env("COLORTERM", "truecolor");
```

---

## 9. Crux Implementation Recommendations

### Phase 1: Minimal Hyperlink Support

Since `alacritty_terminal` already parses OSC 8, Phase 1 can ship basic support with minimal effort:

1. **Render hyperlinks**: Check `cell.hyperlink()` during painting, apply underline style
2. **Cmd+Click to open**: On Cmd+Click, extract URL from cell's hyperlink, validate scheme, open
3. **Hover tooltip**: Show actual URL in a small overlay on Cmd+hover
4. **URI scheme whitelist**: Only allow safe schemes

### Phase 2: Implicit URL Detection

5. **URL regex scanning**: Scan visible lines for URL patterns
6. **Bracket-aware trimming**: Handle trailing punctuation correctly
7. **Cached detection**: Re-scan only damaged lines

### Phase 3: Keyboard Hints + Context Menu

8. **Keyboard hints mode**: Cmd+Shift+U to show hint labels
9. **Right-click context menu**: Open, Copy URL, Copy Link Text
10. **Configurable modifier**: Cmd+Click vs plain Click for URL opening

### Architecture

```rust
/// Hyperlink system for Crux terminal
pub struct HyperlinkSystem {
    /// Implicit URL detection cache
    url_cache: ImplicitUrlCache,
    /// Currently hovered hyperlink (explicit or implicit)
    hovered: Option<HoveredLink>,
    /// Keyboard hints mode state
    hints: HintsMode,
    /// URI scheme whitelist
    allowed_schemes: HashSet<String>,
}

enum HoveredLink {
    Explicit {
        hyperlink: Hyperlink,
        cells: Vec<GridPosition>,
    },
    Implicit {
        url: String,
        start: GridPosition,
        end: GridPosition,
    },
}

impl HyperlinkSystem {
    /// Called during mouse move events
    pub fn update_hover(&mut self, grid_pos: GridPosition, terminal: &Term) {
        // Check explicit hyperlinks first (from cell.hyperlink())
        if let Some(hyperlink) = terminal.grid()[grid_pos].hyperlink() {
            self.hovered = Some(HoveredLink::Explicit {
                hyperlink: hyperlink.clone(),
                cells: find_all_cells_with_id(terminal, hyperlink),
            });
            return;
        }

        // Check implicit URLs
        if let Some(detected) = self.url_cache.url_at(grid_pos) {
            self.hovered = Some(HoveredLink::Implicit {
                url: detected.url.clone(),
                start: detected.start,
                end: detected.end,
            });
            return;
        }

        self.hovered = None;
    }

    /// Called on Cmd+Click
    pub fn open_hovered(&self) -> Result<(), HyperlinkError> {
        let url = match &self.hovered {
            Some(HoveredLink::Explicit { hyperlink, .. }) => hyperlink.uri(),
            Some(HoveredLink::Implicit { url, .. }) => url.as_str(),
            None => return Err(HyperlinkError::NoLink),
        };

        if !self.is_scheme_allowed(url) {
            return Err(HyperlinkError::BlockedScheme(url.to_string()));
        }

        open_url(url)
    }
}
```

### Configuration

```toml
[hyperlinks]
# Enable OSC 8 explicit hyperlinks
explicit = true

# Enable implicit URL detection
implicit = true

# Modifier key for clicking links (default: super/cmd)
click_modifier = "super"

# Show URL tooltip on hover
show_tooltip = true

# Allowed URI schemes (default whitelist)
# allowed_schemes = ["http", "https", "ftp", "file", "mailto", "ssh"]

# Implicit URL detection patterns (advanced)
# [[hyperlinks.patterns]]
# regex = '(?:bug|issue)\s*#?(\d+)'
# url = 'https://github.com/org/repo/issues/$1'
```

---

## Sources

- [Egmont Koblinger: Hyperlinks in Terminal Emulators](https://gist.github.com/egmontkob/eb114294efbcd5adb1944c9f3cb5feda) — The definitive OSC 8 specification
- [XTerm Control Sequences](https://invisible-island.net/xterm/ctlseqs/ctlseqs.html) — OSC 8 in the official xterm spec
- [alacritty_terminal Hyperlink](https://docs.rs/alacritty_terminal/0.25.0/alacritty_terminal/term/cell/struct.Hyperlink.html) — Built-in hyperlink support
- [CVE-2023-46321](https://nvd.nist.gov/vuln/detail/CVE-2023-46321) — iTerm2 URI scheme vulnerability
- [CVE-2023-46322](https://nvd.nist.gov/vuln/detail/CVE-2023-46322) — iTerm2 SSH URI vulnerability
- [Cargo OSC 8 Support](https://blog.rust-lang.org/2023/12/28/Rust-1.75.0.html) — Rust 1.75 release notes
- [ripgrep hyperlink support](https://github.com/BurntSushi/ripgrep/blob/master/CHANGELOG.md#1400-2023-11-27) — ripgrep 14.0 changelog
- [Kitty Hyperlinks](https://sw.kovidgoyal.net/kitty/kittens/hyperlinked_grep/) — Kitty hyperlink integration
