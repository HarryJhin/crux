---
title: "Shell Integration Protocols"
description: "OSC 7 CWD reporting, OSC 133 command boundaries (FinalTerm), OSC 1337 iTerm2 extensions, OSC 633 VS Code sequences, auto-injection patterns, security considerations"
date: 2026-02-12
phase: [2]
topics: [shell-integration, osc-7, osc-133, osc-1337, escape-sequences]
status: final
related:
  - terminal-emulation.md
  - ../integration/ipc-protocol-design.md
---

# Shell Integration Protocols

> 작성일: 2026-02-12
> 목적: Crux 터미널에서 쉘 통합(Shell Integration)을 구현하기 위한 프로토콜 분석 — CWD 추적, 커맨드 경계 탐지, 자동 주입 패턴

---

## 목차

1. [개요](#1-개요)
2. [OSC 7 — Current Working Directory](#2-osc-7--current-working-directory)
3. [OSC 133 — Command Boundaries (FinalTerm)](#3-osc-133--command-boundaries-finalterm)
4. [OSC 633 — VS Code Shell Integration](#4-osc-633--vs-code-shell-integration)
5. [OSC 1337 — iTerm2 Extensions](#5-osc-1337--iterm2-extensions)
6. [Auto-Injection Patterns](#6-auto-injection-patterns)
7. [Security Considerations](#7-security-considerations)
8. [Crux Implementation Recommendations](#8-crux-implementation-recommendations)

---

## 1. 개요

Shell integration allows a terminal emulator to understand the structure of shell sessions beyond raw byte streams. Instead of seeing only characters, the terminal can identify:

- **Current working directory** (OSC 7) — enables "New Tab in Same Directory", file path resolution
- **Command boundaries** (OSC 133) — prompt start/end, command output start/end, exit status
- **Rich metadata** (OSC 1337) — marks, cursor shape, cell size reporting
- **IDE integration** (OSC 633) — VS Code's extended protocol for terminal intelligence

All of these work via **in-band escape sequences**: the shell emits special OSC sequences that the terminal intercepts and acts upon without displaying them.

Sources: [FinalTerm spec](http://finalterm.org/), [iTerm2 shell integration](https://iterm2.com/documentation-shell-integration.html), [Kitty shell integration](https://sw.kovidgoyal.net/kitty/shell-integration/), [Ghostty shell integration](https://ghostty.org/docs/features/shell-integration), [VS Code terminal shell integration](https://code.visualstudio.com/docs/terminal/shell-integration)

---

## 2. OSC 7 — Current Working Directory

### Purpose

Allows the shell to report its current working directory to the terminal. This enables:
- "Open New Tab Here" / "Split Pane in CWD"
- File path completion relative to shell CWD
- Directory display in tab titles

### Escape Sequence Format

```
ESC ] 7 ; file://HOSTNAME/PATH ST
```

Where:
- `HOSTNAME` is the local hostname (from `hostname` command)
- `PATH` is the absolute path, **percent-encoded** (RFC 3986)
- `ST` is String Terminator: `ESC \` or `BEL` (`\x07`)

### Example

```
\x1b]7;file://MacBook-Pro.local/Users/jjh/Projects/crux\x1b\\
```

### Percent-Encoding Rules

Paths must be percent-encoded per RFC 3986. Characters that MUST be encoded:
- Space → `%20`
- Non-ASCII (Korean, CJK) → UTF-8 bytes percent-encoded (e.g., `한` → `%ED%95%9C`)
- Special characters: `#`, `%`, `?`, `[`, `]` and others reserved in URIs

### Shell Setup

#### Bash

```bash
# Add to ~/.bashrc or injected via ENV
__crux_osc7() {
    local hostname
    hostname=$(hostname)
    local pwd_url="file://${hostname}"
    # Percent-encode the path
    local path="${PWD}"
    local encoded=""
    local i ch
    for ((i=0; i<${#path}; i++)); do
        ch="${path:$i:1}"
        if [[ "$ch" =~ [a-zA-Z0-9/._~-] ]]; then
            encoded+="$ch"
        else
            printf -v encoded '%s%%%02X' "$encoded" "'$ch"
        fi
    done
    printf '\e]7;%s%s\e\\' "$pwd_url" "$encoded"
}
PROMPT_COMMAND="__crux_osc7${PROMPT_COMMAND:+;$PROMPT_COMMAND}"
```

#### Zsh

```zsh
# Add to ~/.zshrc or injected via ZDOTDIR
__crux_osc7() {
    emulate -L zsh
    local host="${(%):-%m}"
    # Zsh's (q) flag handles percent-encoding for us
    local pwd_url="file://${host}${PWD// /%20}"
    printf '\e]7;%s\e\\' "$pwd_url"
}
autoload -Uz add-zsh-hook
add-zsh-hook chpwd __crux_osc7
__crux_osc7  # Emit once at startup
```

#### Fish

```fish
# Add to ~/.config/fish/conf.d/crux.fish or injected via XDG_DATA_DIRS
function __crux_osc7 --on-variable PWD
    set -l hostname (hostname)
    set -l encoded (string escape --style=url -- $PWD)
    printf '\e]7;file://%s%s\e\\' $hostname $encoded
end
# Emit once at startup
__crux_osc7
```

### Terminal-Side Parsing

```rust
fn handle_osc(&mut self, params: &[&[u8]]) {
    match params[0] {
        b"7" => {
            if let Some(uri) = params.get(1) {
                let uri = std::str::from_utf8(uri).ok()?;
                if let Ok(url) = url::Url::parse(uri) {
                    if url.scheme() == "file" {
                        let path = percent_decode_str(url.path())
                            .decode_utf8()
                            .ok()?;
                        self.current_working_directory = Some(PathBuf::from(path.as_ref()));
                    }
                }
            }
        }
        // ...
    }
}
```

### Adoption

| Terminal | Support | Notes |
|----------|---------|-------|
| iTerm2 | Yes | Original popularizer |
| Kitty | Yes | Built-in shell integration |
| Ghostty | Yes | Built-in shell integration |
| WezTerm | Yes | New tab/pane uses CWD |
| Alacritty | No | By design (no tabs) |
| macOS Terminal.app | Yes | Uses for "New Tab at Folder" |

---

## 3. OSC 133 — Command Boundaries (FinalTerm)

### Purpose

Originally from the [FinalTerm](http://finalterm.org/) terminal emulator project, this protocol marks the structural boundaries of shell interactions:

```
[PromptStart]user@host:~/dir$ [CommandStart]ls -la[OutputStart]
total 42
drwxr-xr-x  5 user  staff  160 Feb 12 10:00 .
...
[CommandEnd;exit_code=0]
```

This enables:
- **Prompt-to-prompt navigation** (jump between commands)
- **Per-command output selection** (select/copy just one command's output)
- **Exit status decorations** (green/red marks next to each prompt)
- **Command palette** (list recent commands and their outputs)
- **Scrollback semantic search** (find by command, not just text)

### Escape Sequences

| Mark | Sequence | Description |
|------|----------|-------------|
| **A** | `ESC ] 133 ; A ST` | Prompt started |
| **B** | `ESC ] 133 ; B ST` | Command started (user pressed Enter) |
| **C** | `ESC ] 133 ; C ST` | Command output started (after command begins executing) |
| **D** | `ESC ] 133 ; D [; exit_code] ST` | Command finished with optional exit code |

### Typical Session Flow

```
┌──────── A (prompt start)
│ user@host:~/dir$
│──────── B (command start — user types here)
│ ls -la
│──────── C (output start — command is now running)
│ total 42
│ drwxr-xr-x  5 user  staff  160 Feb 12 10:00 .
│ -rw-r--r--  1 user  staff  1234 Feb 12 09:00 file.txt
│──────── D;0 (command finished, exit code 0)
│
└──────── A (next prompt start)
  user@host:~/dir$
```

### Shell Setup

#### Bash

```bash
__crux_prompt_start() { printf '\e]133;A\e\\'; }
__crux_command_start() { printf '\e]133;B\e\\'; }
__crux_output_start() { printf '\e]133;C\e\\'; }
__crux_command_end() { printf '\e]133;D;%s\e\\' "$1"; }

# Integrate with PS1
PS1='\[$(printf "\e]133;A\e\\")\]'"${PS1}"'\[$(printf "\e]133;B\e\\")\]'

# DEBUG trap for output start
trap '__crux_output_start' DEBUG

# PROMPT_COMMAND for command end
__crux_precmd() {
    local exit_code=$?
    __crux_command_end "$exit_code"
}
PROMPT_COMMAND="__crux_precmd${PROMPT_COMMAND:+;$PROMPT_COMMAND}"
```

#### Zsh

```zsh
__crux_precmd() {
    local exit_code=$?
    printf '\e]133;D;%d\e\\' "$exit_code"
    printf '\e]133;A\e\\'
}
__crux_preexec() {
    printf '\e]133;C\e\\'
}

autoload -Uz add-zsh-hook
add-zsh-hook precmd __crux_precmd
add-zsh-hook preexec __crux_preexec

# Mark command start in PROMPT
PROMPT="%{$(printf '\e]133;A\e\\')%}${PROMPT}%{$(printf '\e]133;B\e\\')%}"
```

#### Fish

```fish
function __crux_prompt_start --on-event fish_prompt
    printf '\e]133;A\e\\'
end

function __crux_command_start --on-event fish_preexec
    printf '\e]133;C\e\\'
end

function __crux_command_end --on-event fish_postexec
    printf '\e]133;D;%d\e\\' $status
end
```

### Terminal-Side Data Model

```rust
#[derive(Debug, Clone)]
struct CommandRegion {
    prompt_start: GridPosition,    // Mark A
    command_start: GridPosition,   // Mark B
    output_start: GridPosition,    // Mark C
    command_end: GridPosition,     // Mark D
    exit_code: Option<i32>,
    command_text: Option<String>,  // Extracted from B..C region
}

struct ShellIntegrationState {
    regions: Vec<CommandRegion>,
    current_mark: Option<MarkType>,
}
```

### Feature Implementation

| Feature | Required Marks | How |
|---------|---------------|-----|
| Prompt navigation (Cmd+Up/Down) | A | Jump between consecutive A marks |
| Select command output | C, D | Select text between C and next D |
| Exit status gutter | D | Show icon at D mark position |
| Re-run command | A, B, C | Extract text between B and C |
| Command palette | A, B, C, D | List all command regions with output |

### Adoption

| Terminal | Support | Notes |
|----------|---------|-------|
| iTerm2 | Yes | Marks + annotations |
| Kitty | Yes | Built-in shell integration, auto-injected |
| Ghostty | Yes | Built-in, prompt navigation |
| WezTerm | Yes | Semantic zones |
| VS Code Terminal | Yes (via OSC 633) | Extended variant |
| macOS Terminal.app | Partial | Cmd+Up/Down between marks |

---

## 4. OSC 633 — VS Code Shell Integration

### Purpose

VS Code's terminal defines its own shell integration protocol as an extension of OSC 133. It uses a different OSC number (633) to avoid conflicts with terminals that may handle 133 differently. The sequences carry additional metadata useful for IDE integration.

### Escape Sequences

| Sequence | Description |
|----------|-------------|
| `ESC ] 633 ; A ST` | Prompt start (same as OSC 133 A) |
| `ESC ] 633 ; B ST` | Prompt end / command start (same as 133 B) |
| `ESC ] 633 ; C ST` | Output start (same as 133 C) |
| `ESC ] 633 ; D [; exit_code] ST` | Command end (same as 133 D) |
| `ESC ] 633 ; E ; commandline [; nonce] ST` | Command line content (explicit) |
| `ESC ] 633 ; P ; key=value ST` | Property set (CWD, IsWindows, etc.) |

### Unique VS Code Extensions

**E mark (Command Line)**: Explicitly sends the command text, rather than requiring the terminal to scrape it from the grid:

```
\x1b]633;E;ls -la;some-nonce-value\x1b\\
```

The nonce prevents command replay attacks.

**P mark (Properties)**: Sets key-value metadata:

```
\x1b]633;P;Cwd=/Users/jjh/Projects/crux\x1b\\
\x1b]633;P;IsWindows=False\x1b\\
```

### Relevance for Crux

Supporting OSC 633 is valuable because:
- Claude Code runs in terminals — if Crux supports OSC 633, VS Code-style integrations work natively
- The `E` mark solves the hard problem of reliably extracting command text
- Low implementation cost since it mirrors OSC 133 semantics

**Recommendation**: Parse both OSC 133 and OSC 633 using the same handler, with OSC 633 E and P as extensions.

---

## 5. OSC 1337 — iTerm2 Extensions

### Purpose

iTerm2 defines a rich set of proprietary OSC sequences for advanced terminal features. Many have been adopted by other terminals (WezTerm, Kitty partially). Worth supporting selectively.

### Key Sequences

#### SetMark

```
ESC ] 1337 ; SetMark ST
```

Sets a bookmark at the current cursor position. User can navigate between marks with Cmd+Shift+Up/Down. Useful for error output navigation.

#### CurrentDir

```
ESC ] 1337 ; CurrentDir=/path/to/dir ST
```

Alternative to OSC 7 for CWD reporting. Simpler format (no `file://` URI encoding). Many shell integration scripts emit both OSC 7 and OSC 1337 CurrentDir.

#### ClearScrollback

```
ESC ] 1337 ; ClearScrollback ST
```

Clears the scrollback buffer without clearing the visible screen. Used by some shell tools (e.g., `clear-scrollback` aliases). Compare with `CSI 3 J` (xterm extension) which does the same thing more portably.

#### ReportCellSize

```
ESC ] 1337 ; ReportCellSize ST
```

Terminal responds with:

```
ESC ] 1337 ; ReportCellSize=HEIGHT;WIDTH ST
```

Where HEIGHT and WIDTH are in points (1/72 inch). Used by image display protocols to calculate image dimensions in cells.

#### CursorShape

```
ESC ] 1337 ; CursorShape=N ST
```

| N | Shape |
|---|-------|
| 0 | Block |
| 1 | Vertical bar |
| 2 | Underline |

More commonly handled via `DECSCUSR` (`CSI Ps SP q`) which is the standard way. Supporting both for compatibility.

#### Custom User Variables

```
ESC ] 1337 ; SetUserVar=NAME=BASE64_VALUE ST
```

Sets a named variable that the terminal can query. Used for shell-to-terminal metadata passing.

### Implementation Priority for Crux

| Sequence | Priority | Rationale |
|----------|----------|-----------|
| SetMark | High | Pairs with OSC 133 for navigation |
| CurrentDir | Medium | Redundant with OSC 7 but widely used |
| ClearScrollback | Medium | Common user expectation |
| ReportCellSize | Medium | Needed for image protocols (Phase 4) |
| CursorShape | Low | Prefer DECSCUSR standard |
| SetUserVar | Low | Niche use case |

---

## 6. Auto-Injection Patterns

### The Problem

Shell integration requires the user to add code to their shell rc files. This is fragile:
- Users forget or misconfigure
- Different shells need different setup
- Updating the integration code requires user action
- Conflicts with other terminals' integration scripts

### The Solution: Auto-Injection

Modern terminals (Kitty, Ghostty, WezTerm) inject shell integration automatically by modifying the shell's startup environment before launch.

### Injection Strategies by Shell

#### Bash

**Strategy**: Use the `ENV` environment variable.

Bash loads `$ENV` for interactive shells (when invoked as `bash -i`, or when `$ENV` is set and the shell is interactive).

```rust
fn setup_bash_injection(cmd: &mut Command, integration_dir: &Path) {
    // Bash loads $ENV for interactive shells
    // Our script sources the user's .bashrc, then adds integration
    cmd.env("ENV", integration_dir.join("bash/crux-integration.bash"));

    // Alternative: use --rcfile
    // cmd.arg("--rcfile").arg(integration_dir.join("bash/crux-integration.bash"));
}
```

The injected `crux-integration.bash`:

```bash
#!/bin/bash
# Source user's normal bashrc first
[[ -f ~/.bashrc ]] && source ~/.bashrc

# Only inject if interactive and not already injected
if [[ $- == *i* ]] && [[ -z "$CRUX_SHELL_INTEGRATION" ]]; then
    export CRUX_SHELL_INTEGRATION=1
    # ... shell integration functions ...
fi
```

#### Zsh

**Strategy**: Use `ZDOTDIR` redirect.

Zsh loads `$ZDOTDIR/.zshenv` and `$ZDOTDIR/.zshrc` on startup. Set `ZDOTDIR` to a directory containing a wrapper `.zshrc` that sources the real one.

```rust
fn setup_zsh_injection(cmd: &mut Command, integration_dir: &Path) {
    // Save original ZDOTDIR so our .zshrc can source from it
    let original = env::var("ZDOTDIR")
        .unwrap_or_else(|_| env::var("HOME").unwrap());
    cmd.env("CRUX_ORIGINAL_ZDOTDIR", &original);
    cmd.env("ZDOTDIR", integration_dir.join("zsh"));
}
```

The injected `zsh/.zshrc`:

```zsh
# Restore original ZDOTDIR
ZDOTDIR="${CRUX_ORIGINAL_ZDOTDIR:-$HOME}"
unset CRUX_ORIGINAL_ZDOTDIR

# Source user's real .zshrc
[[ -f "$ZDOTDIR/.zshrc" ]] && source "$ZDOTDIR/.zshrc"

# Only inject if interactive and not already injected
if [[ -o interactive ]] && [[ -z "$CRUX_SHELL_INTEGRATION" ]]; then
    export CRUX_SHELL_INTEGRATION=1
    # ... shell integration functions ...
fi
```

#### Fish

**Strategy**: Use `XDG_DATA_DIRS` prepend.

Fish loads vendor configuration from `$XDG_DATA_DIRS/fish/vendor_conf.d/`. Prepending our directory makes fish load our integration.

```rust
fn setup_fish_injection(cmd: &mut Command, integration_dir: &Path) {
    let base = integration_dir.join("fish");
    // fish loads $XDG_DATA_DIRS/fish/vendor_conf.d/*.fish
    let existing = env::var("XDG_DATA_DIRS")
        .unwrap_or_else(|_| "/usr/local/share:/usr/share".to_string());
    cmd.env("XDG_DATA_DIRS", format!("{}:{}", base.display(), existing));
}
```

### Feature Flags

Use the `CRUX_SHELL_FEATURES` environment variable to control which features are injected:

```bash
# Comma-separated feature flags
export CRUX_SHELL_FEATURES="osc7,osc133,cursor,sudo,title"
```

| Feature Flag | What It Injects |
|-------------|-----------------|
| `osc7` | CWD reporting via OSC 7 |
| `osc133` | Command boundary marks |
| `cursor` | Cursor shape changes (block in normal, bar in insert) |
| `sudo` | Password prompt detection for sudo |
| `title` | Window/tab title updates |
| `cwd` | OSC 1337 CurrentDir (redundant with osc7 but some tools expect it) |

### Reference Implementations

| Terminal | Injection Method | Source |
|----------|-----------------|--------|
| Kitty | ZDOTDIR (zsh), ENV (bash), XDG_DATA_DIRS (fish) | [kitty/shell-integration/](https://github.com/kovidgoyal/kitty/tree/master/shell-integration) |
| Ghostty | Same as Kitty with minor variations | [ghostty shell integration](https://ghostty.org/docs/features/shell-integration) |
| WezTerm | ZDOTDIR (zsh), custom sourcing (bash) | [wezterm shell-integration/](https://github.com/wezterm/wezterm/tree/main/assets/shell-integration) |
| VS Code | Injects scripts via terminal profiles | [vscode shell integration](https://github.com/microsoft/vscode/tree/main/src/vs/workbench/contrib/terminal/browser/media) |

---

## 7. Security Considerations

### Never Inject into Non-Interactive Shells

Auto-injection must only activate for interactive shells. Non-interactive shells (scripts, `ssh remote-command`, cron) should not have integration code that modifies output or PS1.

```bash
# Guard in injected script
if [[ $- != *i* ]]; then
    return  # Non-interactive, skip injection
fi
```

### Sanitize Command Text

OSC 133 / 633 regions may capture command text. Before storing or transmitting:

```rust
fn sanitize_command(text: &str) -> String {
    text.chars()
        .filter(|c| !c.is_control() || *c == '\t')
        .take(4096)  // Limit length
        .collect()
}
```

### Percent-Encode Paths in OSC 7

Paths in `file://` URIs must be percent-encoded. A malicious directory name could inject control characters:

```
# Malicious: mkdir $'\e]7;file://evil.com/\e\\'
# Must encode ESC as %1B
```

### Unset Environment Variables After Reading

The injection env vars (`ZDOTDIR`, `CRUX_ORIGINAL_ZDOTDIR`, etc.) should be cleaned up after use to prevent leaking to child processes:

```zsh
# In the injected .zshrc
ZDOTDIR="${CRUX_ORIGINAL_ZDOTDIR:-$HOME}"
unset CRUX_ORIGINAL_ZDOTDIR
# The user's real ZDOTDIR is restored
```

### Validate OSC 7 Hostname

When parsing OSC 7, verify the hostname matches the local machine:

```rust
fn validate_osc7_host(url: &Url) -> bool {
    match url.host_str() {
        None => true,  // No host = local
        Some("") => true,
        Some("localhost") => true,
        Some(host) => {
            // Compare against actual hostname
            hostname::get()
                .map(|h| h.to_string_lossy().eq_ignore_ascii_case(host))
                .unwrap_or(false)
        }
    }
}
```

This prevents a remote `ssh` session from injecting a local file path as CWD.

### Escape Sequence Injection via Prompt

If a user's PS1 contains unescaped user input (e.g., git branch name), it could inject false OSC 133 marks. The terminal should validate mark ordering (A → B → C → D → A) and discard out-of-order marks.

---

## 8. Crux Implementation Recommendations

### Phase 2 (Minimum Viable)

1. **Parse OSC 7**: Extract CWD for "New Tab Here" / "Split Pane in CWD"
2. **Parse OSC 133 A, B, C, D**: Store command regions in a per-terminal data structure
3. **Prompt navigation**: Cmd+Shift+Up/Down to jump between marks
4. **Auto-injection for zsh**: ZDOTDIR redirect (most common macOS shell)
5. **Feature flag env var**: `CRUX_SHELL_FEATURES` for user control

### Phase 2+ (Enhanced)

6. **Auto-injection for bash and fish**: ENV and XDG_DATA_DIRS
7. **Exit status decorations**: Gutter markers (green check / red X)
8. **Per-command output selection**: Cmd+click on prompt to select that command's output
9. **Parse OSC 633 E, P**: VS Code compatibility
10. **Parse OSC 1337 SetMark, CurrentDir**: iTerm2 compatibility

### Architecture

```rust
/// Shell integration state per terminal instance
pub struct ShellIntegration {
    /// Current working directory from OSC 7
    cwd: Option<PathBuf>,

    /// Command regions from OSC 133
    regions: Vec<CommandRegion>,

    /// Current incomplete region being built
    current_region: PartialRegion,

    /// iTerm2 marks from OSC 1337 SetMark
    marks: Vec<GridPosition>,
}

/// Injector that sets up env vars before shell launch
pub struct ShellInjector {
    integration_dir: PathBuf,
    features: HashSet<String>,
}

impl ShellInjector {
    pub fn inject(&self, cmd: &mut Command, shell: ShellType) {
        match shell {
            ShellType::Zsh => self.inject_zsh(cmd),
            ShellType::Bash => self.inject_bash(cmd),
            ShellType::Fish => self.inject_fish(cmd),
            _ => {} // Unknown shell, no injection
        }
        cmd.env("CRUX_SHELL_FEATURES",
            self.features.iter().cloned().collect::<Vec<_>>().join(","));
        cmd.env("TERM_PROGRAM", "crux");
        cmd.env("TERM_PROGRAM_VERSION", env!("CARGO_PKG_VERSION"));
    }
}
```

### Storage and Resource Files

```
crux-app/
├── resources/
│   └── shell-integration/
│       ├── bash/
│       │   └── crux-integration.bash
│       ├── zsh/
│       │   ├── .zshenv   (minimal, restores ZDOTDIR)
│       │   └── .zshrc    (sources real .zshrc + integration)
│       └── fish/
│           └── vendor_conf.d/
│               └── crux-integration.fish
```

### Testing

1. **Unit tests**: Parse synthetic OSC 7/133/633/1337 sequences
2. **Integration tests**: Launch each shell (bash, zsh, fish), verify marks appear
3. **Security tests**: Malicious directory names, out-of-order marks, hostname validation
4. **Manual verification**: `echo -e '\e]133;A\e\\'` should create a mark at cursor position

---

## Sources

- [FinalTerm Escape Sequences](http://finalterm.org/) — Original OSC 133 specification
- [iTerm2 Proprietary Escape Codes](https://iterm2.com/documentation-escape-codes.html) — OSC 1337 reference
- [Kitty Shell Integration](https://sw.kovidgoyal.net/kitty/shell-integration/) — Auto-injection architecture
- [Ghostty Shell Integration](https://ghostty.org/docs/features/shell-integration) — Feature flags pattern
- [VS Code Terminal Shell Integration](https://code.visualstudio.com/docs/terminal/shell-integration) — OSC 633 spec
- [WezTerm Shell Integration](https://wezfurlong.org/wezterm/shell-integration.html) — Semantic zones
- [XTerm Control Sequences](https://invisible-island.net/xterm/ctlseqs/ctlseqs.html) — OSC standard reference
