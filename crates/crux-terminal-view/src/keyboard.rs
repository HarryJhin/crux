//! Keyboard input handling for CruxTerminalView.

use gpui::*;

use crate::input;
use crate::input::OptionAsAlt;
use crate::view::CruxTerminalView;

impl CruxTerminalView {
    pub(crate) fn handle_key_down(
        &mut self,
        event: &KeyDownEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Reset cursor blink on any key input.
        self.reset_cursor_blink();

        // HARDENING 1: Modifier Key Isolation (Ghostty #4634)
        // When composing, ignore standalone modifier keys (Ctrl, Shift, Cmd, Option alone).
        // These must NOT destroy the preedit.
        if self.marked_text.is_some() && Self::is_standalone_modifier(&event.keystroke) {
            return; // Ignore modifier-only keystrokes during composition.
        }

        // Handle Cmd+V for paste before forwarding to terminal.
        if event.keystroke.modifiers.platform && event.keystroke.key.as_str() == "v" {
            self.paste_from_clipboard(cx);
            cx.stop_propagation();
            return;
        }

        // Handle Cmd+C for copy before forwarding to terminal.
        if event.keystroke.modifiers.platform && event.keystroke.key.as_str() == "c" {
            self.copy_selection(_window, cx);
            cx.stop_propagation();
            return;
        }

        // Handle Cmd+A for select all.
        if event.keystroke.modifiers.platform && event.keystroke.key.as_str() == "a" {
            self.select_all();
            cx.notify();
            cx.stop_propagation();
            return;
        }

        // HARDENING 2: Event Deduplication (Alacritty #8079)
        // If keystroke matches recent IME commit within dedup window, drop it.
        if let Some((ref last_text, last_time)) = self.last_ime_commit {
            if last_time.elapsed() < super::view::IME_DEDUP_WINDOW
                && event.keystroke.key == last_text.as_str()
            {
                // This keystroke is a duplicate of the IME commit. Drop it.
                cx.stop_propagation();
                return;
            }
        }

        // Character keys without special modifiers -> let IME handle via
        // replace_text_in_range(). This avoids double-processing: if we wrote
        // to the PTY here, the IME would also write via insertText:.
        if Self::is_ime_candidate(&event.keystroke, self.option_as_alt) {
            log::debug!(
                "[IME] is_ime_candidate=true, letting key '{}' pass to IME",
                event.keystroke.key,
            );
            return; // Don't stop propagation -- let event reach IME.
        }

        // Get the current terminal mode for application cursor key detection.
        let mode = self.terminal.content().mode;

        log::debug!(
            "[IME] key '{}' NOT ime_candidate, sending to PTY directly",
            event.keystroke.key,
        );

        if let Some(bytes) = input::keystroke_to_bytes(&event.keystroke, mode, self.option_as_alt) {
            // Non-IME keys (arrows, enter, etc.) invalidate the IME buffer
            // since the cursor position in the shell has changed.
            self.ime_buffer.clear();
            // Clear selection when typing.
            self.terminal.with_term_mut(|term| {
                term.selection = None;
            });
            self.terminal.write_to_pty(&bytes);
            cx.stop_propagation();
            cx.notify();
        }
    }

    /// Trim the IME buffer to a reasonable size.
    /// Only the last few characters are needed for Korean recombination.
    pub(crate) fn trim_ime_buffer(&mut self) {
        const MAX_IME_BUFFER_CHARS: usize = 8;
        let char_count = self.ime_buffer.chars().count();
        if char_count > MAX_IME_BUFFER_CHARS {
            let skip = char_count - MAX_IME_BUFFER_CHARS;
            let byte_offset = self
                .ime_buffer
                .char_indices()
                .nth(skip)
                .map(|(i, _)| i)
                .unwrap_or(0);
            self.ime_buffer.drain(..byte_offset);
        }
    }

    /// Returns true if the keystroke is a standalone modifier key (no character).
    /// Used to prevent modifiers from destroying IME composition (Ghostty #4634).
    pub(crate) fn is_standalone_modifier(keystroke: &Keystroke) -> bool {
        matches!(
            keystroke.key.as_str(),
            "shift" | "control" | "alt" | "cmd" | "option" | "command"
        )
    }

    /// Returns true if the keystroke should be handled by IME rather than directly.
    ///
    /// Character keys without Ctrl/Alt/Cmd/Fn modifiers go through the IME pipeline
    /// so that composition (e.g. Korean jamo assembly) works correctly.
    pub(crate) fn is_ime_candidate(keystroke: &Keystroke, option_as_alt: OptionAsAlt) -> bool {
        if keystroke.modifiers.platform
            || keystroke.modifiers.control
            || keystroke.modifiers.function
        {
            return false;
        }
        // Alt+key sends ESC prefix when option_as_alt is enabled -- bypass IME.
        if keystroke.modifiers.alt {
            match option_as_alt {
                OptionAsAlt::None => {} // macOS special char; let IME handle.
                _ => return false,      // Terminal Alt behavior; handle directly.
            }
        }
        // Named terminal control keys produce escape sequences, not character input.
        !matches!(
            keystroke.key.as_str(),
            "enter"
                | "tab"
                | "backspace"
                | "escape"
                | "space"
                | "up"
                | "down"
                | "left"
                | "right"
                | "home"
                | "end"
                | "insert"
                | "delete"
                | "pageup"
                | "pagedown"
                | "f1"
                | "f2"
                | "f3"
                | "f4"
                | "f5"
                | "f6"
                | "f7"
                | "f8"
                | "f9"
                | "f10"
                | "f11"
                | "f12"
        )
    }
}

#[cfg(test)]
mod tests {
    use crate::input::OptionAsAlt;
    use crate::view::CruxTerminalView;
    use gpui::{Keystroke, Modifiers};

    #[test]
    fn test_is_standalone_modifier() {
        // Test known modifier keys
        assert!(CruxTerminalView::is_standalone_modifier(&Keystroke {
            key: "shift".into(),
            modifiers: Modifiers::default(),
            key_char: None,
        }));
        assert!(CruxTerminalView::is_standalone_modifier(&Keystroke {
            key: "control".into(),
            modifiers: Modifiers::default(),
            key_char: None,
        }));
        assert!(CruxTerminalView::is_standalone_modifier(&Keystroke {
            key: "alt".into(),
            modifiers: Modifiers::default(),
            key_char: None,
        }));
        assert!(CruxTerminalView::is_standalone_modifier(&Keystroke {
            key: "cmd".into(),
            modifiers: Modifiers::default(),
            key_char: None,
        }));
        assert!(CruxTerminalView::is_standalone_modifier(&Keystroke {
            key: "option".into(),
            modifiers: Modifiers::default(),
            key_char: None,
        }));
        assert!(CruxTerminalView::is_standalone_modifier(&Keystroke {
            key: "command".into(),
            modifiers: Modifiers::default(),
            key_char: None,
        }));

        // Test non-modifier keys
        assert!(!CruxTerminalView::is_standalone_modifier(&Keystroke {
            key: "a".into(),
            modifiers: Modifiers::default(),
            key_char: None,
        }));
        assert!(!CruxTerminalView::is_standalone_modifier(&Keystroke {
            key: "enter".into(),
            modifiers: Modifiers::default(),
            key_char: None,
        }));
        assert!(!CruxTerminalView::is_standalone_modifier(&Keystroke {
            key: "tab".into(),
            modifiers: Modifiers::default(),
            key_char: None,
        }));
    }

    #[test]
    fn test_is_ime_candidate_character_keys() {
        // Plain character keys should be IME candidates
        assert!(CruxTerminalView::is_ime_candidate(
            &Keystroke {
                key: "a".into(),
                modifiers: Modifiers::default(),
                key_char: None,
            },
            OptionAsAlt::None
        ));
        assert!(CruxTerminalView::is_ime_candidate(
            &Keystroke {
                key: "ㄱ".into(),
                modifiers: Modifiers::default(),
                key_char: None,
            },
            OptionAsAlt::None
        ));
    }

    #[test]
    fn test_is_ime_candidate_with_modifiers() {
        // Keys with Cmd/Ctrl/Fn should not be IME candidates
        assert!(!CruxTerminalView::is_ime_candidate(
            &Keystroke {
                key: "a".into(),
                modifiers: Modifiers {
                    platform: true,
                    ..Default::default()
                },
                key_char: None,
            },
            OptionAsAlt::None
        ));
        assert!(!CruxTerminalView::is_ime_candidate(
            &Keystroke {
                key: "a".into(),
                modifiers: Modifiers {
                    control: true,
                    ..Default::default()
                },
                key_char: None,
            },
            OptionAsAlt::None
        ));
        assert!(!CruxTerminalView::is_ime_candidate(
            &Keystroke {
                key: "a".into(),
                modifiers: Modifiers {
                    function: true,
                    ..Default::default()
                },
                key_char: None,
            },
            OptionAsAlt::None
        ));
    }

    #[test]
    fn test_is_ime_candidate_alt_key_behavior() {
        // Alt+key with OptionAsAlt::None should be IME candidate (macOS special chars)
        assert!(CruxTerminalView::is_ime_candidate(
            &Keystroke {
                key: "a".into(),
                modifiers: Modifiers {
                    alt: true,
                    ..Default::default()
                },
                key_char: None,
            },
            OptionAsAlt::None
        ));

        // Alt+key with OptionAsAlt::Both/Left/Right should not be IME candidate
        assert!(!CruxTerminalView::is_ime_candidate(
            &Keystroke {
                key: "a".into(),
                modifiers: Modifiers {
                    alt: true,
                    ..Default::default()
                },
                key_char: None,
            },
            OptionAsAlt::Both
        ));
        assert!(!CruxTerminalView::is_ime_candidate(
            &Keystroke {
                key: "a".into(),
                modifiers: Modifiers {
                    alt: true,
                    ..Default::default()
                },
                key_char: None,
            },
            OptionAsAlt::Left
        ));
        assert!(!CruxTerminalView::is_ime_candidate(
            &Keystroke {
                key: "a".into(),
                modifiers: Modifiers {
                    alt: true,
                    ..Default::default()
                },
                key_char: None,
            },
            OptionAsAlt::Right
        ));
    }

    #[test]
    fn test_is_ime_candidate_control_keys() {
        // Named terminal control keys should not be IME candidates
        let control_keys = [
            "enter",
            "tab",
            "backspace",
            "escape",
            "space",
            "up",
            "down",
            "left",
            "right",
            "home",
            "end",
            "insert",
            "delete",
            "pageup",
            "pagedown",
            "f1",
            "f2",
            "f3",
            "f4",
            "f5",
            "f6",
            "f7",
            "f8",
            "f9",
            "f10",
            "f11",
            "f12",
        ];

        for key in control_keys {
            assert!(
                !CruxTerminalView::is_ime_candidate(
                    &Keystroke {
                        key: key.into(),
                        modifiers: Modifiers::default(),
                        key_char: None,
                    },
                    OptionAsAlt::None
                ),
                "Control key '{}' should not be IME candidate",
                key
            );
        }
    }
}
