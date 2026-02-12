# Crux Implementation Plan

> Detailed phased implementation plan for the Crux terminal emulator
> Created: 2026-02-11 | Updated: 2026-02-12
> End Goal: Homebrew distribution + Claude Code Feature Request + Native MCP Server integration

---

## Overview

6 phases, each building on the previous. Each phase produces a usable milestone.

**Strategic context**: Claude Code is **not open source** (proprietary, bundled `cli.js`). Direct PR to `anthropics/claude-code` core is not possible. Instead, Crux will build a CLI that perfectly matches Claude Code's `PaneBackend` interface (13 methods, reverse-engineered), then request integration via Feature Request issue and community engagement.

---

## Phase 1: Basic Terminal (MVP)

**Goal**: A single terminal window that renders shell output and accepts keyboard input.

**Duration estimate**: Foundation phase -- most critical to get right.

### 1.1 Project Scaffolding

- [x] Initialize Cargo workspace with `crates/` structure
  - Workspace root: `resolver = "2"`, `members` array for all crates
  - Crate dependency graph: `crux-protocol` (leaf) -> `crux-terminal` -> `crux-terminal-view` -> `crux-app` (root)
  - `crux-ipc`, `crux-clipboard` as initially empty crates for Phase 2/3
- [x] Set up `crux-app` crate with GPUI application bootstrap
  - `Application::new().run()` -> `cx.open_window()` -> Root view
  - Window default size: 800x600
  - `FocusHandle` for keyboard event capture
- [x] Set up `crux-terminal` crate for terminal entity
- [x] Set up `crux-terminal-view` crate for rendering
- [x] Set up `crux-protocol` crate for shared types
- [x] Configure dependencies from crates.io (not git):
  - `gpui = "0.2.2"` (crates.io -- faster builds, better caching than git dep)
  - `gpui-component = "0.5.1"` (Phase 2, but configure early)
  - `alacritty_terminal = "0.25"`
  - `portable-pty = "0.9"`
- [x] **build.rs is NOT needed** -- GPUI handles Metal shader compilation and framework linking internally
- [x] **Xcode.app full install required** (Command Line Tools alone insufficient for Metal shaders)
  - Verify: `xcrun -sdk macosx metal --version`
  - Switch if needed: `sudo xcode-select --switch /Applications/Xcode.app/Contents/Developer`
- [x] Dev profile optimization: `opt-level = 1` for GPUI rendering performance
  - `[profile.dev.package."*"] opt-level = 2` for dependency optimization
- [x] Release profile: `lto = "thin"`, `codegen-units = 1`, `strip = "symbols"`
- [x] Create `resources/Info.plist` with `LSMinimumSystemVersion = 13.0`, `CFBundleIdentifier = com.crux.terminal`

### 1.2 Terminfo Entry (`crux.terminfo`)

- [x] Create `extra/crux.terminfo` source file
- [x] Use **fragment pattern**: `crux+common` (shared capabilities), `xterm-crux` (256-color), `crux` (alias), `crux-direct` (true color)
- [x] TERM name strategy: **`xterm-crux`** (following Ghostty/Kitty/Rio pattern)
  - `xterm-` prefix ensures compatibility (many apps check for "xterm" substring)
  - Ghostty tried `ghostty` without prefix, hit compatibility issues, switched to `xterm-ghostty`
- [x] Independent definition (not `use=xterm-256color`) -- self-contained like Alacritty
- [x] Include modern capabilities:
  - `Tc` (true color, tmux), `RGB` (true color, ncurses)
  - `Su` (styled underlines), `Smulx` (underline style: `\E[4\:%p1%dm`)
  - `Setulc` (underline color: RGB via `CSI 58:2::R:G:B m`)
  - `Ss`/`Se` (cursor style DECSCUSR), `Ms` (clipboard OSC 52)
  - `Sync` (synchronized output Mode 2026)
  - `XT` (xterm compat -- auto-enables bracketed paste, focus events)
  - `hs`/`tsl`/`fsl`/`dsl` (status line for nvim)
  - SGR mouse (`kmous=\E[<`), bracketed paste (`BD`/`BE`/`PS`/`PE`), focus events (`Dsfcs`/`Enfcs`)
  - F1-F63 key definitions (F1-F12 + Shift/Ctrl/Meta variants)
- [x] Compile with `tic -x` (the `-x` flag is critical for non-standard extensions like `Tc`, `Su`)
  - Install: `tic -x -e xterm-crux,crux,crux-direct extra/crux.terminfo`
  - Verify: `infocmp -x xterm-crux`
- [x] Bundle in app: `Crux.app/Contents/Resources/terminfo/x/xterm-crux`
  - Terminfo source embedded via `include_str!`, auto-installed to `~/.terminfo/` via `tic` at launch
- [x] Fallback logic: if `xterm-crux` terminfo not found, fall back to `TERM=xterm-256color`

### 1.3 Terminal Entity (crux-terminal)

- [x] Integrate `alacritty_terminal = "0.25"` as VT parser
- [x] Create `CruxTerminal` entity wrapping `Term<CruxListener>`
- [x] Implement `CruxListener` for alacritty events (Title, Wakeup, Bell, PtyWrite)
- [x] Integrate `portable-pty = "0.9"` for PTY management
- [x] Shell spawning with correct environment variables:
  - `TERM=xterm-crux` (with fallback to `xterm-256color`)
  - `COLORTERM=truecolor`
  - `TERM_PROGRAM=Crux`, `TERM_PROGRAM_VERSION=x.y.z`
  - `LANG` inherited from system
- [x] Shell selection logic: config file -> `$SHELL` -> `/etc/passwd` -> `/bin/zsh`
  - Login shell (`-l` flag) by default
  - macOS: `dscl . -read /Users/$USER UserShell` as fallback
- [x] PTY I/O event loop (read PTY -> feed parser -> update grid -> notify render)
- [x] Event batching: max 100 events or 4ms window (Zed pattern)
- [x] PTY resize on window/pane size change (`TIOCSWINSZ` -> `SIGWINCH`)
- [x] `TerminalContent` render snapshot: cells, cursor, mode, display_offset
- [x] Alternate screen buffer handling (DECSET 1049) -- `alacritty_terminal` handles this via `inactive_grid`
- [x] Process lifecycle management:
  - SIGHUP -> SIGTERM -> SIGKILL sequence on close
  - `waitpid` to prevent zombie processes
  - Graceful cleanup on app exit

### 1.4 Terminal Rendering (crux-terminal-view)

- [x] Implement `CruxTerminalElement` as GPUI Element
- [x] Cell grid rendering with monospace font
- [x] Text run batching (`BatchedTextRun`) -- group cells with same style
- [x] Background color rectangles with horizontal merging
- [x] Cursor rendering (block, bar, underline shapes)
  - Cursor blinking timer (GPUI periodic repaint)
  - DECSCUSR (`\e[N q`) cursor shape changes
  - Hollow cursor when unfocused
- [x] Basic color support: 16 ANSI colors + default fg/bg
- [x] Font metrics: cell width/height calculation from primary font
- [x] CJK wide character rendering (2-cell width via `unicode-width`)

### 1.5 Keyboard Input

- [x] GPUI key event handling (`on_key_down`, Action system, `FocusHandle`)
- [x] ASCII character input -> PTY write (UTF-8 bytes direct)
- [x] Control key combinations: Ctrl+A..Z -> 0x01..0x1A (C0 control codes)
  - Ctrl+C (0x03 interrupt), Ctrl+D (0x04 EOF), Ctrl+Z (0x1A suspend)
  - Ctrl+[ = ESC (0x1B), Ctrl+\\ = SIGQUIT (0x1C)
- [x] Special keys:
  - Enter: 0x0D, Tab: 0x09, Backspace: 0x7F (modern standard, not 0x08)
  - Shift+Tab: `CSI Z` (backtab)
  - Escape: 0x1B
- [x] Cursor keys -- two modes based on DECCKM:
  - **Normal mode** (default): `CSI A/B/C/D` (Up/Down/Right/Left)
  - **Application mode** (DECCKM ON): `SS3 A/B/C/D`
  - With modifiers (always CSI): `CSI 1;{mod} A/B/C/D`
- [x] Function keys F1-F12:
  - F1-F4: `SS3 P/Q/R/S` (no modifier) or `CSI 1;{mod} P/Q/R/S`
  - F5-F12: `CSI {15,17,18,19,20,21,23,24} ~` (note: discontinuous numbers)
- [x] Editing/navigation keys:
  - Home/End: `CSI H` / `CSI F` (xterm style, recommended)
  - Insert/Delete/PgUp/PgDn: `CSI {2,3,5,6} ~`
- [x] Modifier encoding: `modifier_param = 1 + (Shift:1 | Alt:2 | Ctrl:4)` bits
  - Param omitted when no modifiers (send `CSI A` not `CSI 1;1 A`)
- [x] Alt key handling: ESC prefix for characters (`Alt+a` -> `ESC a`)
- [x] macOS Option key: configurable `option_as_alt` setting
  - Values: `left` (default), `right`, `both`, `none`
  - Left Option as Meta, Right Option for character composition
  - Detect left/right via `ModifiersKeyState`

### 1.6 Text Selection & Copy

- [x] Mouse click-drag text selection (GPUI mouse event handling)
- [x] Double-click: word selection
- [x] Triple-click: line selection
- [x] Cmd+A: select all
- [x] Selection highlight rendering (inverted colors or highlight overlay)
- [x] Cmd+C: copy selected text to system clipboard
  - Use `alacritty_terminal`'s `Selection` API and `selection_to_string()`
- [x] Shift+click to force selection when terminal mouse mode is active (1000/1002/1003)

### 1.7 Basic Features

- [x] 256 color + True color (24-bit RGB) SGR rendering
- [x] Bold, italic, underline, strikethrough text styles
- [x] Scrollback buffer (default 10,000 lines)
- [x] Mouse scroll for scrollback navigation
- [x] Window title from OSC 0/2 sequences
- [x] Bell notification (visual flash via `flash` capability or system sound)
  - Bell rate limiting (ignore rapid consecutive bells)
- [x] Synchronized output (Mode 2026): buffer rendering during `CSI ? 2026 h` .. `CSI ? 2026 l`
- [x] License decision: MIT recommended (compatible with GPUI Apache-2.0, alacritty_terminal Apache-2.0, portable-pty MIT)

### Milestone 1 Deliverable

A single-window terminal that can:
- Launch default shell (zsh/bash)
- Render colored output correctly
- Handle keyboard input including all special keys and modifier combos
- Support cursor keys in both Normal and Application modes
- Select text with mouse, copy with Cmd+C
- Scroll through output history
- Run programs like `vim`, `htop`, `git log` correctly
- Set `TERM=xterm-crux` with custom terminfo

### Key Dependencies

```toml
gpui = "0.2.2"
# gpui-component = "0.5.1"  # Phase 2
alacritty_terminal = "0.25"
portable-pty = "0.9"
anyhow = "1"
log = "0.4"
env_logger = "0.11"
libc = "0.2"
```

---

## Phase 2: Tabs, Split Panes, IPC & Shell Integration

**Goal**: Multi-pane terminal with programmatic control via CLI and modern shell integration.

### 2.1 Tab System

- [x] Integrate gpui-component `Tabs` + `TabBar`
  - DockArea with TabPanel, CruxTerminalPanel implementing Panel trait
- [x] Tab creation (Cmd+T), closing (Cmd+W), switching (Cmd+1-9)
  - Keybindings registered, NewTab implemented via DockArea::add_panel, CloseTab/switching stubbed
- [ ] Tab reordering via drag
- [ ] Tab title from active pane's shell title
- [ ] Tab close confirmation when process is running

### 2.2 Split Panes

- [x] Integrate gpui-component `DockArea` + `ResizablePanel`
  - DockArea initialized as center layout, DockItem::tab for terminal panels
- [ ] Horizontal split (Cmd+D) and vertical split (Cmd+Shift+D)
  - Keybindings registered, action handlers stubbed
- [ ] Resizable dividers between panes
- [ ] Pane focus navigation (Cmd+[/], Cmd+Alt+Arrow)
- [x] Pane zoom toggle (Cmd+Shift+Enter)
  - Keybinding registered, action handler stubbed
- [ ] Pane close with graceful process termination
- [ ] Top-level window split vs individual pane split

### 2.3 Pane Manager

- [x] `PaneManager` with `Arc<RwLock<HashMap<PaneId, PaneState>>>`
  - DockArea serves as pane manager via its internal tree structure
- [ ] Atomic pane ID generation (`AtomicU64`)
- [x] Pane lifecycle: create -> active -> close
  - Create via NewTab action, DockArea manages lifecycle
- [ ] Pane event broadcasting (`broadcast::Sender<PaneEvent>`)
- [ ] Track pane hierarchy (parent-child for splits)

### 2.4 Shell Integration (OSC 133 & OSC 7)

- [ ] OSC 133 (FinalTerm) prompt marking:
  - `\e]133;A\a` (prompt start), `\e]133;B\a` (command start)
  - `\e]133;C\a` (output start), `\e]133;D;exit_code\a` (command complete)
  - Not supported by alacritty_terminal; byte-stream scanner infrastructure ready
- [x] OSC 7 CWD tracking: `\e]7;file://hostname/path\a`
  - Byte-stream scanner in PTY read loop, percent-decoding, CWD stored in CruxTerminal
- [ ] Semantic zones: prompt, command, output regions
- [ ] Smart navigation: jump between prompts (Cmd+Up/Down)
- [ ] Shell integration scripts for zsh/bash/fish (bundled)

### 2.5 IPC Server (crux-ipc)

- [ ] Unix domain socket server with `tokio::net::UnixListener`
- [ ] Socket path: `$CRUX_SOCKET` or `$XDG_RUNTIME_DIR/crux/gui-sock-$PID`
- [ ] Socket permissions: `0o600` (owner-only)
- [ ] Peer credential verification (`UCred`)
- [ ] JSON-RPC 2.0 message handling with length-prefix framing
- [ ] Connection handshake (`crux:handshake`)

### 2.6 Crux Protocol -- Pane Control (P0)

- [ ] `crux:pane/split` -- split pane, return new pane_id
- [ ] `crux:pane/send-text` -- send text with optional bracketed paste
- [ ] `crux:pane/get-text` -- capture pane content (with scrollback access)
- [ ] `crux:pane/list` -- list all panes with metadata (JSON)
- [ ] `crux:pane/activate` -- focus a pane
- [ ] `crux:pane/close` -- close pane (graceful or forced)
- [ ] `crux:window/create` -- new window
- [ ] `crux:window/list` -- list windows

### 2.7 CLI Client

- [ ] `crux cli` binary (same binary, subcommand)
- [ ] Socket discovery: `$CRUX_SOCKET` -> runtime dir scan
- [ ] `crux cli split-pane [--right|--left|--top|--bottom] [--percent N] [-- COMMAND]`
- [ ] `crux cli send-text [--pane-id ID] [--no-paste] [TEXT]`
- [ ] `crux cli get-text [--pane-id ID] [--start-line N] [--escapes]`
- [ ] `crux cli list [--format table|json]`
- [ ] `crux cli activate-pane --pane-id ID`
- [ ] Stdin pipe support for `send-text`
- [ ] Human-readable table output + JSON output

### 2.8 Environment Variable Propagation

- [ ] Set `CRUX_SOCKET` in all child PTY processes
- [ ] Set `CRUX_PANE` to current pane ID in each PTY

### 2.9 MCP Server — Core (crux-mcp)

Native MCP (Model Context Protocol) server embedded in Crux, enabling all MCP-compatible AI agents (Claude Desktop, Claude Code, Cursor, etc.) to programmatically control Crux. See [research/integration/mcp-integration.md](research/integration/mcp-integration.md) for full design.

**Architecture**: Separate Tokio runtime thread + Unix socket (`~/.crux/mcp.sock`), communicating with GPUI main thread via `mpsc` channel.

- [ ] Create `crux-mcp` crate with `rmcp` SDK integration
  - `rmcp = { version = "0.15", features = ["server", "macros", "transport-io"] }`
  - `tokio`, `axum` for Unix socket / HTTP transport
- [ ] MCP server lifecycle: start on app launch, stop on app exit
  - Separate thread with `tokio::runtime::Runtime`
  - `mpsc::Sender<PaneCommand>` for MCP → GPUI commands
  - `oneshot::Sender` for GPUI → MCP responses
- [ ] Unix socket transport at `~/.crux/mcp.sock`
  - File permissions `0o600` (owner-only)
  - Cleanup on graceful shutdown
- [ ] HTTP localhost fallback transport (`127.0.0.1:{port}`)
- [ ] MCP capability negotiation: `tools` + `resources`
- [ ] Pane management tools (5):
  - [ ] `crux_create_pane` — split pane (horizontal/vertical), return PaneInfo
  - [ ] `crux_close_pane` — close by pane_id
  - [ ] `crux_focus_pane` — switch focus
  - [ ] `crux_list_panes` — all panes with metadata (id, pid, cwd, size)
  - [ ] `crux_resize_pane` — adjust cols/rows
- [ ] Command execution tools (5):
  - [ ] `crux_execute_command` — run command, return exit_code + stdout
  - [ ] `crux_send_keys` — raw key sequences (Ctrl+C, Enter, arrows)
  - [ ] `crux_send_text` — type text into pane
  - [ ] `crux_get_output` — capture recent N lines
  - [ ] `crux_wait_for_output` — block until pattern matches (with timeout)
- [ ] State inspection tools (5):
  - [ ] `crux_get_current_directory` — shell CWD (via OSC 7)
  - [ ] `crux_get_running_process` — foreground process name + pid
  - [ ] `crux_get_pane_state` — full snapshot (cols, rows, cursor, scroll)
  - [ ] `crux_get_selection` — currently selected text
  - [ ] `crux_get_scrollback` — scrollback buffer with offset/limit pagination
- [ ] Content capture tools (5):
  - [ ] `crux_screenshot_pane` — GPUI render to base64 PNG
  - [ ] `crux_get_raw_text` — ANSI-stripped plain text
  - [ ] `crux_get_formatted_output` — ANSI codes preserved
  - [ ] `crux_save_session` — serialize session state
  - [ ] `crux_restore_session` — restore saved session
- [ ] MCP resources: expose pane scrollback as `crux://pane/{id}/scrollback`

### 2.10 MCP Bridge Binary (crux-mcp-bridge)

stdio ↔ Unix socket bridge for Claude Desktop compatibility (Claude Desktop only supports stdio transport).

- [ ] Create `crux-mcp-bridge` binary crate
  - Reads JSON-RPC from stdin, forwards to `~/.crux/mcp.sock`
  - Reads responses from socket, writes to stdout
- [ ] Socket discovery: `$CRUX_MCP_SOCKET` → `~/.crux/mcp.sock`
- [ ] Connection retry with backoff (Crux may not be running yet)
- [ ] Claude Desktop config example:
  ```json
  {
    "mcpServers": {
      "crux-terminal": {
        "command": "crux-mcp-bridge",
        "args": ["--socket", "~/.crux/mcp.sock"]
      }
    }
  }
  ```

### Milestone 2 Deliverable

A multi-pane terminal where:
- Tabs can be created, switched, reordered, closed
- Panes can be split horizontally/vertically and resized
- Shell integration marks prompts and tracks CWD
- `crux cli split-pane` creates a new pane and returns its ID
- `crux cli send-text --pane-id 42 "ls\n"` sends text to any pane
- `crux cli list --format json` returns structured pane info
- **20 MCP tools expose full terminal control to any AI agent**
- **Claude Desktop can control Crux via `crux-mcp-bridge`**
- Claude Code could theoretically use Crux as an Agent Teams backend

---

## Phase 3: IME & Rich Clipboard

**Goal**: First-class Korean/CJK input and image clipboard support.

### 3.1 NSTextInputClient Implementation

- [ ] Implement full `NSTextInputClient` protocol via `objc2-app-kit`
- [ ] `insertText:replacementRange:` -- commit text to PTY
- [ ] `setMarkedText:selectedRange:replacementRange:` -- store preedit overlay (NOT sent to PTY)
- [ ] `unmarkText` -- commit marked text
- [ ] `hasMarkedText`, `markedRange`, `selectedRange` -- state queries
- [ ] `firstRectForCharacterRange:actualRange:` -- cell coord -> view coord -> window coord -> screen coord
- [ ] `doCommandBySelector:` -- handle insertNewline, deleteBackward, insertTab
- [ ] `validAttributesForMarkedText` -- return empty array
- [ ] `characterIndexForPoint:` -- screen coord to cell position

### 3.2 Composition Overlay Rendering

- [ ] Preedit text rendered as overlay on terminal grid
- [ ] Underline style for composition text
- [ ] Distinct color for composing vs committed text
- [ ] Correct overlay positioning for wide (CJK) characters
- [ ] Overlay cleanup on composition cancel/commit

### 3.3 Korean IME Hardening

Based on failure analysis of Alacritty, Ghostty, WezTerm:

- [ ] **Modifier key isolation**: Ignore standalone Ctrl/Shift/Cmd during `hasMarkedText`
  - Prevents Ghostty-style preedit destruction (#4634)
- [ ] **Event deduplication**: Filter duplicate space/text from IME commit + keyboard event
  - Prevents Alacritty-style double space (#8079)
  - Window: 10ms dedup window for identical text
- [ ] **IME crash resilience**: Timeout on IME event processing (100ms)
  - Prevents Alacritty-style freeze (#4469)
  - Reset IME state on timeout
- [ ] **NFD normalization**: Convert decomposed Hangul (NFD) to composed (NFC) before rendering
- [ ] **Wide character cursor**: Correct cursor positioning after 2-cell CJK characters

### 3.4 Rich Clipboard (crux-clipboard)

- [ ] NSPasteboard content type detection (text, HTML, image, file URL)
- [ ] Image paste (Cmd+V):
  - Read PNG/TIFF from pasteboard
  - Convert TIFF to PNG if needed
  - Save to `/tmp/crux-clipboard/paste-{timestamp}.png`
  - Transmit file path to application via sideband (not PTY text stream)
- [ ] `clipboard-rs` integration for cross-platform clipboard API
- [ ] Direct `objc2-app-kit` NSPasteboard access for image data

### 3.5 Drag & Drop

- [ ] Register `NSView` for drag types: fileURL, PNG, TIFF, string
- [ ] `NSDraggingDestination` protocol implementation
- [ ] File drop: insert file path as text into PTY
- [ ] Image drop: save to temp file, handle like clipboard image
- [ ] Visual drop indicator (highlight pane border)

### 3.6 Crux Protocol -- Clipboard & IME (P1)

- [ ] `crux:clipboard/read` -- read clipboard with type preference
- [ ] `crux:clipboard/write` -- write text/HTML/image to clipboard
- [ ] `crux:ime/get-state` -- query IME composition state
- [ ] `crux:ime/set-input-source` -- switch input method programmatically
- [ ] `crux:events/subscribe` -- pane events, focus events

### 3.7 Vim Mode IME Auto-Switch

- [ ] Detect cursor shape change escape sequences from PTY output:
  - `\e[2 q` (block) = Normal mode -> switch to ASCII
  - `\e[6 q` (bar) = Insert mode -> restore previous IME
- [ ] `TISSelectInputSource` API for programmatic IME switching
- [ ] User-configurable enable/disable

### Milestone 3 Deliverable

- Korean input works flawlessly
- IME candidate window appears at correct cursor position
- No freezing, no double spaces, no preedit destruction
- Cmd+V pastes images as temp file paths for Claude Code
- Drag & drop files/images into terminal
- Vim users can type Korean in Insert mode with auto-switch in Normal mode

---

## Phase 4: Markdown Preview, Links & Graphics

**Goal**: Rich content rendering and terminal graphics protocol support.

### 4.1 Link Detection

- [ ] URL regex pattern matching on terminal output
- [ ] OSC 8 hyperlink protocol support (`\e]8;;URL\e\\text\e]8;;\e\\`)
- [ ] Cmd+click to open links in default browser
- [ ] Visual hover indicator (underline + color change)
- [ ] Right-click context menu: Copy Link, Open Link

### 4.2 Markdown Preview

- [ ] Detect markdown output patterns (headings, lists, code blocks)
- [ ] Inline markdown rendering using gpui-component's Markdown support
- [ ] Toggle between raw and rendered modes (Cmd+Shift+M)
- [ ] Code block syntax highlighting
- [ ] Markdown preview pane (side panel option)

### 4.3 Kitty Graphics Protocol (Priority 1)

- [ ] APC sequence parsing (`\e_G...;\e\\`)
- [ ] Image transmission: direct (base64), file path, temp file, shared memory
- [ ] Image formats: PNG, RGBA, RGB
- [ ] Chunked transfer (multi-part `m=1` / `m=0`)
- [ ] Image placement: cursor position, z-index
- [ ] Image display within terminal grid
- [ ] Image deletion commands
- [ ] Response protocol (`OK` / error messages)

### 4.4 iTerm2 Image Protocol (Priority 2)

- [ ] OSC 1337 inline image display
- [ ] `imgcat` compatibility
- [ ] Supported formats: PNG, JPEG, GIF, PDF
- [ ] Width/height specification (cells, pixels, percent, auto)

### 4.5 Sixel Graphics (Priority 3)

- [ ] DCS Sixel sequence parsing
- [ ] 256-color Sixel rendering
- [ ] tmux Sixel passthrough compatibility

### 4.6 Font System

- [ ] CoreText font fallback cascade:
  - Primary font (user-configured)
  - Korean CJK (Apple SD Gothic Neo)
  - Japanese CJK (Hiragino Sans)
  - Chinese CJK (PingFang SC)
  - Emoji (Apple Color Emoji)
  - Symbols (Nerd Font compatible)
- [ ] `CTFontCopyDefaultCascadeListForLanguages` for Han Unification
- [ ] Ambiguous width character setting (1-cell or 2-cell, user-configurable)
- [ ] Grapheme cluster rendering (Mode 2027 support)

### 4.7 Kitty Keyboard Protocol (Progressive Enhancement)

- [ ] `CSI > flags u` push / `CSI < u` pop / `CSI ? u` query
- [ ] Flag 1 (Disambiguate): resolve Tab/Ctrl+I, Enter/Ctrl+M, Escape/Ctrl+[ ambiguity
- [ ] Flag 2 (Report events): repeat/release event reporting
- [ ] Flag 4 (Report alternates): shifted and base-layout variants
- [ ] Flag 8 (Report all keys): all keys as CSI sequences
- [ ] Flag 16 (Report text): include text codepoints in CSI sequences
- [ ] Stack depth limit: 4096 (matches Alacritty)
- [ ] Full CSI u format: `CSI unicode-key-code:alternate-keys ; modifiers:event-type ; text-as-codepoints u`

### 4.8 Crux Protocol -- Render (P2)

- [ ] `crux:render/image` -- display image inline
- [ ] `crux:render/markdown` -- render markdown block
- [ ] Custom OSC 7700-7799 in-band protocol for PTY apps

### Milestone 4 Deliverable

- Clickable URLs in terminal output
- Markdown content rendered with formatting
- `imgcat` works for inline images
- Kitty graphics protocol for modern CLI tools
- Kitty keyboard protocol for enhanced key handling
- Correct CJK font rendering with proper fallback chain

---

## Phase 5: tmux Compatibility, Claude Code Integration & Configuration

**Goal**: Full tmux compatibility, Claude Code Feature Request submission, and production polish.

### 5.1 tmux Compatibility Verification

- [ ] All tmux-required VT100 features working:
  - Cursor movement (CSI A/B/C/D/H)
  - Screen erase (CSI J/K)
  - Scroll regions (DECSTBM)
  - Character attributes (SGR)
- [ ] `TERM=tmux-256color` support
- [ ] True color passthrough: `terminal-features` and `terminal-overrides`
- [ ] Mouse modes: 1000, 1002, 1003, 1006 (SGR encoding)
- [ ] Bracketed paste mode (CSI 2004 h/l)
- [ ] Focus events (CSI 1004 h, CSI I/O)
- [ ] DECSCUSR cursor shape changes
- [ ] Left-right margins (DECLRMM) for tmux horizontal split optimization

### 5.2 tmux Control Mode (Long-term)

- [ ] DCS `\033P1000p` detection and response
- [ ] `%begin`/`%end`/`%error` command response parsing
- [ ] `%output` notification handling -- route pane output to native panes
- [ ] `%window-add`/`%window-close` -- map to Crux tabs
- [ ] `%pane-mode-changed` event handling
- [ ] Flow control (`%pause`/`%continue`)
- [ ] `refresh-client -C WxH` for size negotiation
- [ ] Session reconnection and state restoration

### 5.3 Claude Code Agent Teams -- Feature Request Strategy

Claude Code's `PaneBackend` interface has **13 methods** (reverse-engineered from bundled `cli.js`):

```typescript
interface PaneBackend {
  type: string;                     // "crux"
  displayName: string;              // "Crux"
  supportsHideShow: boolean;
  isAvailable(): Promise<boolean>;
  isRunningInside(): Promise<boolean>;
  createTeammatePaneInSwarmView(name, color): Promise<{paneId, isFirstTeammate}>;
  sendCommandToPane(paneId, command, external?): Promise<void>;
  setPaneBorderColor(paneId, color, external?): Promise<void>;
  setPaneTitle(paneId, title, color, external?): Promise<void>;
  enablePaneBorderStatus(windowTarget?, external?): Promise<void>;
  rebalancePanes(target, withLeader): Promise<void>;
  killPane(paneId, external?): Promise<boolean>;
  hidePane(paneId, external?): Promise<boolean>;
  showPane(paneId, target, external?): Promise<boolean>;
  getCurrentPaneId(): Promise<string | null>;
  getCurrentWindowTarget(): Promise<string | null>;
  getCurrentWindowPaneCount(target?, external?): Promise<number | null>;
}
```

**Detection mechanism**: `TERM_PROGRAM=Crux` (matching tmux's `$TMUX`, iTerm2's `TERM_PROGRAM=iTerm.app` pattern)

**Strategy (Feature Request, not direct PR):**

- [ ] Build Crux CLI to match PaneBackend interface exactly:
  - `crux cli split-pane --direction right|bottom -- <cmd>` (returns pane ID on stdout)
  - `crux cli list-panes [--format json]` (JSON: `[{paneId, windowId, tabId, title, size, cwd, active}]`)
  - `crux cli kill-pane --pane-id <id>` (exit 0=success, 2=pane not found)
  - `crux cli send-keys --pane-id <id> <text> Enter`
  - `crux cli focus-pane --pane-id <id>`
  - `crux cli set-pane-title --pane-id <id> <title>`
  - `crux cli set-pane-border-color --pane-id <id> <color>`
  - `crux cli set-layout --target <window> tiled|main-vertical`
  - `crux cli hide-pane --pane-id <id>` / `crux cli show-pane --pane-id <id> --target <window>`
- [ ] Set environment variables in all spawned panes:
  - `TERM_PROGRAM=Crux`, `TERM_PROGRAM_VERSION=x.y.z`
  - `CRUX_PANE_ID=<pane-id>` (per-pane, auto-set)
- [ ] Submit **Feature Request issue** to `anthropics/claude-code`:
  - Title: `[FEATURE] Add Crux as a split-pane backend for agent teams (teammateMode)`
  - Include tmux <-> Crux CLI mapping table
  - Include working CLI demo
  - Reference WezTerm #23574, Zellij #24122 issues
  - Propose generic terminal backend interface
- [ ] Submit **plugin PR** to `plugins/crux-terminal-backend/`:
  - hooks-based prototype (hooks/ directory with setup.sh, teammate-spawn.sh)
  - README with installation guide
  - `marketplace.json` metadata
- [ ] Community engagement:
  - Claude Developers Discord participation
  - Comment on WezTerm/Zellij issues proposing generic backend interface
  - Contact Anthropic DevRel if possible
- [ ] Test with actual Agent Teams workflow:
  - Team lead creates teammates via `crux cli split-pane`
  - Teammates run in split panes
  - Messages routed correctly
  - Panes cleaned up on shutdown

### 5.4 MCP Server — Differentiation Tools (crux-mcp Phase 2)

Advanced MCP tools leveraging Crux's unique capabilities (GPUI, IME, clipboard, DockArea). Built on the Phase 2.9 MCP server foundation.

- [ ] `crux_parse_output_structured` — parse terminal output into structured JSON
  - Table detection (borders, columns, headers) → JSON array
  - Tree view parsing → hierarchical object
  - Error message extraction → `{file, line, error, suggestion}`
  - Leverages GPUI rendering engine's visual structure awareness
- [ ] `crux_visual_diff` — screenshot before/after command, return diff
  - Pixel diff + semantic diff ("table gained 3 rows")
  - Useful for TUI app state verification
- [ ] `crux_type_with_ime` — Korean/Japanese/Chinese IME input simulation
  - Waits for composition commit (not just key send)
  - Guarantees preedit never reaches PTY
  - Only terminal MCP with CJK automation support
- [ ] `crux_clipboard_context` — clipboard history with source attribution
  - Track source pane, timestamp, content type (text/image/RTF/file)
  - `crux_paste_smart` — format-adaptive paste (JSON → pretty-print)
- [ ] `crux_load_workspace` — predefined multi-pane layouts for agent teams
  - Presets: `debug-session`, `full-stack`, `agent-team-3`, `monitoring`
  - Custom layouts via JSON schema
  - DockArea programmatic layout API
- [ ] `crux_stream_output` — real-time output streaming via SSE
  - Event types: `output`, `exit`, `error`
  - Server-side pattern filtering
  - Enables reactive agents (cancel on first error)
- [ ] `crux_coordinate_panes` — multi-pane orchestrated execution
  - Declarative steps: `[{pane, command, wait_for}, ...]`
  - Sequential conditional execution in single tool call
  - Killer feature for Agent Teams service startup ordering
- [ ] `crux_inject_context` — dynamic shell env injection without restart
  - Temporary env vars with auto-expiry (N commands)
  - Alias/function injection
- [ ] `crux_create_snapshot` / `crux_restore_snapshot` — full terminal state serialization
  - All panes, processes, scrollback, environment
  - Reproducible debugging sessions
- [ ] `crux_detect_intent` — command + output → intent classification
  - Categories: `build`, `test`, `error`, `wait_input`, `success`
  - Suggested next actions based on state

### 5.5 MCP Server — Testing Tools (crux-mcp Phase 3)

7 testing-specific MCP tools enabling AI agents (Claude Code) to autonomously verify Crux's correctness. See [research/testing/ai-agent-testing.md](research/testing/ai-agent-testing.md) for full design.

- [ ] `crux_inspect_cell` — single cell inspection (char, fg/bg RGB, flags)
  - Row/col addressing (0-based, viewport-relative)
  - Returns: char, width, fg/bg as `[r,g,b]`, bold/italic/underline/strikethrough flags
- [ ] `crux_dump_grid` — full grid snapshot as structured JSON
  - Optional region parameter for partial dumps
  - Returns: 2D cell array, cursor position, scroll region, dimensions
- [ ] `crux_get_terminal_modes` — terminal state machine query
  - DEC modes: DECCKM, DECNKM, bracketed paste, mouse mode, origin, autowrap
  - Charset: G0/G1 designation, active set
  - Cursor style, window title, icon name
- [ ] `crux_get_performance` — runtime performance metrics
  - FPS (60-frame rolling average), frame time, input latency
  - Scroll throughput (lines/sec), cell render time (μs)
  - Memory usage
- [ ] `crux_get_accessibility` — accessibility tree snapshot
  - Role hierarchy, labels, values, plain text content per pane
- [ ] `crux_subscribe_events` — event stream subscription
  - Event types: input, output, resize, mode_change
  - Timestamped events for replay testing
- [ ] `crux_visual_hash` — perceptual hash of rendered output
  - pHash for visual regression detection
  - Returns hash + screenshot path + viewport metadata
- [ ] Golden state test infrastructure:
  - [ ] `tests/golden/*.json` — expected grid states for VT sequences
  - [ ] `crux --test-mode` — hidden window + MCP server for CI
  - [ ] `crux --headless` — VT logic only, no GPU rendering
  - [ ] `scripts/run-tests.sh` — launch/connect/test/teardown harness

### 5.6 MCP Security Configuration

- [ ] Command whitelist/blocklist in `config.toml`:
  ```toml
  [mcp.security]
  allowed_commands = ["ls", "cat", "git *", "cargo *", "npm *"]
  dangerous_patterns = ["rm", "sudo", "chmod", "kill"]
  command_timeout_ms = 30000
  max_panes = 20
  ```
- [ ] Dangerous command confirmation prompt (GPUI dialog)
- [ ] ANSI escape sanitization on output returned via MCP
- [ ] Rate limiting for MCP tool calls

### 5.7 Configuration System

#### Config File & Parsing
- [ ] TOML configuration file (`~/.config/crux/config.toml`)
- [ ] XDG-first config discovery with macOS native fallback (`~/Library/Application Support/com.crux.terminal/`)
- [ ] `deny_unknown_fields` — typo detection at parse time
- [ ] Layered config merging: CLI flags > env vars (`CRUX_*`) > config file > built-in defaults
- [ ] `crux --generate-config` — emit annotated default config
- [ ] `crux --check-config` — validate config without launching
- [ ] Deprecated field migration with versioned warnings

#### Configurable Settings
- [ ] Font family, size, line height, ligatures
- [ ] CJK font fallback chain (`[font.fallback]`)
- [ ] Color scheme / theme (16 ANSI + 256 palette + fg/bg/cursor)
- [ ] Key bindings (customizable)
- [ ] Scrollback size
- [ ] Default shell and args
- [ ] IME settings (auto-switch enable/disable, preedit render mode)
- [ ] Option key behavior (`option_as_alt`: left/right/both/none)
- [ ] Ambiguous width preference
- [ ] Window opacity, blur, decorations
- [ ] MCP security policy (allowed tools, command whitelist/blocklist)
- [ ] OS theme mode (`theme_mode`: auto/light/dark — macOS appearance 연동)
- [ ] Bell settings: audible bell toggle, visual bell (flash) with duration/color
- [ ] Session restore on launch (window/tab/pane state persistence)
- [ ] Tab display settings (indicator toggle, title format)
- [ ] Global hotkey / Quake mode (system-wide toggle key, position, size)
- [ ] Integrated GPU preference (`prefer_integrated_gpu` for battery saving)
- [ ] Long-running command notifications (threshold, macOS notification center)
- [ ] Autosuggestion toggle (fish-style history-based, Phase 2 dependency)

#### Hot Reload
- [ ] File system watcher via `notify` crate (watch parent directory for atomic saves)
- [ ] 10ms debounce to prevent rapid successive reloads
- [ ] On parse error: keep old config, show user-facing notification
- [ ] Diff-based application — only re-render changed properties

### 5.8 GUI Settings Window (⌘,)

- [ ] Native settings window built with gpui-component widgets
- [ ] Bidirectional sync: GUI edits → write TOML, TOML edits → update GUI
- [ ] Tab-based layout:
  - [ ] **General** — shell, startup behavior, working directory, session restore, notifications
  - [ ] **Appearance** — font family/size picker, color picker, theme selector (+ OS sync), opacity slider, blur toggle
  - [ ] **Terminal** — scrollback, cursor style/blink, mouse mode, bell (audible/visual), global hotkey
  - [ ] **Keybindings** — visual key recorder, conflict detection, searchable list
  - [ ] **IME** — input source, Vim auto-switch toggle, composition overlay style
  - [ ] **MCP** — security policy, allowed tools, socket path
  - [ ] **Performance** — integrated GPU preference, rendering diagnostics
- [ ] Live preview — changes apply in real-time as user adjusts settings
- [ ] No "Apply" button — direct write to config.toml on every change
- [ ] "Open Config File" button — open TOML in user's editor
- [ ] "Reset to Default" per section

### 5.9 Polish

- [ ] Application icon and window chrome
- [ ] macOS menu bar integration
- [ ] About dialog with version info

### Milestone 5 Deliverable

- tmux works perfectly inside Crux
- Claude Code Feature Request submitted with working demo
- Crux CLI matches PaneBackend interface for seamless integration
- **10 differentiation MCP tools leverage GPUI, IME, clipboard uniquely**
- **7 testing MCP tools enable AI agent self-testing (37 total MCP tools)**
- **MCP security configuration with command whitelist/blocklist**
- **`crux_coordinate_panes` enables declarative multi-service orchestration**
- Golden state test infrastructure with `--test-mode` and `--headless` flags
- Full configuration system with TOML config + GUI settings window (⌘,) + hot reload
- Production-ready terminal emulator

---

## Phase 6: Distribution & Community

**Goal**: Homebrew distribution, code signing, and community growth.

### 6.1 GitHub Releases (Phase 1 -- $0)

- [ ] Universal Binary build (arm64 + x86_64):
  ```bash
  rustup target add aarch64-apple-darwin x86_64-apple-darwin
  MACOSX_DEPLOYMENT_TARGET="13.0" cargo build --release --target=aarch64-apple-darwin
  MACOSX_DEPLOYMENT_TARGET="13.0" cargo build --release --target=x86_64-apple-darwin
  lipo -create target/aarch64-apple-darwin/release/crux target/x86_64-apple-darwin/release/crux -output target/release/crux
  ```
- [ ] `.app` bundle creation (Makefile or `cargo-bundle`)
  - `Crux.app/Contents/MacOS/crux-app` (binary)
  - `Crux.app/Contents/Resources/crux.icns` (icon)
  - `Crux.app/Contents/Resources/terminfo/` (compiled terminfo)
  - `Crux.app/Contents/Info.plist`
- [ ] GitHub Actions CI workflow:
  - Check & lint (clippy, rustfmt) on push/PR
  - Build Universal Binary
  - Run tests
  - Cache: `Swatinem/rust-cache@v2` + sccache (up to 80% build time reduction)
- [ ] GitHub Actions Release workflow (triggered by `v*` tags):
  - Build Universal Binary
  - Create `.app` bundle
  - Create DMG
  - Generate changelog (git-cliff)
  - Create GitHub Release with assets + SHA256 checksums
- [ ] Versioning: SemVer (`0.x.y` during initial development)
- [ ] Conventional Commits for changelog automation

### 6.2 Custom Homebrew Tap (Phase 1 -- $0)

- [ ] Create `crux-terminal/homebrew-crux` repository
- [ ] Write Formula (source build, no code signing needed):
  ```ruby
  class Crux < Formula
    desc "GPU-accelerated terminal emulator built with Rust and GPUI"
    homepage "https://github.com/crux-terminal/crux"
    url "https://github.com/crux-terminal/crux/archive/refs/tags/v0.1.0.tar.gz"
    sha256 "..."
    license "MIT"
    depends_on "rust" => :build
    depends_on :macos
    def install
      system "cargo", "install", *std_cargo_args
      system "tic", "-x", "-o", "#{share}/terminfo", "extra/crux.terminfo"
    end
    test do
      assert_match version.to_s, shell_output("#{bin}/crux --version")
    end
  end
  ```
- [ ] User install: `brew tap crux-terminal/crux && brew install crux`
- [ ] Tap auto-update workflow (GitHub Actions in homebrew-crux repo)

### 6.3 Code Signing & Notarization (Phase 2 -- $99/year)

- [ ] Apple Developer Program enrollment ($99/year)
- [ ] "Developer ID Application" certificate
- [ ] `codesign --force --deep --options runtime --sign "Developer ID Application: ..." --timestamp Crux.app`
- [ ] Notarization via `xcrun notarytool submit` (API key method for CI)
- [ ] `xcrun stapler staple` for offline verification
- [ ] Add signed DMG to Tap as Cask:
  ```ruby
  cask "crux" do
    version "0.1.0"
    url "https://github.com/crux-terminal/crux/releases/download/v#{version}/Crux-v#{version}.dmg"
    app "Crux.app"
    binary "#{appdir}/Crux.app/Contents/MacOS/crux"
  end
  ```
- [ ] CI secrets: `APPLE_CERTIFICATE_BASE64`, `APPLE_SIGNING_IDENTITY`, `APPLE_API_KEY`, etc.

### 6.4 homebrew-core Formula Submission (Phase 3 -- requires 75+ stars)

- [ ] Checklist before submission:
  - [ ] GitHub Stars >= 75 (or Forks >= 30, or Watchers >= 30)
  - [ ] External user PRs/issues exist (not just author)
  - [ ] Stable tagged release
  - [ ] `brew audit --strict --new --online crux` passes
  - [ ] `brew test crux` passes
  - [ ] Builds on latest 3 macOS versions (Apple Silicon + x86_64)
  - [ ] DFSG-compatible license (MIT)
  - [ ] No auto-update functionality
- [ ] Fork `homebrew-core`, create Formula, submit PR
- [ ] BrewTestBot automated testing
- [ ] Maintainer review (1-4 weeks typical)

### 6.5 homebrew-cask Submission (Phase 4 -- mature)

- [ ] Prerequisites: code signing + notarization (Gatekeeper pass required)
- [ ] Fork `homebrew-cask`, create Cask, submit PR
- [ ] **Lesson from Alacritty**: Alacritty Cask was deprecated Oct 2025 due to Gatekeeper failure (ad-hoc signing). Always use proper Developer ID signing.

### Milestone 6 Deliverable

- `brew tap crux-terminal/crux && brew install crux` works
- Signed and notarized `.app` bundle available
- Automated release pipeline (tag -> build -> sign -> notarize -> release -> tap update)
- Path to homebrew-core inclusion

---

## Key Architectural Decisions

### 1. VT Parser: `alacritty_terminal`
- Complete terminal implementation (grid, events, search, selection, damage tracking)
- Battle-tested in Alacritty -- most widely used GPU terminal
- Damage tracking aligns perfectly with GPUI's frame-based rendering
- **Future**: Monitor `libghostty-vt` for potential migration (SIMD, Kitty Graphics built-in)

### 2. PTY: `portable-pty` -> Custom
- Start with `portable-pty` for rapid prototyping (well-tested WezTerm component)
- Migrate to direct `openpty(3)` + `fork` if finer control is needed

### 3. IME: Direct `NSTextInputClient` via `objc2`
- GPUI already implements `NSTextInputClient` at the platform layer
- Crux extends this with terminal-specific overlay rendering
- Preedit text is **never** sent to PTY -- only committed text goes through
- Use `objc2` ecosystem (not deprecated `core-foundation-rs`)

### 4. IPC: Unix Domain Socket + JSON-RPC 2.0
- JSON for debuggability (`jq` friendly)
- Length-prefix framing for efficient parsing
- Peer credential verification for security
- Extensible namespace system (`crux:<domain>/<action>`)

### 5. UI: GPUI + gpui-component
- DockArea for IDE-style panel management
- Tabs for terminal session management
- Resizable panels for split panes
- Pin to specific GPUI version to avoid breaking changes

### 6. Triple Protocol Strategy
- **IPC channel** (Unix socket, `crux:<domain>/<action>`): Low-level terminal control (CLI, internal components)
- **MCP channel** (Unix socket + stdio bridge, MCP tools): AI-agent-friendly high-level tools (Claude Desktop, Cursor, Claude Code)
- **In-band channel** (escape sequences): PTY application communication (OSC, DCS, APC)
- MCP is a wrapper over IPC: `AI Agent → MCP → IPC → Terminal`
- Custom OSC 7700-7799 for Crux-specific in-band extensions

### 7. Terminfo Strategy: `xterm-crux`
- `xterm-` prefix for maximum app compatibility (Ghostty/Kitty/Rio validated pattern)
- Fragment pattern: `crux+common` shared base, `xterm-crux` (256-color default), `crux-direct` (true color variant)
- Fully independent definition (not inheriting from `xterm-256color`)
- Fallback: `TERM=xterm-256color` when crux terminfo unavailable (SSH to remote hosts)

### 8. Key Mapping: xterm Standard + Kitty Protocol
- Phase 1: xterm standard encoding (CSI/SS3 sequences, modifier params)
- Phase 4: Kitty keyboard protocol as progressive enhancement (opt-in via `CSI > flags u`)
- macOS Option key: configurable left/right/both/none (default: left=Meta)

### 9. Distribution: Formula First, Cask Later
- Start with source-build Formula in custom Tap (no code signing needed, $0)
- Add signed Cask after Apple Developer Program enrollment ($99/year)
- Submit to homebrew-core when popularity thresholds met (75+ stars)
- **Lesson**: Alacritty Cask deprecated due to missing code signing -- avoid this path

### 10. Claude Code: Feature Request, Not Direct PR
- Claude Code is proprietary (bundled `cli.js`, "All rights reserved")
- External contributions limited to `plugins/`, `docs/`, `examples/` directories
- Core terminal backend code changes require Anthropic internal implementation
- Strategy: Build perfect CLI interface, submit Feature Request with working demo, engage community

---

## Risk Mitigation

| Risk | Mitigation |
|------|------------|
| GPUI breaking changes | Pin to `gpui = "0.2.2"` on crates.io, upgrade periodically with test suite |
| GPUI documentation gaps | Reference Zed source code, gpui-ghostty, gpui-terminal projects |
| alacritty_terminal API changes | Pin version, adapter layer isolates VT backend |
| Korean IME edge cases | Comprehensive test matrix (2-set, 3-set, Gureum, macOS native) |
| tmux Control Mode complexity | Implement basic tmux compat first, Control Mode as separate long-term effort |
| **Claude Code not open-source** | Feature Request approach + plugin prototype + community engagement. Cannot submit core backend PR directly. Build compelling CLI demo to incentivize Anthropic to add support internally |
| **Claude Code API instability** | PaneBackend interface is internal, may change without notice. Track `anthropics/claude-code` releases, maintain adapter layer |
| **Homebrew 75+ stars requirement** | Start with custom Tap (no popularity requirement). Build community via Claude Code integration story, dev tool marketing |
| **Apple Developer Program cost** | $99/year for notarization. Start without it (Formula source build). Defer Cask until project has traction |
| **Alacritty deprecation lesson** | Never ship unsigned binaries as Cask. Use Formula (source build) by default, add signed Cask only after proper Developer ID signing |
| `core-foundation` dependency conflict | GPUI crates.io build may hit `core-foundation 0.10.1` conflict. Fix: `cargo update` or `[patch.crates-io]` override |
| Xcode requirement for GPUI | GPUI needs full Xcode.app (not just CLT) for Metal shader compilation. Document in README, verify in CI |
| **rmcp Edition 2024 (nightly)** | `rmcp` v0.15 requires Rust Edition 2024 (nightly). Monitor stable promotion timeline. Fallback: use `rust-mcp-sdk` crate or pin older rmcp version |
| **MCP protocol evolution** | MCP spec is still evolving (2025-11-25 current). Pin to spec version, maintain adapter layer for protocol changes |
| **GPUI main thread constraint** | MCP server needs separate Tokio thread since GPUI owns main thread (macOS requirement). Use `mpsc`/`oneshot` channels, test for deadlocks |
| **MCP security surface** | MCP tools expose arbitrary command execution. Implement command whitelist, dangerous command confirmation, ANSI sanitization, rate limiting |

---

## References

### Core Research Documents
- [GPUI Framework Research](research/gpui-research.md)
- [Terminal Core Research](research/terminal-core-research.md)
- [IME & Clipboard Research](research/ime-clipboard-research.md)
- [IPC & Agent Teams Research](research/ipc-agent-teams-research.md)

### Architecture & Implementation References
- [GPUI Terminal Implementations Analysis](research/gpui-terminal-implementations.md)
- [Rust Terminal Architecture Patterns](research/rust-terminal-architecture.md)
- [IPC & Claude Code Integration Details](research/04-ipc-claude-code-integration.md)

### Gap Analysis & New Research
- [Gap Analysis Report (30+ gaps identified)](research/GAP-ANALYSIS.md)
- [Claude Code PR Research (NOT open source)](research/claude-code-pr-research.md)
- [Homebrew Distribution Pipeline](research/homebrew-distribution-pipeline.md)
- [Terminfo Entry Research](research/terminfo-research.md)
- [Key Mapping & Escape Sequences](research/keymapping-research.md)
- [GPUI Project Bootstrap](research/gpui-bootstrap.md)

### MCP Integration & Testing Research
- [MCP Integration Strategy](research/integration/mcp-integration.md) -- Protocol, SDK, 30 tools design, architecture
- [AI Agent Testing Infrastructure](research/testing/ai-agent-testing.md) -- 7 testing MCP tools, self-testing, golden state, CI/CD
- [Model Context Protocol Specification](https://modelcontextprotocol.io/specification/2025-11-25) -- Official spec
- [rmcp (Official Rust MCP SDK)](https://github.com/modelcontextprotocol/rust-sdk) -- v0.15.0
- [terminal-mcp](https://github.com/elleryfamilia/terminal-mcp) -- Reference terminal MCP implementation
- [conductor-mcp](https://github.com/GGPrompts/conductor-mcp) -- 33-tool Claude Code orchestration reference

### External References
- [Zed Source Code](https://github.com/zed-industries/zed) -- GPUI reference implementation
- [gpui-ghostty](https://github.com/Xuanwo/gpui-ghostty) -- GPUI + terminal integration reference
- [WezTerm CLI](https://wezterm.org/cli/cli/index.html) -- CLI interface reference
- [Claude Code Agent Teams](https://github.com/anthropics/claude-code/issues/23574) -- WezTerm integration request (reference)
- [Ghostty Terminfo](https://ghostty.org/docs/help/terminfo) -- xterm-ghostty TERM strategy reference
- [Kitty Keyboard Protocol](https://sw.kovidgoyal.net/kitty/keyboard-protocol/) -- Progressive enhancement spec
- [xterm Control Sequences](https://invisible-island.net/xterm/ctlseqs/ctlseqs.html) -- Canonical key encoding reference
- [Alacritty Cask Deprecation](https://github.com/alacritty/alacritty/issues/8749) -- Code signing cautionary tale
