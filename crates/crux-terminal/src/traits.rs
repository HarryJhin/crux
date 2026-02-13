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
