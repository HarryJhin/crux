use std::sync::mpsc;

use alacritty_terminal::event::{Event as AlacEvent, EventListener};
use alacritty_terminal::vte::ansi::CursorShape;

/// Semantic zone types from OSC 133 (FinalTerm) shell integration.
///
/// Shells that support prompt marking emit OSC 133 sequences to delimit
/// regions of terminal output into prompt, user input, and command output.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SemanticZoneType {
    /// Prompt text (between 133;A and 133;B).
    Prompt,
    /// User-typed command (between 133;B and 133;C).
    Input,
    /// Command output (between 133;C and 133;D).
    Output,
}

/// A semantic zone marking a region of terminal output.
#[derive(Debug, Clone)]
pub struct SemanticZone {
    pub start_line: i32,
    pub start_col: usize,
    pub end_line: i32,
    pub end_col: usize,
    pub zone_type: SemanticZoneType,
    /// Exit code from 133;D (only meaningful for Output zones).
    pub exit_code: Option<i32>,
}

/// Graphics protocol identifier for inline image support.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GraphicsProtocol {
    /// Kitty graphics protocol (APC-based, chunked transfer).
    Kitty,
    /// iTerm2 inline image protocol (OSC 1337).
    Iterm2,
}

/// Events produced by the terminal emulator for the UI layer.
#[derive(Debug, Clone)]
pub enum TerminalEvent {
    /// Terminal content changed; UI should repaint.
    Wakeup,
    /// Window title changed.
    Title(String),
    /// Bell character received.
    Bell,
    /// Terminal requests text be written to PTY (e.g. DSR response).
    PtyWrite(String),
    /// Child process exited.
    ProcessExit(i32),
    /// Shell reports current working directory (OSC 7).
    ///
    /// The payload is the decoded directory path (e.g. `/Users/jjh/Projects`).
    /// Shells emit `ESC ] 7 ; file://hostname/path ST` after each command.
    CwdChanged(String),
    /// Shell integration prompt mark (OSC 133).
    ///
    /// Emitted when the byte-stream scanner detects an OSC 133 sequence.
    /// The payload indicates which boundary was crossed, with an optional
    /// exit code for `D` (command-complete) markers.
    PromptMark {
        mark: SemanticZoneType,
        /// Exit code carried by `133;D;N`. `None` for A/B/C markers
        /// and for D markers that omit the exit code.
        exit_code: Option<i32>,
    },
    /// Program requested clipboard write via OSC 52.
    ClipboardSet { data: String },
    /// Inline graphics data received via Kitty APC or iTerm2 OSC 1337.
    Graphics {
        protocol: GraphicsProtocol,
        payload: Vec<u8>,
    },
    /// Cursor shape changed (e.g. Vim mode switch detected via DECSCUSR).
    CursorShapeChanged {
        old_shape: CursorShape,
        new_shape: CursorShape,
    },
}

/// Bridges alacritty_terminal events into our channel-based system.
pub struct CruxEventListener {
    sender: mpsc::Sender<TerminalEvent>,
}

impl CruxEventListener {
    pub fn new(sender: mpsc::Sender<TerminalEvent>) -> Self {
        Self { sender }
    }
}

impl EventListener for CruxEventListener {
    fn send_event(&self, event: AlacEvent) {
        let mapped = match event {
            AlacEvent::Wakeup => Some(TerminalEvent::Wakeup),
            AlacEvent::Title(title) => Some(TerminalEvent::Title(title)),
            AlacEvent::Bell => Some(TerminalEvent::Bell),
            AlacEvent::PtyWrite(text) => Some(TerminalEvent::PtyWrite(text)),
            AlacEvent::ChildExit(code) => Some(TerminalEvent::ProcessExit(code)),
            AlacEvent::ColorRequest(_idx, format_fn) => {
                // Respond with a default color to prevent vim/neovim startup delays.
                // Use black (0,0,0) as placeholder; a real theme system would
                // supply actual palette colors.
                let color_str = format_fn(alacritty_terminal::vte::ansi::Rgb { r: 0, g: 0, b: 0 });
                Some(TerminalEvent::PtyWrite(color_str))
            }
            AlacEvent::ClipboardStore(_, content) => {
                Some(TerminalEvent::ClipboardSet { data: content })
            }
            AlacEvent::ClipboardLoad(_, format_fn) => {
                // Return empty string to prevent hangs from unanswered requests.
                let response = format_fn("");
                Some(TerminalEvent::PtyWrite(response))
            }
            // Events we handle elsewhere or don't need yet:
            // TextAreaSizeRequest, CursorBlinkingChange, MouseCursorDirty, ResetTitle, Exit
            _ => None,
        };

        if let Some(event) = mapped {
            if let Err(e) = self.sender.send(event) {
                log::debug!("failed to send terminal event: {}", e);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    #[test]
    fn test_wakeup_event_mapping() {
        let (tx, rx) = mpsc::channel();
        let listener = CruxEventListener::new(tx);
        listener.send_event(AlacEvent::Wakeup);
        let event = rx.try_recv().unwrap();
        assert!(matches!(event, TerminalEvent::Wakeup));
    }

    #[test]
    fn test_title_event_mapping() {
        let (tx, rx) = mpsc::channel();
        let listener = CruxEventListener::new(tx);
        listener.send_event(AlacEvent::Title("test title".to_string()));
        let event = rx.try_recv().unwrap();
        assert!(matches!(event, TerminalEvent::Title(t) if t == "test title"));
    }

    #[test]
    fn test_bell_event_mapping() {
        let (tx, rx) = mpsc::channel();
        let listener = CruxEventListener::new(tx);
        listener.send_event(AlacEvent::Bell);
        let event = rx.try_recv().unwrap();
        assert!(matches!(event, TerminalEvent::Bell));
    }

    #[test]
    fn test_pty_write_event_mapping() {
        let (tx, rx) = mpsc::channel();
        let listener = CruxEventListener::new(tx);
        listener.send_event(AlacEvent::PtyWrite("hello".to_string()));
        let event = rx.try_recv().unwrap();
        assert!(matches!(event, TerminalEvent::PtyWrite(s) if s == "hello"));
    }

    #[test]
    fn test_child_exit_event_mapping() {
        let (tx, rx) = mpsc::channel();
        let listener = CruxEventListener::new(tx);
        listener.send_event(AlacEvent::ChildExit(42));
        let event = rx.try_recv().unwrap();
        assert!(matches!(event, TerminalEvent::ProcessExit(42)));
    }

    #[test]
    fn test_unhandled_events_are_dropped() {
        let (tx, rx) = mpsc::channel();
        let listener = CruxEventListener::new(tx);
        // Verify that after sending and receiving a mapped event,
        // the channel is empty (no extra events).
        listener.send_event(AlacEvent::Bell);
        let _ = rx.try_recv().unwrap();
        assert!(
            rx.try_recv().is_err(),
            "no extra events should be in the channel"
        );
    }

    #[test]
    fn test_cwd_changed_event() {
        // CwdChanged is not produced by the EventListener bridge
        // (it comes from the OSC 7 scanner in the PTY read loop),
        // but verify the variant is constructible and matchable.
        let event = TerminalEvent::CwdChanged("/Users/jjh".to_string());
        assert!(matches!(event, TerminalEvent::CwdChanged(p) if p == "/Users/jjh"));
    }

    #[test]
    fn test_prompt_mark_event() {
        // PromptMark is produced by the OSC 133 scanner, not the
        // EventListener bridge. Verify the variant is constructible.
        let event = TerminalEvent::PromptMark {
            mark: SemanticZoneType::Prompt,
            exit_code: None,
        };
        assert!(matches!(
            event,
            TerminalEvent::PromptMark {
                mark: SemanticZoneType::Prompt,
                exit_code: None,
            }
        ));
    }

    #[test]
    fn test_prompt_mark_with_exit_code() {
        let event = TerminalEvent::PromptMark {
            mark: SemanticZoneType::Output,
            exit_code: Some(1),
        };
        assert!(matches!(
            event,
            TerminalEvent::PromptMark {
                mark: SemanticZoneType::Output,
                exit_code: Some(1),
            }
        ));
    }

    #[test]
    fn test_semantic_zone_type_equality() {
        assert_eq!(SemanticZoneType::Prompt, SemanticZoneType::Prompt);
        assert_eq!(SemanticZoneType::Input, SemanticZoneType::Input);
        assert_eq!(SemanticZoneType::Output, SemanticZoneType::Output);
        assert_ne!(SemanticZoneType::Prompt, SemanticZoneType::Input);
    }

    #[test]
    fn test_clipboard_set_event() {
        let (tx, rx) = mpsc::channel();
        let listener = CruxEventListener::new(tx);
        listener.send_event(AlacEvent::ClipboardStore(
            alacritty_terminal::term::ClipboardType::Clipboard,
            "test data".to_string(),
        ));
        let event = rx.try_recv().unwrap();
        assert!(matches!(event, TerminalEvent::ClipboardSet { data } if data == "test data"));
    }
}
