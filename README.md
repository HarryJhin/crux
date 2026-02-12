<p align="center">
  <img src="extra/crux-logo.svg" width="160" alt="Crux">
</p>

<h1 align="center">Crux</h1>

<p align="center">
  GPU-accelerated terminal emulator for macOS, built with Rust and Metal.<br>
  Designed for the AI coding era — with a native MCP server and first-class Korean/CJK IME.
</p>

<p align="center">
  <a href="README.md"><strong>English</strong></a> · <a href="README.ko.md">한국어</a>
</p>

<p align="center">
  <img src="https://img.shields.io/badge/status-early%20development-orange" alt="Status: Early Development">
  <img src="https://img.shields.io/badge/platform-macOS%2013%2B-blue" alt="macOS 13+">
  <img src="https://img.shields.io/badge/license-MIT%20%2F%20Apache--2.0-green" alt="License: MIT / Apache-2.0">
</p>

> **Early Development (Phase 1 of 6)**
> Core terminal rendering works. Most features below are on the roadmap.
> See [PLAN.md](PLAN.md) for the full implementation plan.

<!-- TODO: Add hero screenshot once visual polish is sufficient -->

---

## Why Crux?

AI coding tools like Claude Code need **programmatic pane control** — split terminals, send commands, read output. Today this means bolting AppleScript wrappers onto terminals or routing everything through tmux. No terminal has this built in natively.

Meanwhile, **every terminal on macOS has Korean input bugs.** Alacritty double-types spaces during Hangul composition. Ghostty destroys preedit text on modifier key presses. iTerm2 mispositions the candidate window. These aren't edge cases — they affect millions of CJK users daily.

Crux is built to solve both problems from the ground up:

- **Native MCP server** — AI agents (Claude Desktop, Claude Code, Cursor) control Crux directly with zero configuration
- **First-class Korean/CJK IME** — not an afterthought, but a core design constraint from day one
- **Programmatic pane control** — a real API for split panes, command execution, and output monitoring
- **GPU-accelerated** — Metal rendering via [GPUI](https://gpui.rs), targeting 120 FPS

---

## What Works Today

- Metal GPU-rendered terminal window via GPUI
- Full VT100/xterm emulation powered by [alacritty_terminal](https://github.com/alacritty/alacritty)
- True color (24-bit RGB) + 256 color support
- Keyboard input with modifier key handling
- SGR mouse reporting and bracketed paste
- Custom terminfo entry (`xterm-crux`) with `xterm-256color` fallback

---

## Roadmap

| Phase | Focus | Status |
|-------|-------|--------|
| **1. Basic Terminal** | Shell rendering, keyboard, VT emulation, terminfo | **In Progress** |
| **2. Tabs & Panes** | Split panes, IPC server, CLI client, shell integration | Planned |
| **3. Korean/CJK IME** | NSTextInputClient, Hangul composition, candidate window | Planned |
| **4. Rich Features** | Markdown preview, clickable links, graphics protocols | Planned |
| **5. AI Integration** | Native MCP server (30 tools), tmux compat, config system | Planned |
| **6. Distribution** | Homebrew, code signing, notarization, Universal Binary | Planned |

See [PLAN.md](PLAN.md) for the detailed checklist with 200+ items.

---

## Building from Source

**Prerequisites**: macOS 13+ (Ventura), Rust stable toolchain, full **Xcode.app** install (Command Line Tools alone won't work — Metal shader compilation requires the full IDE).

```bash
# Verify Metal compiler
xcrun -sdk macosx metal --version

# Clone, build, and run
git clone https://github.com/HarryJhin/crux.git
cd crux
cargo run -p crux-app
```

Optionally, compile the terminfo entry for full capability negotiation:

```bash
tic -x -e xterm-crux,crux,crux-direct extra/crux.terminfo
```

---

## Project Structure

Cargo workspace with 7 crates:

```
crux-terminal       VT emulation (alacritty_terminal + portable-pty)
crux-terminal-view  GPU rendering (GPUI canvas, cells, cursor, selection)
crux-app            Application shell (window management, GPUI bootstrap)
crux-protocol       Shared types and protocol definitions          [stub]
crux-ipc            Unix socket server, JSON-RPC 2.0               [stub]
crux-clipboard      Rich clipboard and drag-and-drop               [stub]
crux-mcp            Native MCP server                           [planned]
```

See the [research/](research/) directory for architecture decisions and technical deep-dives.

---

## Contributing

Contributions are welcome — the project is in its early stages, which is a great time to get involved.

- **Language**: Rust (stable toolchain)
- **Style**: `cargo fmt` + `cargo clippy -- -D warnings`
- **Commits**: [Conventional Commits](https://www.conventionalcommits.org/) with crate-name scopes (e.g. `feat(terminal): add sixel support`)
- **Tests**: Required for new functionality

---

## License

Dual-licensed under [MIT](LICENSE-MIT) and [Apache 2.0](LICENSE-APACHE). Choose whichever you prefer.

---

<p align="center">
  <strong>Crux</strong> — Latin for "the essential point" and the Southern Cross constellation.<br>
  The crux of terminal UX for AI coding.
</p>
