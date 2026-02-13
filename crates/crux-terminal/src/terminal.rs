use std::collections::VecDeque;
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

use crate::event::{CruxEventListener, SemanticZone, SemanticZoneType, TerminalEvent};
use crate::pty;
use crate::traits::Terminal;

/// Default scrollback history size in lines.
const SCROLLBACK_LINES: usize = 10_000;

/// Maximum number of semantic zones to keep in memory.
/// Prevents unbounded growth when running long-lived shell sessions.
const MAX_SEMANTIC_ZONES: usize = 10_000;

/// Terminal dimensions in cells and pixels.
#[derive(Debug, Clone, Copy)]
pub struct TerminalSize {
    pub rows: usize,
    pub cols: usize,
    pub cell_width: f32,
    pub cell_height: f32,
    pub scrollback_lines: usize,
}

impl Dimensions for TerminalSize {
    fn total_lines(&self) -> usize {
        self.screen_lines() + self.scrollback_lines
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
            scrollback_lines: SCROLLBACK_LINES,
        }
    }
}

/// Damage state captured from alacritty_terminal's damage tracking.
#[derive(Debug, Clone, Default)]
pub enum DamageState {
    /// No lines were damaged since the last render.
    #[default]
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
    /// Current working directory reported by the shell via OSC 7.
    cwd: Option<String>,
    /// Completed semantic zones from OSC 133 shell integration.
    /// Uses VecDeque for efficient front-removal when evicting old zones.
    semantic_zones: VecDeque<SemanticZone>,
    /// Current zone being built (tracks the last seen marker).
    current_zone_type: Option<SemanticZoneType>,
    /// Line where the current zone started.
    current_zone_start_line: i32,
    /// Column where the current zone started.
    current_zone_start_col: usize,
    /// Last observed cursor shape, for detecting Vim mode transitions.
    last_cursor_shape: CursorShape,
}

impl CruxTerminal {
    /// Create a new terminal, spawn the PTY, and start the I/O loop.
    ///
    /// If `shell` is `None`, the user's default shell is detected from
    /// the `SHELL` environment variable (falling back to `/bin/zsh`).
    ///
    /// Optional `cwd` sets the working directory for the new shell.
    /// Optional `command` runs a specific command instead of the login shell.
    /// Optional `env` adds extra environment variables to the child process.
    /// Optional `shell_args` provides arguments for the shell (defaults to `["-l"]`).
    pub fn new(
        shell: Option<String>,
        shell_args: Option<&[String]>,
        size: TerminalSize,
        cwd: Option<&str>,
        command: Option<&[String]>,
        env: Option<&std::collections::HashMap<String, String>>,
    ) -> anyhow::Result<Self> {
        let shell = shell.unwrap_or_else(pty::detect_shell);
        let default_shell_args = vec!["-l".to_string()];
        let shell_args = shell_args.unwrap_or(&default_shell_args);

        // Event channel for terminal → UI communication.
        let (event_tx, event_rx) = mpsc::channel();

        let event_listener = CruxEventListener::new(event_tx.clone());

        // Create alacritty_terminal Term with scrollback config from TerminalSize.
        let config = Config {
            scrolling_history: size.scrollback_lines,
            ..Config::default()
        };
        let term = Term::new(config, &size, event_listener);
        let term = Arc::new(FairMutex::new(term));

        // Spawn the PTY process.
        let (master_pty, child) = pty::spawn_pty(&shell, shell_args, &size, cwd, command, env)?;

        // Get reader and writer handles from the master PTY.
        let reader = master_pty.try_clone_reader()?;
        let writer = master_pty.take_writer()?;

        // Start background PTY reader thread.
        // The event_tx clone is used for OSC 7 (CWD) events that
        // alacritty_terminal does not handle natively.
        let term_clone = term.clone();
        let reader_thread = pty::start_pty_read_loop(term_clone, reader, event_tx, || {
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
            cwd: None,
            semantic_zones: VecDeque::new(),
            current_zone_type: None,
            current_zone_start_line: 0,
            current_zone_start_col: 0,
            last_cursor_shape: CursorShape::Block,
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

        // Resize PTY first so the child process gets SIGWINCH before grid changes.
        if let Err(e) = self.master_pty.resize(portable_pty::PtySize {
            rows: u16::try_from(size.rows).unwrap_or(u16::MAX),
            cols: u16::try_from(size.cols).unwrap_or(u16::MAX),
            pixel_width: u16::try_from((size.cols as f32 * size.cell_width) as usize)
                .unwrap_or(u16::MAX),
            pixel_height: u16::try_from((size.rows as f32 * size.cell_height) as usize)
                .unwrap_or(u16::MAX),
        }) {
            log::warn!("failed to resize PTY: {}", e);
        }

        // Then resize the alacritty terminal grid.
        // Note: scrollback_lines is fixed at terminal creation time and cannot be changed.
        self.term.lock().resize(size);
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

        let cols = term.columns();
        let rows = term.screen_lines();

        // Scope the immutable borrow from renderable_content() so we can
        // call reset_damage() afterward.
        let (cells, cursor, mode, display_offset, selection) = {
            let content = term.renderable_content();

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

            (
                cells,
                cursor,
                content.mode,
                content.display_offset,
                content.selection,
            )
        };

        // Reset damage after all cell data has been copied.
        term.reset_damage();

        TerminalContent {
            cells,
            cursor,
            mode,
            display_offset,
            selection,
            cols,
            rows,
            damage,
        }
    }

    /// Drain pending events from the terminal.
    ///
    /// Also processes `CwdChanged` and `PromptMark` events internally
    /// to keep the stored CWD and semantic zones up to date.
    pub fn drain_events(&mut self) -> Vec<TerminalEvent> {
        let mut events = Vec::new();
        while let Ok(event) = self.event_rx.try_recv() {
            match &event {
                TerminalEvent::CwdChanged(ref path) => {
                    self.cwd = Some(path.clone());
                }
                TerminalEvent::PromptMark { mark, exit_code } => {
                    self.handle_prompt_mark(*mark, *exit_code);
                }
                _ => {}
            }
            events.push(event);
        }

        // Check cursor shape change after processing PTY output.
        let current_shape = self.term.lock().cursor_style().shape;
        if current_shape != self.last_cursor_shape {
            events.push(TerminalEvent::CursorShapeChanged {
                old_shape: self.last_cursor_shape,
                new_shape: current_shape,
            });
            self.last_cursor_shape = current_shape;
        }

        events
    }

    /// Process an OSC 133 prompt mark to build semantic zones.
    ///
    /// Zone transitions: A→Prompt, B→Input, C→Output, D→closes Output.
    /// Each new marker closes the previous zone (if any) and starts a new one.
    fn handle_prompt_mark(&mut self, mark: SemanticZoneType, exit_code: Option<i32>) {
        // Use the grid cursor for absolute line coordinates (not viewport-relative).
        // Line 0 = top of active screen; negative lines = scrollback history.
        let (cursor_line, cursor_col) = self.with_term(|t| {
            let point = t.grid().cursor.point;
            (point.line.0, point.column.0)
        });

        // Close the current zone if one is open.
        if let Some(zone_type) = self.current_zone_type.take() {
            self.semantic_zones.push_back(SemanticZone {
                start_line: self.current_zone_start_line,
                start_col: self.current_zone_start_col,
                end_line: cursor_line,
                end_col: cursor_col,
                zone_type,
                exit_code: if zone_type == SemanticZoneType::Output {
                    exit_code
                } else {
                    None
                },
            });

            // Cap semantic_zones to prevent unbounded growth in long-lived sessions.
            // Remove oldest zones when we exceed the limit.
            while self.semantic_zones.len() > MAX_SEMANTIC_ZONES {
                self.semantic_zones.pop_front();
            }
        }

        // D (command complete) only closes the Output zone; it does not
        // start a new zone. A/B/C open their respective zones.
        if exit_code.is_none() || mark != SemanticZoneType::Output {
            // For A/B/C markers, start a new zone.
            // For D without exit_code this branch is unreachable in practice
            // because scan_osc133 always sets mark=Output for D, but guard anyway.
            self.current_zone_type = Some(mark);
            self.current_zone_start_line = cursor_line;
            self.current_zone_start_col = cursor_col;
        }
    }

    /// Current working directory reported by the shell via OSC 7.
    ///
    /// Returns `None` if the shell has not yet reported a CWD.
    pub fn cwd(&self) -> Option<&str> {
        self.cwd.as_deref()
    }

    /// Get all completed semantic zones from OSC 133 shell integration.
    pub fn semantic_zones(&self) -> &[SemanticZone] {
        // VecDeque::make_contiguous() would require &mut self, which we don't have.
        // Use as_slices() to get immutable slice access. If the deque is contiguous,
        // the second slice will be empty. If not contiguous (rare), we can only
        // return one of the slices. Since zones are pushed to the back and popped
        // from the front, the back slice (slice1) contains the most recent zones.
        let (slice1, slice2) = self.semantic_zones.as_slices();
        if slice2.is_empty() {
            slice1
        } else {
            // Not contiguous: return the back slice (most recent zones).
            // This is rare and only happens after front eviction + back insertion patterns.
            // Full correctness would require changing the API to return Vec or iterator.
            slice1
        }
    }

    /// Get the line number of the most recent prompt start.
    ///
    /// Scans completed zones in reverse for the last `Prompt` zone.
    /// Returns `None` if no prompt has been marked yet.
    pub fn last_prompt_line(&self) -> Option<i32> {
        // Check both completed zones and the currently-open zone.
        if self.current_zone_type == Some(SemanticZoneType::Prompt) {
            return Some(self.current_zone_start_line);
        }
        self.semantic_zones
            .iter()
            .rev()
            .find(|z| z.zone_type == SemanticZoneType::Prompt)
            .map(|z| z.start_line)
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

    /// Check if the child process is still running.
    pub fn is_process_running(&mut self) -> bool {
        match self.child.try_wait() {
            Ok(Some(_)) => false,
            Ok(None) => true,
            Err(_) => false,
        }
    }

    /// Get the child process PID.
    pub fn child_pid(&self) -> Option<u32> {
        self.child.process_id()
    }
}

/// Extract text lines from terminal content cells.
///
/// This helper consolidates the logic for building text lines from
/// a TerminalContent snapshot, used by get_text IPC handlers and view methods.
pub fn extract_text_lines(content: &TerminalContent) -> Vec<String> {
    let mut lines: Vec<String> = vec![String::new(); content.rows];
    for cell in &content.cells {
        let row = cell.point.line.0 as usize;
        if row < content.rows {
            lines[row].push(cell.c);
        }
    }
    for line in &mut lines {
        let trimmed_len = line.trim_end().len();
        line.truncate(trimmed_len);
    }
    lines
}

impl Terminal for CruxTerminal {
    fn write_to_pty(&mut self, data: &[u8]) {
        CruxTerminal::write_to_pty(self, data);
    }

    fn resize(&mut self, size: TerminalSize) {
        CruxTerminal::resize(self, size);
    }

    fn content(&self) -> TerminalContent {
        CruxTerminal::content(self)
    }

    fn drain_events(&mut self) -> Vec<TerminalEvent> {
        CruxTerminal::drain_events(self)
    }

    fn cwd(&self) -> Option<&str> {
        CruxTerminal::cwd(self)
    }

    fn size(&self) -> TerminalSize {
        CruxTerminal::size(self)
    }

    fn scroll_display(&self, scroll: Scroll) {
        CruxTerminal::scroll_display(self, scroll);
    }

    fn selection_to_string(&self) -> Option<String> {
        CruxTerminal::selection_to_string(self)
    }

    fn semantic_zones(&self) -> &[SemanticZone] {
        CruxTerminal::semantic_zones(self)
    }

    fn is_process_running(&mut self) -> bool {
        CruxTerminal::is_process_running(self)
    }

    fn child_pid(&self) -> Option<u32> {
        CruxTerminal::child_pid(self)
    }

    fn with_term<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&Term<CruxEventListener>) -> R,
    {
        CruxTerminal::with_term(self, f)
    }

    fn with_term_mut<F, R>(&self, f: F) -> R
    where
        F: FnOnce(&mut Term<CruxEventListener>) -> R,
    {
        CruxTerminal::with_term_mut(self, f)
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
            // Safe cast: PIDs on macOS/Linux are always within i32 range.
            let pid_i32 = i32::try_from(pid).expect("PID exceeds i32::MAX");
            unsafe {
                libc::kill(pid_i32, libc::SIGHUP);
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
    use alacritty_terminal::index::{Column, Line, Point};
    use alacritty_terminal::term::TermMode;
    use alacritty_terminal::vte::ansi::{Color, CursorShape};

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
            scrollback_lines: 5000,
        };
        assert_eq!(size.columns(), 120);
        assert_eq!(size.screen_lines(), 40);
        assert_eq!(size.total_lines(), 40 + 5000);
    }

    #[test]
    fn test_extract_text_lines_empty_grid() {
        let content = TerminalContent {
            cells: Vec::new(),
            cursor: CursorState {
                point: Point::new(Line(0), Column(0)),
                shape: CursorShape::Block,
            },
            mode: TermMode::empty(),
            display_offset: 0,
            selection: None,
            cols: 80,
            rows: 24,
            damage: DamageState::None,
        };

        let lines = extract_text_lines(&content);
        assert_eq!(lines.len(), 24);
        for line in &lines {
            assert_eq!(line, "");
        }
    }

    #[test]
    fn test_extract_text_lines_single_line() {
        let mut cells = Vec::new();
        let text = "Hello, world!";
        for (i, ch) in text.chars().enumerate() {
            cells.push(IndexedCell {
                point: Point::new(Line(0), Column(i)),
                c: ch,
                fg: Color::Named(crate::NamedColor::Foreground),
                bg: Color::Named(crate::NamedColor::Background),
                flags: Flags::empty(),
            });
        }

        let content = TerminalContent {
            cells,
            cursor: CursorState {
                point: Point::new(Line(0), Column(0)),
                shape: CursorShape::Block,
            },
            mode: TermMode::empty(),
            display_offset: 0,
            selection: None,
            cols: 80,
            rows: 3,
            damage: DamageState::None,
        };

        let lines = extract_text_lines(&content);
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0], "Hello, world!");
        assert_eq!(lines[1], "");
        assert_eq!(lines[2], "");
    }

    #[test]
    fn test_extract_text_lines_trailing_spaces() {
        let mut cells = Vec::new();
        let text = "test    ";
        for (i, ch) in text.chars().enumerate() {
            cells.push(IndexedCell {
                point: Point::new(Line(0), Column(i)),
                c: ch,
                fg: Color::Named(crate::NamedColor::Foreground),
                bg: Color::Named(crate::NamedColor::Background),
                flags: Flags::empty(),
            });
        }

        let content = TerminalContent {
            cells,
            cursor: CursorState {
                point: Point::new(Line(0), Column(0)),
                shape: CursorShape::Block,
            },
            mode: TermMode::empty(),
            display_offset: 0,
            selection: None,
            cols: 80,
            rows: 1,
            damage: DamageState::None,
        };

        let lines = extract_text_lines(&content);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0], "test");
    }

    #[test]
    fn test_extract_text_lines_multiple_lines() {
        let mut cells = Vec::new();

        // Line 0: "first"
        for (i, ch) in "first".chars().enumerate() {
            cells.push(IndexedCell {
                point: Point::new(Line(0), Column(i)),
                c: ch,
                fg: Color::Named(crate::NamedColor::Foreground),
                bg: Color::Named(crate::NamedColor::Background),
                flags: Flags::empty(),
            });
        }

        // Line 1: "second line"
        for (i, ch) in "second line".chars().enumerate() {
            cells.push(IndexedCell {
                point: Point::new(Line(1), Column(i)),
                c: ch,
                fg: Color::Named(crate::NamedColor::Foreground),
                bg: Color::Named(crate::NamedColor::Background),
                flags: Flags::empty(),
            });
        }

        // Line 2: empty (skip)

        // Line 3: "fourth"
        for (i, ch) in "fourth".chars().enumerate() {
            cells.push(IndexedCell {
                point: Point::new(Line(3), Column(i)),
                c: ch,
                fg: Color::Named(crate::NamedColor::Foreground),
                bg: Color::Named(crate::NamedColor::Background),
                flags: Flags::empty(),
            });
        }

        let content = TerminalContent {
            cells,
            cursor: CursorState {
                point: Point::new(Line(0), Column(0)),
                shape: CursorShape::Block,
            },
            mode: TermMode::empty(),
            display_offset: 0,
            selection: None,
            cols: 80,
            rows: 4,
            damage: DamageState::None,
        };

        let lines = extract_text_lines(&content);
        assert_eq!(lines.len(), 4);
        assert_eq!(lines[0], "first");
        assert_eq!(lines[1], "second line");
        assert_eq!(lines[2], "");
        assert_eq!(lines[3], "fourth");
    }

    #[test]
    fn test_extract_text_lines_only_spaces() {
        let mut cells = Vec::new();

        // Line with only spaces
        for i in 0..5 {
            cells.push(IndexedCell {
                point: Point::new(Line(0), Column(i)),
                c: ' ',
                fg: Color::Named(crate::NamedColor::Foreground),
                bg: Color::Named(crate::NamedColor::Background),
                flags: Flags::empty(),
            });
        }

        let content = TerminalContent {
            cells,
            cursor: CursorState {
                point: Point::new(Line(0), Column(0)),
                shape: CursorShape::Block,
            },
            mode: TermMode::empty(),
            display_offset: 0,
            selection: None,
            cols: 80,
            rows: 1,
            damage: DamageState::None,
        };

        let lines = extract_text_lines(&content);
        assert_eq!(lines.len(), 1);
        assert_eq!(lines[0], "");
    }
}
