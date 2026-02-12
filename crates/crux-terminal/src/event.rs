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
            let _ = self.sender.send(event);
        }
    }
}
