use std::io::Write;
use std::sync::mpsc;
use std::sync::Arc;
use std::thread::JoinHandle;

use alacritty_terminal::grid::{Dimensions, Indexed, Scroll};
use alacritty_terminal::index::Point;
use alacritty_terminal::selection::SelectionRange;
use alacritty_terminal::sync::FairMutex;
use alacritty_terminal::term::cell::Flags;
use alacritty_terminal::term::{Config, Term, TermDamage, TermMode};
use alacritty_terminal::vte::ansi::{Color, CursorShape};

use crate::event::{CruxEventListener, TerminalEvent};
use crate::pty;

/// Default scrollback history size in lines.
const SCROLLBACK_LINES: usize = 10_000;

/// Terminal dimensions in cells and pixels.
#[derive(Debug, Clone, Copy)]
pub struct TerminalSize {
    pub rows: usize,
    pub cols: usize,
    pub cell_width: f32,
    pub cell_height: f32,
}

impl Dimensions for TerminalSize {
    fn total_lines(&self) -> usize {
        self.screen_lines() + SCROLLBACK_LINES
    }

    fn screen_lines(&self) -> usize {
        self.rows
    }

    fn columns(&self) -> usize {
        self.cols
    }
}

impl Default for TerminalSize {
    fn default() -> Self {
        Self {
            rows: 24,
            cols: 80,
            cell_width: 8.0,
            cell_height: 16.0,
        }
    }
}

/// Damage state captured from alacritty_terminal's damage tracking.
#[derive(Debug, Clone)]
pub enum DamageState {
    /// No lines were damaged since the last render.
    None,
    /// Only specific lines were damaged (partial update).
    Partial(Vec<LineDamage>),
    /// The entire terminal is damaged and needs full re-render.
    Full,
}

/// Damage bounds for a single line.
#[derive(Debug, Clone, Copy)]
pub struct LineDamage {
    pub line: usize,
    pub left: usize,
    pub right: usize,
}

/// Snapshot of terminal content for rendering.
///
/// This is a self-contained copy of the terminal state so that the
/// renderer never needs to hold a lock on the terminal.
pub struct TerminalContent {
    pub cells: Vec<IndexedCell>,
    pub cursor: CursorState,
    pub mode: TermMode,
    pub display_offset: usize,
    pub selection: Option<SelectionRange>,
    pub cols: usize,
    pub rows: usize,
    /// Damage information from alacritty_terminal's damage tracking.
    pub damage: DamageState,
}

/// A single cell with its grid position.
#[derive(Debug, Clone)]
pub struct IndexedCell {
    pub point: Point,
    pub c: char,
    pub fg: Color,
    pub bg: Color,
    pub flags: Flags,
}

/// Cursor rendering state.
#[derive(Debug, Clone, Copy)]
pub struct CursorState {
    pub point: Point,
    pub shape: CursorShape,
}

/// The core terminal entity. Owns the alacritty_terminal state, PTY
/// handles, and I/O threads.
pub struct CruxTerminal {
    term: Arc<FairMutex<Term<CruxEventListener>>>,
    pty_writer: Box<dyn Write + Send>,
    master_pty: Box<dyn portable_pty::MasterPty + Send>,
    child: Box<dyn portable_pty::Child + Send + Sync>,
    reader_thread: Option<JoinHandle<()>>,
    event_rx: mpsc::Receiver<TerminalEvent>,
    size: TerminalSize,
}

impl CruxTerminal {
    /// Create a new terminal, spawn the PTY, and start the I/O loop.
    ///
    /// If `shell` is `None`, the user's default shell is detected from
    /// the `SHELL` environment variable (falling back to `/bin/zsh`).
    pub fn new(shell: Option<String>, size: TerminalSize) -> anyhow::Result<Self> {
        let shell = shell.unwrap_or_else(pty::detect_shell);

        // Event channel for terminal → UI communication.
        let (event_tx, event_rx) = mpsc::channel();

        let event_listener = CruxEventListener::new(event_tx);

        // Create alacritty_terminal Term with default config.
        let config = Config::default();
        let term = Term::new(config, &size, event_listener);
        let term = Arc::new(FairMutex::new(term));

        // Spawn the PTY process.
        let (master_pty, child) = pty::spawn_pty(&shell, &size)?;

        // Get reader and writer handles from the master PTY.
        let reader = master_pty.try_clone_reader()?;
        let writer = master_pty.take_writer()?;

        // Start background PTY reader thread.
        let term_clone = term.clone();
        let reader_thread = pty::start_pty_read_loop(term_clone, reader, || {
            // The wakeup callback is intentionally minimal.
            // In the GPUI integration layer, this will be replaced
            // with a cx.notify() call via the event channel.
        });

        Ok(Self {
            term,
            pty_writer: writer,
            master_pty,
            child,
            reader_thread: Some(reader_thread),
            event_rx,
            size,
        })
    }

    /// Write keyboard input or other data to the PTY.
    pub fn write_to_pty(&mut self, data: &[u8]) {
        if let Err(e) = self.pty_writer.write_all(data) {
            log::warn!("failed to write to PTY: {}", e);
            return;
        }
        if let Err(e) = self.pty_writer.flush() {
            log::warn!("failed to flush PTY: {}", e);
        }
    }

    /// Resize the terminal grid and PTY.
    pub fn resize(&mut self, size: TerminalSize) {
        self.size = size;

        // Resize the alacritty terminal grid.
        self.term.lock().resize(size);

        // Resize the PTY so the child process gets SIGWINCH.
        if let Err(e) = self.master_pty.resize(portable_pty::PtySize {
            rows: size.rows as u16,
            cols: size.cols as u16,
            pixel_width: (size.cols as f32 * size.cell_width) as u16,
            pixel_height: (size.rows as f32 * size.cell_height) as u16,
        }) {
            log::warn!("failed to resize PTY: {}", e);
        }
    }

    /// Access the terminal state under a lock.
    pub fn with_term<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&Term<CruxEventListener>) -> R,
    {
        let term = self.term.lock();
        f(&term)
    }

    /// Access the terminal state mutably under a lock.
    pub fn with_term_mut<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut Term<CruxEventListener>) -> R,
    {
        let mut term = self.term.lock();
        f(&mut term)
    }

    /// Get a shared reference to the underlying `Arc<FairMutex<Term>>`.
    pub fn term_arc(&self) -> &Arc<FairMutex<Term<CruxEventListener>>> {
        &self.term
    }

    /// Create a snapshot of the terminal content for rendering.
    ///
    /// This locks the terminal briefly to copy all visible cells,
    /// then returns an owned snapshot that can be used without holding
    /// any lock.
    pub fn content(&self) -> TerminalContent {
        let mut term = self.term.lock();

        // Capture damage state before rendering.
        let damage = match term.damage() {
            TermDamage::Full => DamageState::Full,
            TermDamage::Partial(iter) => {
                let lines: Vec<LineDamage> = iter
                    .map(|d| LineDamage {
                        line: d.line,
                        left: d.left,
                        right: d.right,
                    })
                    .collect();
                if lines.is_empty() {
                    DamageState::None
                } else {
                    DamageState::Partial(lines)
                }
            }
        };

        // Reset damage after capturing it.
        term.reset_damage();

        let content = term.renderable_content();

        let cols = term.columns();
        let rows = term.screen_lines();

        let mut cells = Vec::with_capacity(cols * rows);
        for Indexed { point, cell } in content.display_iter {
            cells.push(IndexedCell {
                point,
                c: cell.c,
                fg: cell.fg,
                bg: cell.bg,
                flags: cell.flags,
            });
        }

        let cursor = CursorState {
            point: content.cursor.point,
            shape: content.cursor.shape,
        };

        TerminalContent {
            cells,
            cursor,
            mode: content.mode,
            display_offset: content.display_offset,
            selection: content.selection,
            cols,
            rows,
            damage,
        }
    }

    /// Drain pending events from the terminal.
    pub fn drain_events(&self) -> Vec<TerminalEvent> {
        let mut events = Vec::new();
        while let Ok(event) = self.event_rx.try_recv() {
            events.push(event);
        }
        events
    }

    /// Get the current terminal size.
    pub fn size(&self) -> TerminalSize {
        self.size
    }

    /// Scroll the terminal display by a delta (positive = scroll up into history).
    pub fn scroll_display(&self, scroll: Scroll) {
        self.term.lock().scroll_display(scroll);
    }

    /// Get the selected text as a string, if any selection is active.
    pub fn selection_to_string(&self) -> Option<String> {
        self.term.lock().selection_to_string()
    }
}

impl Drop for CruxTerminal {
    fn drop(&mut self) {
        // Graceful shutdown: SIGHUP first, then SIGKILL if needed.
        //
        // 1. Send SIGHUP to let the shell clean up (save history, etc.)
        // 2. Wait briefly for graceful exit
        // 3. Force SIGKILL only if the child refuses to exit
        if let Some(pid) = self.child.process_id() {
            unsafe {
                libc::kill(pid as i32, libc::SIGHUP);
            }

            // Give the child up to 500ms to exit gracefully.
            for _ in 0..10 {
                if let Ok(Some(_)) = self.child.try_wait() {
                    if let Some(thread) = self.reader_thread.take() {
                        let _ = thread.join();
                    }
                    return;
                }
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
        }

        // Force kill if still running.
        if let Err(e) = self.child.kill() {
            log::debug!("failed to kill child process: {}", e);
        }

        // Reap to prevent zombies.
        if let Err(e) = self.child.wait() {
            log::debug!("failed to wait for child process: {}", e);
        }

        // Join the reader thread.
        if let Some(thread) = self.reader_thread.take() {
            if thread.join().is_err() {
                log::debug!("reader thread panicked during join");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_terminal_size_default() {
        let size = TerminalSize::default();
        assert_eq!(size.rows, 24);
        assert_eq!(size.cols, 80);
    }

    #[test]
    fn test_terminal_size_dimensions() {
        let size = TerminalSize {
            rows: 40,
            cols: 120,
            cell_width: 8.0,
            cell_height: 16.0,
        };
        assert_eq!(size.columns(), 120);
        assert_eq!(size.screen_lines(), 40);
        assert_eq!(size.total_lines(), 40 + SCROLLBACK_LINES);
    }
}
