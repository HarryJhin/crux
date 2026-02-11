---
paths:
  - "crates/crux-ipc/**"
---

# IPC Integration Development

Before implementing IPC features, consult these research documents:

- `research/integration/ipc-protocol-design.md` — Crux IPC architecture, protocol spec, CLI interface design
- `research/integration/ipc-external-patterns.md` — WezTerm CLI internals, JSON-RPC 2.0, security, event subscription
- `research/integration/claude-code-strategy.md` — Claude Code PaneBackend interface, Feature Request strategy

Key decisions:
- Protocol: JSON-RPC 2.0 over Unix domain socket
- Namespace: `crux:<domain>/<action>` (hierarchical, extensible)
- CLI must match Claude Code's PaneBackend interface (13 methods)
- Socket path: `$XDG_RUNTIME_DIR/crux-$PID.sock` or `$TMPDIR/crux-$PID.sock`
- Security: file permission (0600), PID verification, optional token auth
