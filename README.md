# Crux

> **crux** (n.) — the essential point; the Southern Cross constellation

A GPU-accelerated terminal emulator for macOS, built with Rust and Metal. The first terminal with a **native MCP server** — any AI agent can control Crux out of the box. Designed for the AI coding era with first-class CJK/Korean IME support, programmatic pane control, and rich clipboard input.

---

## Why Crux?

No existing terminal satisfies all three requirements simultaneously:

| Requirement | Warp | Ghostty | WezTerm | iTerm2 | **Crux** |
|-------------|------|---------|---------|--------|----------|
| **Native MCP server** (AI agent control) | X | X | X | X | **O** |
| Modern UX (tabs, splits, MD preview) | O | △ | △ | O | **O** |
| Programmatic split-pane CLI/API | X | X | O | △ | **O** |
| tmux compatibility | X | O | O | O | **O** |
| First-class Korean/CJK IME | △ | △ | △ | △ | **O** |
| Binary clipboard input (images) | X | X | X | △ | **O** |
| GPU-accelerated rendering | O | O | O | X | **O** |

**Core problem**: AI coding tools like Claude Code Agent Teams need programmatic pane control (`split-pane`, `send-text`, `list`) to orchestrate multiple agent instances. Current solutions are either closed-source cloud platforms (Warp) or external MCP wrappers bolted onto terminals via AppleScript or tmux. **No terminal has a native MCP server built in.** Crux changes this — any MCP-compatible AI agent (Claude Desktop, Claude Code, Cursor, Windsurf) can control Crux directly with zero configuration.

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

### Native MCP Server (Key Differentiator)
- **Built-in MCP server** — no external wrapper, no AppleScript overhead
- **30 MCP tools** across pane management, command execution, state inspection, content capture
- **Any AI agent works instantly** — Claude Desktop, Claude Code, Cursor, Windsurf, Copilot
- **`crux_coordinate_panes`** — declarative multi-service orchestration in a single tool call
- **`crux_screenshot_pane`** — GPU-rendered visual capture for AI vision
- **`crux_wait_for_output`** — pattern-matching output monitor with timeout
- **`crux_type_with_ime`** — CJK input simulation (only MCP terminal with IME support)
- **`crux_load_workspace`** — predefined multi-pane layouts for agent teams
- **stdio bridge** (`crux-mcp-bridge`) for Claude Desktop compatibility

### Crux Protocol (IPC)
- **Unix domain socket** + JSON-RPC 2.0 protocol
- **CLI client** (`crux cli split-pane`, `send-text`, `get-text`, `list`)
- **Claude Code Agent Teams** native backend support
- **Event subscription** — pane lifecycle, output, focus changes
- **Triple protocol** — MCP for AI agents + IPC for CLI/programmatic control + in-band escape sequences for PTY apps
- **Custom OSC 7700-7799** namespace for Crux-specific extensions

### tmux Compatibility
- Full VT100 feature set required by tmux
- True color passthrough (`Tc` / `RGB` terminfo flags)
- SGR mouse mode, bracketed paste, focus events
- **tmux Control Mode** (`-CC`) integration (long-term goal)

---

## Architecture

```mermaid
flowchart TD
  subgraph crux["Crux App — GPUI Main Thread"]
    direction TB

    subgraph ui["DockArea (gpui-component)"]
      TabPanel["TabPanel — tabs"]
      Split["Split — h/v layouts"]
    end

    subgraph view["CruxTerminalView — GPUI Element"]
      CellRender["Cell rendering"]
      CursorR["Cursor rendering"]
      SelectionR["Selection rendering"]
      IMEOverlay["IME overlay"]
    end

    subgraph entity["CruxTerminal — Entity"]
      VT["alacritty_terminal\nVT parser"]
      PTY["portable-pty\nPTY management"]
      EventQueue["Event queue\nbatched, max 100/4ms"]
    end

    subgraph servers["Server Layer — Tokio Threads"]
      IPC["IPC Server\nJSON-RPC 2.0\ncrux:pane/* clipboard/* ime/*"]
      MCP["MCP Server\nrmcp + Axum\n30 tools + resources"]
    end

    ui --> view --> entity
    entity <--> servers
  end

  CLI["crux cli"] <-- "IPC\nUnix socket" --> IPC
  Bridge["crux-mcp-bridge\nstdio ↔ socket"] <-- "Unix socket" --> MCP
  DirectMCP["Direct MCP clients"] <-- "Unix socket\n~/.crux/mcp.sock" --> MCP

  Shell["Shell scripts"] <--> CLI
  ClaudeDesktop["Claude Desktop\nClaude Code"] <-- stdio --> Bridge
  Cursor["Cursor / Windsurf"] <--> DirectMCP

  style MCP fill:#4a9eff,color:#fff
  style IPC fill:#50c878,color:#fff
  style crux fill:#1a1a2e,color:#fff
  style ui fill:#16213e,color:#fff
  style view fill:#16213e,color:#fff
  style entity fill:#16213e,color:#fff
  style servers fill:#0f3460,color:#fff
```

### Triple Protocol

```mermaid
flowchart LR
  subgraph agents["AI Agents"]
    CD[Claude Desktop]
    CC[Claude Code]
    CR[Cursor]
  end

  subgraph cli["CLI / Scripts"]
    CL[crux cli]
    SH[Shell scripts]
  end

  subgraph pty["PTY Applications"]
    VIM[vim / nvim]
    HTOP[htop / btm]
    APP[Custom apps]
  end

  subgraph crux["Crux Terminal"]
    MCP_S[MCP Server\n30 tools]
    IPC_S[IPC Server\nJSON-RPC 2.0]
    INB[In-band Parser\nOSC · DCS · APC]
  end

  CD -- "MCP\n(stdio bridge)" --> MCP_S
  CC -- "MCP\n(Unix socket)" --> MCP_S
  CR -- "MCP\n(Unix socket)" --> MCP_S

  CL -- "IPC\n(Unix socket)" --> IPC_S
  SH -- "IPC\n(Unix socket)" --> IPC_S

  VIM -- "Escape sequences\n(PTY stream)" --> INB
  HTOP -- "Escape sequences\n(PTY stream)" --> INB
  APP -- "OSC 7700-7799\n(Crux extensions)" --> INB

  style MCP_S fill:#4a9eff,color:#fff
  style IPC_S fill:#50c878,color:#fff
  style INB fill:#ff9f43,color:#fff
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
| MCP SDK | `rmcp` | 0.15.0 | Native MCP server (Model Context Protocol) |
| HTTP Server | `axum` | 0.8 | MCP Unix socket / HTTP transport |
| Async Runtime | `tokio` | latest | Unix socket IPC server, MCP server, async I/O |
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
| `CRUX_MCP_SOCKET` | MCP server Unix socket path (default: `~/.crux/mcp.sock`) |
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

## MCP Integration

Crux is the first terminal emulator with a **native MCP (Model Context Protocol) server**. Any MCP-compatible AI tool can control Crux without external wrappers.

### Claude Desktop Setup

Add to `~/Library/Application Support/Claude/claude_desktop_config.json`:

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

### Available MCP Tools (30)

| Category | Tools | Examples |
|----------|-------|---------|
| **Pane Management** (5) | create, close, focus, list, resize | `crux_create_pane`, `crux_list_panes` |
| **Command Execution** (5) | execute, send_keys, send_text, get_output, wait_for_output | `crux_execute_command`, `crux_wait_for_output` |
| **State Inspection** (5) | cwd, process, pane_state, selection, scrollback | `crux_get_current_directory` |
| **Content Capture** (5) | screenshot, raw_text, formatted, save/restore session | `crux_screenshot_pane` |
| **Differentiation** (10) | structured output, visual diff, IME input, clipboard, workspace layouts, streaming, pane coordination, context injection, snapshots, intent detection | `crux_coordinate_panes`, `crux_type_with_ime` |

### Example: Multi-Service Orchestration

An AI agent can start a full-stack environment with a single MCP tool call:

```json
{
  "tool": "crux_coordinate_panes",
  "input": {
    "steps": [
      { "pane": "backend",  "command": "cargo run",     "wait_for": "Listening on 0.0.0.0:8080" },
      { "pane": "frontend", "command": "npm run dev",    "wait_for": "ready in" },
      { "pane": "test",     "command": "cargo test --test e2e" }
    ]
  }
}
```

---

## Crux Protocol

Hierarchical namespace: `crux:<domain>/<action>`

| Protocol | Domain | Methods | Priority |
|----------|--------|---------|----------|
| **IPC** | `crux:pane/*` | split, send-text, get-text, list, activate, close, resize, move | P0 |
| **IPC** | `crux:window/*` | create, list, close | P0 |
| **IPC** | `crux:clipboard/*` | read, write (text, HTML, images) | P1 |
| **IPC** | `crux:ime/*` | get-state, set-input-source | P1 |
| **IPC** | `crux:render/*` | image, markdown | P2 |
| **IPC** | `crux:events/*` | subscribe, unsubscribe | P1 |
| **MCP** | `crux_*` | 30 tools (pane, execute, state, content, differentiation) | P0 |

See the [Triple Protocol diagram](#triple-protocol) above for the full architecture.

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
│   ├── crux-app/           # Main application, window management
│   ├── crux-terminal/      # Terminal entity, VT integration, PTY
│   ├── crux-terminal-view/ # GPUI rendering, IME overlay, selection
│   ├── crux-ipc/           # Unix socket server, JSON-RPC, CLI client
│   ├── crux-mcp/           # Native MCP server (30 tools, rmcp SDK)
│   ├── crux-mcp-bridge/    # stdio ↔ Unix socket bridge for Claude Desktop
│   ├── crux-clipboard/     # Rich clipboard, drag & drop
│   └── crux-protocol/      # Protocol types, Crux Protocol definitions
├── research/               # Technical research documents
├── README.md
├── PLAN.md
└── Cargo.toml
```

### Crate Dependency Graph

```mermaid
flowchart BT
  protocol["crux-protocol\nshared types"]
  terminal["crux-terminal\nVT emulation"]
  view["crux-terminal-view\nGPUI Element"]
  app["crux-app\nwindow, DockArea"]
  ipc["crux-ipc\nUnix socket, JSON-RPC"]
  mcp["crux-mcp\nMCP server, 30 tools"]
  bridge["crux-mcp-bridge\nstdio bridge"]
  clipboard["crux-clipboard\nNSPasteboard"]

  protocol --> terminal --> view --> app
  protocol --> ipc --> app
  protocol --> clipboard --> app
  protocol --> mcp
  ipc --> mcp
  mcp --> bridge

  style app fill:#e74c3c,color:#fff
  style mcp fill:#4a9eff,color:#fff
  style bridge fill:#4a9eff,color:#fff
  style protocol fill:#95a5a6,color:#fff
```

---

## Research

Detailed technical research is available in the `research/` directory:

- [GPUI Framework Research](research/gpui-research.md) — rendering pipeline, components, IME support
- [Terminal Core Research](research/terminal-core-research.md) — VT parsers, PTY, graphics protocols, tmux, Unicode
- [IME & Clipboard Research](research/ime-clipboard-research.md) — NSTextInputClient, Hangul composition, failure analysis
- [IPC & Agent Teams Research](research/ipc-agent-teams-research.md) — WezTerm CLI, Claude Code integration, Crux Protocol
- [MCP Integration Strategy](research/integration/mcp-integration.md) — Protocol, Rust SDK, 30 tools design, architecture

---

## License

TBD

---

## Name

**Crux** — Latin for "the essential point" and the name of the Southern Cross constellation. It represents both the core problem this terminal solves (the crux of terminal UX for AI coding) and navigational guidance (the Southern Cross has guided travelers for millennia).

The name is also designed to become a protocol namespace: `crux:<domain>/<action>`.
