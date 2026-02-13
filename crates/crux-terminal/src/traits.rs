//! Trait abstractions for terminal emulation.
//!
//! The [`Terminal`] trait captures the essential interface that UI layers
//! (e.g. `crux-terminal-view`) need from a terminal emulator. This enables
//! testing with mock implementations and future alternative backends.

use alacritty_terminal::grid::Scroll;
use alacritty_terminal::term::Term;

use crate::event::{CruxEventListener, SemanticZone, TerminalEvent};
use crate::terminal::{TerminalContent, TerminalSize};

/// Core terminal emulator interface.
///
/// This trait abstracts over the concrete `CruxTerminal` implementation,
/// enabling mock terminals for testing and potential alternative backends.
///
/// # Design Notes
///
/// `with_term` and `with_term_mut` expose `alacritty_terminal::Term` directly.
/// This is a pragmatic choice: the view layer needs direct access for selection
/// manipulation and grid queries. A future refactor could replace these with
/// higher-level methods (e.g. `clear_selection()`, `set_selection()`), but for
/// now we prioritize minimal disruption.
pub trait Terminal {
    /// Write keyboard input or other data to the PTY.
    fn write_to_pty(&mut self, data: &[u8]);

    /// Resize the terminal grid and PTY.
    fn resize(&mut self, size: TerminalSize);

    /// Create a snapshot of the terminal content for rendering.
    ///
    /// Returns an owned snapshot that can be used without holding any lock.
    fn content(&self) -> TerminalContent;

    /// Drain pending events from the terminal.
    fn drain_events(&mut self) -> Vec<TerminalEvent>;

    /// Current working directory reported by the shell via OSC 7.
    ///
    /// Returns `None` if the shell has not yet reported a CWD.
    fn cwd(&self) -> Option<&str>;

    /// Get the current terminal size.
    fn size(&self) -> TerminalSize;

    /// Scroll the terminal display.
    fn scroll_display(&self, scroll: Scroll);

    /// Get the selected text as a string, if any selection is active.
    fn selection_to_string(&self) -> Option<String>;

    /// Get all completed semantic zones from OSC 133 shell integration.
    fn semantic_zones(&self) -> &[SemanticZone];

    /// Check if the child process is still running.
    fn is_process_running(&mut self) -> bool;

    /// Get the child process PID.
    fn child_pid(&self) -> Option<u32>;

    /// Access the terminal state under a lock (read-only).
    ///
    /// Exposes alacritty_terminal internals directly. This is pragmatic;
    /// a future refactor should replace callers with higher-level methods.
    fn with_term<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&Term<CruxEventListener>) -> R;

    /// Access the terminal state mutably under a lock.
    ///
    /// Exposes alacritty_terminal internals directly. This is pragmatic;
    /// a future refactor should replace callers with higher-level methods.
    fn with_term_mut<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut Term<CruxEventListener>) -> R;
}

#[cfg(test)]
pub mod mock {
    //! Mock terminal implementation for testing.

    use super::*;
    use std::cell::RefCell;

    use alacritty_terminal::index::{Column, Line, Point};
    use alacritty_terminal::term::{Config, TermMode};
    use alacritty_terminal::vte::ansi::CursorShape;

    use crate::terminal::{CursorState, DamageState};

    /// Mock terminal for testing the view layer without a real PTY.
    ///
    /// Stores written data and provides configurable return values for queries.
    pub struct MockTerminal {
        /// Data written to the PTY via write_to_pty()
        pub written_data: RefCell<Vec<u8>>,
        /// Events to drain from drain_events()
        pub mock_events: RefCell<Vec<TerminalEvent>>,
        /// CWD to return from cwd()
        pub mock_cwd: RefCell<Option<String>>,
        /// Size to return from size()
        pub mock_size: RefCell<TerminalSize>,
        /// Selection text to return from selection_to_string()
        pub mock_selection: RefCell<Option<String>>,
        /// Semantic zones to return from semantic_zones()
        pub mock_zones: RefCell<Vec<SemanticZone>>,
        /// Process running state
        pub mock_process_running: RefCell<bool>,
        /// Child PID
        pub mock_child_pid: RefCell<Option<u32>>,
        /// Call count for write_to_pty
        pub write_count: RefCell<usize>,
    }

    impl MockTerminal {
        /// Create a new MockTerminal with default values.
        pub fn new(size: TerminalSize) -> Self {
            Self {
                written_data: RefCell::new(Vec::new()),
                mock_events: RefCell::new(Vec::new()),
                mock_cwd: RefCell::new(None),
                mock_size: RefCell::new(size),
                mock_selection: RefCell::new(None),
                mock_zones: RefCell::new(Vec::new()),
                mock_process_running: RefCell::new(true),
                mock_child_pid: RefCell::new(Some(12345)),
                write_count: RefCell::new(0),
            }
        }

        /// Get the data written to the PTY.
        pub fn take_written_data(&self) -> Vec<u8> {
            self.written_data.borrow_mut().drain(..).collect()
        }

        /// Get the data written to the PTY as a UTF-8 string (lossy).
        pub fn take_written_string(&self) -> String {
            String::from_utf8_lossy(&self.take_written_data()).into_owned()
        }

        /// Set the CWD that will be returned by cwd().
        pub fn set_cwd(&self, cwd: Option<String>) {
            *self.mock_cwd.borrow_mut() = cwd;
        }

        /// Set the selection text that will be returned by selection_to_string().
        pub fn set_selection(&self, text: Option<String>) {
            *self.mock_selection.borrow_mut() = text;
        }

        /// Add an event that will be drained by drain_events().
        pub fn push_event(&self, event: TerminalEvent) {
            self.mock_events.borrow_mut().push(event);
        }

        /// Set whether the process is running.
        pub fn set_process_running(&self, running: bool) {
            *self.mock_process_running.borrow_mut() = running;
        }
    }

    impl Terminal for MockTerminal {
        fn write_to_pty(&mut self, data: &[u8]) {
            self.written_data.borrow_mut().extend_from_slice(data);
            *self.write_count.borrow_mut() += 1;
        }

        fn resize(&mut self, size: TerminalSize) {
            *self.mock_size.borrow_mut() = size;
        }

        fn content(&self) -> TerminalContent {
            let size = *self.mock_size.borrow();
            TerminalContent {
                cells: Vec::new(),
                cursor: CursorState {
                    point: Point::new(Line(0), Column(0)),
                    shape: CursorShape::Block,
                },
                mode: TermMode::empty(),
                display_offset: 0,
                selection: None,
                cols: size.cols,
                rows: size.rows,
                damage: DamageState::default(),
            }
        }

        fn drain_events(&mut self) -> Vec<TerminalEvent> {
            self.mock_events.borrow_mut().drain(..).collect()
        }

        fn cwd(&self) -> Option<&str> {
            // SAFETY: This is unsafe but necessary for the mock.
            // The reference is valid as long as the MockTerminal lives.
            // In production code, consider using a different API.
            unsafe {
                self.mock_cwd
                    .borrow()
                    .as_ref()
                    .map(|s| std::mem::transmute::<&str, &str>(s.as_str()))
            }
        }

        fn size(&self) -> TerminalSize {
            *self.mock_size.borrow()
        }

        fn scroll_display(&self, _scroll: Scroll) {
            // No-op for mock
        }

        fn selection_to_string(&self) -> Option<String> {
            self.mock_selection.borrow().clone()
        }

        fn semantic_zones(&self) -> &[SemanticZone] {
            // SAFETY: This is unsafe but necessary for the mock.
            // The reference is valid as long as the MockTerminal lives.
            unsafe {
                let zones = self.mock_zones.borrow();
                std::mem::transmute::<&[SemanticZone], &[SemanticZone]>(zones.as_slice())
            }
        }

        fn is_process_running(&mut self) -> bool {
            *self.mock_process_running.borrow()
        }

        fn child_pid(&self) -> Option<u32> {
            *self.mock_child_pid.borrow()
        }

        fn with_term<F, R>(&self, f: F) -> R
        where
            F: FnOnce(&Term<CruxEventListener>) -> R,
        {
            // Create a temporary term for testing
            use alacritty_terminal::term::test::TermSize;
            let size = *self.mock_size.borrow();
            let term_size = TermSize::new(size.cols as usize, size.rows as usize);
            let term = Term::new(Config::default(), &term_size, {
                let (tx, _rx) = std::sync::mpsc::channel();
                CruxEventListener::new(tx)
            });
            f(&term)
        }

        fn with_term_mut<F, R>(&self, f: F) -> R
        where
            F: FnOnce(&mut Term<CruxEventListener>) -> R,
        {
            // Create a temporary term for testing
            use alacritty_terminal::term::test::TermSize;
            let size = *self.mock_size.borrow();
            let term_size = TermSize::new(size.cols as usize, size.rows as usize);
            let mut term = Term::new(Config::default(), &term_size, {
                let (tx, _rx) = std::sync::mpsc::channel();
                CruxEventListener::new(tx)
            });
            f(&mut term)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_terminal_write_to_pty() {
        use mock::MockTerminal;

        let mut mock = MockTerminal::new(TerminalSize {
            rows: 24,
            cols: 80,
            cell_width: 8.0,
            cell_height: 16.0,
        });

        mock.write_to_pty(b"hello");
        mock.write_to_pty(b" world");

        assert_eq!(mock.take_written_string(), "hello world");
        assert_eq!(*mock.write_count.borrow(), 2);
    }

    #[test]
    fn test_mock_terminal_resize() {
        use mock::MockTerminal;

        let mut mock = MockTerminal::new(TerminalSize {
            rows: 24,
            cols: 80,
            cell_width: 8.0,
            cell_height: 16.0,
        });

        let new_size = TerminalSize {
            rows: 30,
            cols: 100,
            cell_width: 8.0,
            cell_height: 16.0,
        };
        mock.resize(new_size);

        let content = mock.content();
        assert_eq!(content.rows, 30);
        assert_eq!(content.cols, 100);
    }

    #[test]
    fn test_mock_terminal_cwd() {
        use mock::MockTerminal;

        let mock = MockTerminal::new(TerminalSize {
            rows: 24,
            cols: 80,
            cell_width: 8.0,
            cell_height: 16.0,
        });

        assert_eq!(mock.cwd(), None);

        mock.set_cwd(Some("/home/user".into()));
        assert_eq!(mock.cwd(), Some("/home/user"));
    }

    #[test]
    fn test_mock_terminal_selection() {
        use mock::MockTerminal;

        let mock = MockTerminal::new(TerminalSize {
            rows: 24,
            cols: 80,
            cell_width: 8.0,
            cell_height: 16.0,
        });

        assert_eq!(mock.selection_to_string(), None);

        mock.set_selection(Some("selected text".into()));
        assert_eq!(mock.selection_to_string(), Some("selected text".into()));
    }

    #[test]
    fn test_mock_terminal_events() {
        use mock::MockTerminal;

        let mut mock = MockTerminal::new(TerminalSize {
            rows: 24,
            cols: 80,
            cell_width: 8.0,
            cell_height: 16.0,
        });

        mock.push_event(TerminalEvent::Title("Test".into()));
        mock.push_event(TerminalEvent::Bell);

        let events = mock.drain_events();
        assert_eq!(events.len(), 2);
        assert!(matches!(events[0], TerminalEvent::Title(_)));
        assert!(matches!(events[1], TerminalEvent::Bell));

        // Events should be drained
        let events2 = mock.drain_events();
        assert_eq!(events2.len(), 0);
    }

    #[test]
    fn test_mock_terminal_process_state() {
        use mock::MockTerminal;

        let mut mock = MockTerminal::new(TerminalSize {
            rows: 24,
            cols: 80,
            cell_width: 8.0,
            cell_height: 16.0,
        });

        assert!(mock.is_process_running());
        assert_eq!(mock.child_pid(), Some(12345));

        mock.set_process_running(false);
        assert!(!mock.is_process_running());
    }

    #[test]
    fn test_mock_terminal_size() {
        use mock::MockTerminal;

        let mock = MockTerminal::new(TerminalSize {
            rows: 40,
            cols: 120,
            cell_width: 8.0,
            cell_height: 16.0,
        });

        let size = mock.size();
        assert_eq!(size.rows, 40);
        assert_eq!(size.cols, 120);
    }
}
