---
paths:
  - "crates/crux-terminal-view/**"
  - "crates/crux-app/**"
---

# GPUI Rendering Development

Before implementing UI/rendering features, consult these research documents:

- `research/gpui/framework.md` — GPUI rendering pipeline, component system, IME support, limitations
- `research/gpui/terminal-implementations.md` — Source analysis of gpui-ghostty, Zed terminal, gpui-terminal
- `research/gpui/bootstrap.md` — Cargo workspace setup, dependency config, minimal app

Key decisions:
- Use `gpui = "0.2.2"` from crates.io (NOT git dependency)
- Use `gpui-component = "0.5.1"` for DockArea, Tabs, ResizablePanel
- Entity-View-Element pattern: CruxTerminal (entity) → CruxTerminalView (view) → CruxTerminalElement (element)
- Event batching: max 100 events or 4ms window before flush
- Dev profile: `[profile.dev.package."*"] opt-level = 2` (required for usable GPUI performance)
- No build.rs needed — GPUI handles Metal shaders internally
- Requires full Xcode.app (not just CLT) for Metal shader compilation
