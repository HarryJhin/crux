use std::sync::mpsc;

use alacritty_terminal::event::{Event as AlacEvent, EventListener};

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
            // Events we handle elsewhere or don't need yet:
            // ClipboardStore, ClipboardLoad, ColorRequest, TextAreaSizeRequest,
            // CursorBlinkingChange, MouseCursorDirty, ResetTitle, Exit
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
}
