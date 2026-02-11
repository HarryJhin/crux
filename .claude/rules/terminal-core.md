---
paths:
  - "crates/crux-terminal/**"
  - "crates/crux-protocol/**"
---

# Terminal Core Development

Before implementing terminal core features, consult these research documents:

- `research/core/terminal-emulation.md` — VT parser (alacritty_terminal), PTY (portable-pty), graphics protocols, tmux compat, Unicode/CJK
- `research/core/terminal-architecture.md` — Architecture patterns from Alacritty, WezTerm, Rio, Ghostty
- `research/core/keymapping.md` — Escape sequence tables, Kitty keyboard protocol, modifier encoding
- `research/core/terminfo.md` — Terminfo format, TERM strategy, modern capabilities

Key decisions:
- Use `alacritty_terminal = "0.25"` as VT parser (not vte or libghostty)
- Use `portable-pty = "0.9"` for PTY management
- Entity pattern: `CruxTerminal` wraps `Term<CruxListener>` with PTY + event batching
- Damage tracking via `alacritty_terminal::TermDamage` — only re-render changed cells
- TERM name: `xterm-crux` (xterm- prefix required for compatibility)
