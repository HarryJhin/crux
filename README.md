# Crux

> **crux** (n.) — the essential point; the Southern Cross constellation

A GPU-accelerated terminal emulator for macOS, built with Rust and Metal. Designed for the AI coding era with first-class CJK/Korean IME support, programmatic pane control, and rich clipboard input.

---

## Why Crux?

No existing terminal satisfies all three requirements simultaneously:

| Requirement | Warp | Ghostty | WezTerm | iTerm2 | **Crux** |
|-------------|------|---------|---------|--------|----------|
| Modern UX (tabs, splits, MD preview) | O | △ | △ | O | **O** |
| Programmatic split-pane CLI/API | X | X | O | △ | **O** |
| tmux compatibility | X | O | O | O | **O** |
| First-class Korean/CJK IME | △ | △ | △ | △ | **O** |
| Binary clipboard input (images) | X | X | X | △ | **O** |
| GPU-accelerated rendering | O | O | O | X | **O** |

**Core problem**: AI coding tools like Claude Code Agent Teams need programmatic pane control (`split-pane`, `send-text`, `list`) to orchestrate multiple agent instances. Currently only tmux and WezTerm provide this, but neither offers a polished UX with proper CJK input handling.

---

## Features

### Terminal Core
- **Metal GPU rendering** via GPUI framework — targeting 120 FPS
- **Full VT100/xterm emulation** powered by `alacritty_terminal`
- **True color** (24-bit RGB) + 256 color support
- **SGR mouse events**, bracketed paste, focus events
- **Scrollback buffer** with regex search (default 10,000 lines)
- **Unicode grapheme clusters** (Mode 2027) — correct emoji and CJK rendering

### UI/UX
- **Tabs** with drag reordering
- **Split panes** — horizontal/vertical with resizable dividers
- **Markdown preview** — inline rendered markdown output
- **Clickable links** — URL detection + OSC 8 hyperlinks
- **Theme system** — dark/light modes with customizable color schemes

### Korean/CJK IME (Key Differentiator)
- **NSTextInputClient** full protocol implementation
- **Hangul composition overlay** — preedit text rendered separately from PTY buffer
- **Accurate candidate window positioning** via `firstRectForCharacterRange`
- **Modifier key isolation** during composition (prevents Ghostty-style preedit destruction)
- **IME/keyboard event deduplication** (prevents Alacritty-style double space)
- **Vim mode auto-switch** — detect cursor shape changes, auto-toggle IME to ASCII in Normal mode

### Rich Clipboard & Input
- **Binary clipboard paste** — images from NSPasteboard saved to temp files
- **Drag & drop** — files and images via NSDraggingDestination
- **Content type detection** — text, HTML, images, file URLs
- **Sideband channel** for rich input separate from PTY text stream

### Crux Protocol (IPC)
- **Unix domain socket** + JSON-RPC 2.0 protocol
- **CLI client** (`crux cli split-pane`, `send-text`, `get-text`, `list`)
- **Claude Code Agent Teams** native backend support
- **Event subscription** — pane lifecycle, output, focus changes
- **Dual protocol** — IPC for external control + in-band escape sequences for PTY apps
- **Custom OSC 7700-7799** namespace for Crux-specific extensions

### tmux Compatibility
- Full VT100 feature set required by tmux
- True color passthrough (`Tc` / `RGB` terminfo flags)
- SGR mouse mode, bracketed paste, focus events
- **tmux Control Mode** (`-CC`) integration (long-term goal)

---

## Architecture

```
┌──────────────────────────────────────────────────────────────┐
│                        Crux App (GPUI)                        │
│  ┌────────────────────────────────────────────────────────┐  │
│  │  DockArea (gpui-component)                             │  │
│  │  ├── TabPanel: tabs for multiple terminals             │  │
│  │  └── Split: horizontal/vertical resizable layouts      │  │
│  └──────────────────┬─────────────────────────────────────┘  │
│                     │                                        │
│  ┌──────────────────▼─────────────────────────────────────┐  │
│  │  CruxTerminalView (GPUI Element)                       │  │
│  │  ├── Cell rendering (BatchedTextRun)                   │  │
│  │  ├── Cursor rendering                                  │  │
│  │  ├── Selection rendering                               │  │
│  │  └── IME composition overlay (preedit)                 │  │
│  └──────────────────┬─────────────────────────────────────┘  │
│                     │                                        │
│  ┌──────────────────▼─────────────────────────────────────┐  │
│  │  CruxTerminal (Entity)                                 │  │
│  │  ├── VT parser: alacritty_terminal                     │  │
│  │  ├── PTY management: portable-pty                      │  │
│  │  ├── Event queue (batched, max 100/4ms)                │  │
│  │  └── TerminalContent (render snapshot + damage track)  │  │
│  └────────────────────────────────────────────────────────┘  │
│                                                              │
│  ┌────────────────────────────────────────────────────────┐  │
│  │  IPC Server (tokio Unix domain socket)                 │  │
│  │  ├── JSON-RPC 2.0 with length-prefix framing           │  │
│  │  ├── crux:pane/* — split, send-text, get-text, list    │  │
│  │  ├── crux:clipboard/* — rich clipboard operations      │  │
│  │  ├── crux:ime/* — IME state control                    │  │
│  │  └── crux:events/* — subscription & notifications      │  │
│  └────────────────────────────────────────────────────────┘  │
└──────────────────────────────────────────────────────────────┘
```

---

## Tech Stack

| Component | Crate / Technology | Version | Purpose |
|-----------|-------------------|---------|---------|
| UI Framework | `gpui` | 0.2.2 | Metal GPU rendering, Flexbox layout, event system |
| UI Components | `gpui-component` | 0.5.1 | DockArea, Tabs, Resizable panels, 60+ widgets |
| VT Parser | `alacritty_terminal` | 0.25.1 | Terminal emulation, grid, search, selection, damage tracking |
| PTY | `portable-pty` | 0.9.0 | PTY creation, resize, process management |
| macOS Bindings | `objc2` + `objc2-app-kit` | latest | NSTextInputClient, NSPasteboard, NSDragging |
| Async Runtime | `tokio` | latest | Unix socket IPC server, async I/O |
| Unicode | `unicode-width` + `unicode-segmentation` | latest | wcwidth, grapheme clusters (UAX #29) |
| Serialization | `serde` + `serde_json` | latest | JSON-RPC protocol |
| Image | `image` + `base64` | latest | Kitty Graphics Protocol, clipboard images |
| Text Shaping | CoreText (macOS native) | — | Font fallback, CJK glyph rendering |

---

## Environment Variables

| Variable | Description |
|----------|-------------|
| `TERM` | `xterm-256color` |
| `COLORTERM` | `truecolor` |
| `TERM_PROGRAM` | `Crux` |
| `TERM_PROGRAM_VERSION` | Current version |
| `CRUX_SOCKET` | IPC Unix socket path |
| `CRUX_PANE` | Current pane ID |
| `LANG` | Inherited (e.g., `ko_KR.UTF-8`) |

---

## CLI Usage

```bash
# Split pane and run a command
crux cli split-pane --right --percent 30 -- claude --teammate

# Send text to a pane
crux cli send-text --pane-id 42 --no-paste "ls -la\n"

# Read pane content
crux cli get-text --pane-id 42

# List all panes (JSON)
crux cli list --format json

# Activate a pane
crux cli activate-pane --pane-id 42
```

---

## Crux Protocol

Hierarchical namespace: `crux:<domain>/<action>`

| Domain | Methods | Priority |
|--------|---------|----------|
| `crux:pane/*` | split, send-text, get-text, list, activate, close, resize, move | P0 |
| `crux:window/*` | create, list, close | P0 |
| `crux:clipboard/*` | read, write (text, HTML, images) | P1 |
| `crux:ime/*` | get-state, set-input-source | P1 |
| `crux:render/*` | image, markdown | P2 |
| `crux:events/*` | subscribe, unsubscribe | P1 |

All communication uses JSON-RPC 2.0 over Unix domain sockets with 4-byte big-endian length-prefix framing.

---

## Building

```bash
# Prerequisites
rustup update stable
cargo install create-gpui-app  # optional scaffolding tool

# Build
cd crux
cargo build --release

# Run
cargo run --release
```

Requires macOS 13+ (Ventura) for Metal rendering.

---

## Project Structure

```
crux/
├── crates/
│   ├── crux-app/          # Main application, window management
│   ├── crux-terminal/     # Terminal entity, VT integration, PTY
│   ├── crux-terminal-view/ # GPUI rendering, IME overlay, selection
│   ├── crux-ipc/          # Unix socket server, JSON-RPC, CLI client
│   ├── crux-clipboard/    # Rich clipboard, drag & drop
│   └── crux-protocol/     # Protocol types, Crux Protocol definitions
├── research/              # Technical research documents
├── README.md
├── PLAN.md
└── Cargo.toml
```

---

## Research

Detailed technical research is available in the `research/` directory:

- [GPUI Framework Research](research/gpui-research.md) — rendering pipeline, components, IME support
- [Terminal Core Research](research/terminal-core-research.md) — VT parsers, PTY, graphics protocols, tmux, Unicode
- [IME & Clipboard Research](research/ime-clipboard-research.md) — NSTextInputClient, Hangul composition, failure analysis
- [IPC & Agent Teams Research](research/ipc-agent-teams-research.md) — WezTerm CLI, Claude Code integration, Crux Protocol

---

## License

TBD

---

## Name

**Crux** — Latin for "the essential point" and the name of the Southern Cross constellation. It represents both the core problem this terminal solves (the crux of terminal UX for AI coding) and navigational guidance (the Southern Cross has guided travelers for millennia).

The name is also designed to become a protocol namespace: `crux:<domain>/<action>`.
