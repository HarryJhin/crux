---
paths:
  - "crates/crux-clipboard/**"
---

# Platform Native Development

Before implementing clipboard/IME features, consult these research documents:

- `research/platform/ime-clipboard.md` — NSTextInputClient, Korean IME failure analysis, NSPasteboard, objc2 bindings, drag-and-drop
- `research/core/keymapping.md` — Keyboard input handling (related to IME interaction)
- `research/gpui/framework.md` § 3 — GPUI's IME support and limitations

Key decisions:
- Use `objc2` + `objc2-app-kit` for all macOS native bindings (not objc crate)
- IME preedit MUST be rendered as overlay — never send composition text to PTY
- NSPasteboard for rich clipboard (images, RTF, file promises)
- Vim mode IME auto-switch: detect cursor shape changes, toggle to ASCII in Normal mode
- Known competitor bugs to avoid: Alacritty double-space (#8079), Ghostty preedit destruction (#4634)
