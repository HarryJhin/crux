# Crux — AI Development Context

GPU-accelerated terminal emulator for macOS (Rust + Metal/GPUI). Key differentiators: Korean/CJK IME, Claude Code Agent Teams programmatic pane control, rich clipboard.

## Quick Reference

- **Language**: Rust (stable toolchain)
- **Platform**: macOS 13+ only (Metal GPU rendering)
- **License**: Dual MIT + Apache 2.0
- **Prerequisite**: Full Xcode.app install (not just CLT) — verify with `xcrun -sdk macosx metal --version`

## Commands

```bash
cargo build                         # Build all crates
cargo test --workspace              # Run all tests
cargo fmt --check                   # Check formatting
cargo clippy -- -D warnings         # Lint (treat warnings as errors)
cargo run -p crux-app               # Run the application
tic -x -e xterm-crux,crux,crux-direct extra/crux.terminfo  # Compile terminfo
```

## Workspace Structure

Cargo workspace with `resolver = "2"`. Crate dependency graph (leaf → root):

```
crux-protocol  (shared types, no internal deps)
    ↓
crux-terminal  (VT emulation: alacritty_terminal + portable-pty)
    ↓
crux-terminal-view  (GPUI Element: cell rendering, IME overlay, cursor)
    ↓
crux-app  (main: window management, GPUI bootstrap, DockArea)

crux-ipc        (Unix socket server, JSON-RPC 2.0 — depends on crux-protocol)
crux-clipboard  (NSPasteboard, drag-and-drop — depends on crux-protocol)
crux-mcp        (MCP server, 30 tools, rmcp SDK — depends on crux-protocol) [planned]
crux-mcp-bridge (stdio ↔ Unix socket bridge for Claude Desktop) [planned]
```

All crates live under `crates/`. The root `Cargo.toml` is workspace-only.

## Key Dependencies (pinned versions matter)

| Crate | Version | Notes |
|-------|---------|-------|
| `gpui` | `0.2.2` | From crates.io, NOT git. Pre-1.0 with breaking changes between versions |
| `gpui-component` | `0.5.1` | DockArea, Tabs, ResizablePanel (60+ widgets) |
| `alacritty_terminal` | `0.25` | VT100/xterm parser, grid, selection, damage tracking |
| `portable-pty` | `0.9` | PTY creation and management |
| `objc2` + `objc2-app-kit` | latest | NSTextInputClient (IME), NSPasteboard (clipboard) |

## Architecture Patterns

- **Entity-View-Element**: GPUI's native pattern. `CruxTerminal` (entity/state) → `CruxTerminalView` (view/controller) → `CruxTerminalElement` (GPU renderer)
- **Damage tracking**: Only re-render changed cells. Inherited from `alacritty_terminal::TermDamage`
- **Event batching**: Max 100 events or 4ms window before flush (Zed pattern)
- **Dual protocol**: IPC (Unix socket) for external control + in-band escape sequences (OSC/DCS/APC) for PTY apps
- **Protocol namespace**: `crux:<domain>/<action>` over JSON-RPC 2.0

## Commit Convention

[Conventional Commits](https://www.conventionalcommits.org/) with crate-name scopes. English only.

### Format

```
<type>(<scope>): <subject>

[body]

[footer]
```

### Types

| Type | When |
|------|------|
| `feat` | New feature or capability |
| `fix` | Bug fix |
| `docs` | Documentation only |
| `style` | Formatting, no logic change (rustfmt) |
| `refactor` | Code restructuring, no behavior change |
| `perf` | Performance improvement |
| `test` | Adding or fixing tests |
| `build` | Build system, dependencies, Cargo.toml |
| `ci` | CI/CD configuration |
| `chore` | Maintenance tasks (gitignore, tooling) |

### Scopes (required for code changes)

Use the crate name without `crux-` prefix for brevity:

| Scope | Crate |
|-------|-------|
| `protocol` | crux-protocol |
| `terminal` | crux-terminal |
| `terminal-view` | crux-terminal-view |
| `app` | crux-app |
| `ipc` | crux-ipc |
| `clipboard` | crux-clipboard |

Non-crate scopes: `deps`, `ci`, `workspace`, `release`

Omit scope only for project-wide changes: `chore: update LICENSE headers`

### Rules

- **Subject**: imperative mood, lowercase, no period, max 72 chars
- **Body**: wrap at 72 chars, explain *why* not *what*
- **Breaking changes**: `feat(terminal)!: ...` + `BREAKING CHANGE:` footer
- **Issue refs**: `Closes #123` or `Refs #456` in footer

### Examples

```
feat(terminal): add sixel graphics rendering

Implement DECSIXEL parsing in the VT emulator to support
inline image display via the Sixel protocol.

Closes #42
```

```
fix(app): prevent panic on zero-size window resize

The GPUI resize callback could fire with (0, 0) dimensions
during rapid window manipulation, causing a division by zero
in the cell grid calculation.
```

```
refactor(terminal-view): extract cursor rendering to separate method
```

```
build(deps): bump gpui to 0.2.3
```

## Code Style

- `cargo fmt` before every commit (rustfmt defaults)
- Zero `clippy` warnings (`-D warnings` enforced)
- Small, focused functions
- Tests required for new functionality
- English for code and public API docs; Korean acceptable in research docs

## Important Gotchas

- **GPUI requires full Xcode.app** — Metal shader compilation fails with CLT alone. Switch with: `sudo xcode-select --switch /Applications/Xcode.app/Contents/Developer`
- **No `build.rs` needed** — GPUI handles Metal shaders and framework linking internally
- **Dev profile optimization** — Set `[profile.dev.package."*"] opt-level = 2` or GPUI rendering will be unusably slow in debug
- **TERM name** — Use `xterm-crux` (not `crux`). The `xterm-` prefix is critical for compatibility; Ghostty learned this the hard way
- **IME preedit must never touch PTY** — Composition text is rendered as an overlay, committed text goes to PTY write. Mixing these causes the bugs documented in `research/platform/ime-clipboard.md`
- **Claude Code is proprietary** — Cannot submit direct PRs. Strategy: build CLI matching `PaneBackend` interface (13 methods), then submit Feature Request. See `PLAN.md:13`

## Research Documents

Extensive research (~480KB) lives in `research/`. See `research/README.md` for the full index with task-based navigation.

All documents have YAML frontmatter (`title`, `description`, `phase`, `topics`, `related`) for AI discovery.
Path-scoped rules in `.claude/rules/` auto-inject relevant research when working on specific crates.

| Directory | Scope | Documents |
|-----------|-------|-----------|
| `research/core/` | Terminal core | `terminal-emulation.md`, `terminal-architecture.md`, `keymapping.md`, `terminfo.md` |
| `research/gpui/` | GPUI framework | `framework.md`, `terminal-implementations.md`, `bootstrap.md` |
| `research/integration/` | IPC & Claude Code | `ipc-protocol-design.md`, `ipc-external-patterns.md`, `claude-code-strategy.md` |
| `research/platform/` | macOS native | `ime-clipboard.md`, `homebrew-distribution.md` |
| `research/` | Meta | `gap-analysis.md` (needs update) |

## Implementation Plan

See `PLAN.md` for the full 6-phase roadmap with 200+ checklist items:
1. **Phase 1**: Basic terminal MVP (shell rendering, keyboard, VT emulation, terminfo)
2. **Phase 2**: Tabs, split panes, IPC server, CLI, shell integration
3. **Phase 3**: Korean/CJK IME, rich clipboard, drag-and-drop
4. **Phase 4**: Markdown preview, links, graphics protocols, Kitty keyboard protocol
5. **Phase 5**: tmux compatibility, Claude Code Feature Request, config system
6. **Phase 6**: Homebrew distribution, code signing, notarization, Universal Binary
