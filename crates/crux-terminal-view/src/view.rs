//! CruxTerminalView: GPUI View that owns a CruxTerminal and handles I/O.

use std::ops::Range;
use std::time::{Duration, Instant};

use gpui::*;
use unicode_normalization::UnicodeNormalization;

use crux_terminal::{
    Column, CruxTerminal, DamageState, Dimensions, Line, Point, Scroll, Selection, SelectionType,
    Side, TermMode, TerminalContent, TerminalEvent, TerminalSize,
};

use crate::element::render_terminal_canvas;
use crate::input;
use crate::input::OptionAsAlt;
use crate::mouse;

const FONT_FAMILY: &str = "Menlo";
const FONT_SIZE: f32 = 14.0;

/// Duration for bell visual flash.
const BELL_FLASH_DURATION: Duration = Duration::from_millis(150);

/// Lines to scroll per mouse wheel tick.
const SCROLL_LINES_PER_TICK: i32 = 3;

/// Window for IME event deduplication. Identical insertText: calls within
/// this interval are treated as duplicates (prevents the double-space bug
/// observed in some CJK input methods).
const IME_DEDUP_WINDOW: Duration = Duration::from_millis(10);

/// GPUI View wrapping a terminal emulator with keyboard input and rendering.
pub struct CruxTerminalView {
    terminal: CruxTerminal,
    focus_handle: FocusHandle,
    font: Font,
    font_size: Pixels,
    cell_width: Pixels,
    cell_height: Pixels,
    /// Origin of the terminal canvas in window coordinates, updated each render.
    canvas_origin: Point2D<Pixels>,
    /// The last title reported by the terminal via OSC.
    title: Option<String>,
    /// Instant when the bell last fired; used for visual flash.
    bell_at: Option<Instant>,
    /// Whether cell dimensions have been measured (cached after first layout).
    cell_measured: bool,
    /// Whether the terminal has new content to render.
    dirty: bool,
    /// Whether the cursor is currently visible in the blink cycle.
    cursor_blink_visible: bool,
    /// When the cursor blink last reset (user input or click).
    cursor_blink_epoch: Instant,
    /// Interval for cursor blink on/off cycles.
    cursor_blink_interval: Duration,
    /// Whether the terminal view is currently focused.
    is_focused: bool,
    /// Whether the macOS Option key should be treated as Alt.
    option_as_alt: OptionAsAlt,
    /// Last reported mouse grid position, for motion event deduplication.
    last_mouse_grid: Option<Point>,
    /// IME composition (preedit) text, displayed as overlay at cursor position.
    /// Set by `replace_and_mark_text_in_range`, cleared on commit or `unmark_text`.
    marked_text: Option<String>,
    /// Selected range within composition text (UTF-16 offsets from IME).
    marked_text_selected_range: Option<Range<usize>>,
    /// Last IME commit for deduplication (text, timestamp). Prevents double-space bug (Alacritty #8079).
    last_ime_commit: Option<(String, Instant)>,
}

/// Alias for GPUI's 2D point to avoid confusion with alacritty's grid Point.
type Point2D<T> = gpui::Point<T>;

impl CruxTerminalView {
    /// Returns the current terminal title (set by OSC escape sequence), if any.
    pub fn title(&self) -> Option<&str> {
        self.title.as_deref()
    }

    /// Returns the current working directory reported by the shell (OSC 7), if any.
    pub fn cwd(&self) -> Option<&str> {
        self.terminal.cwd()
    }

    pub fn new(cx: &mut Context<Self>) -> Self {
        Self::new_with_options(None, None, None, cx)
    }

    /// Create a new terminal view with optional cwd, command, and env.
    pub fn new_with_options(
        cwd: Option<&str>,
        command: Option<&[String]>,
        env: Option<&std::collections::HashMap<String, String>>,
        cx: &mut Context<Self>,
    ) -> Self {
        let focus_handle = cx.focus_handle();

        let terminal_font = font(FONT_FAMILY);
        let font_size = px(FONT_SIZE);

        // Default cell metrics; will be recalculated on first layout.
        let cell_width = px(8.4);
        let cell_height = px(17.0);

        let size = TerminalSize {
            rows: 24,
            cols: 80,
            cell_width: f32::from(cell_width),
            cell_height: f32::from(cell_height),
        };

        let terminal =
            CruxTerminal::new(None, size, cwd, command, env).expect("failed to create terminal");

        // Periodic refresh at ~60fps to pick up PTY output and handle cursor blink.
        cx.spawn(async |this: WeakEntity<Self>, cx: &mut AsyncApp| loop {
            cx.background_executor()
                .timer(Duration::from_millis(16))
                .await;
            let ok = this.update(cx, |this: &mut Self, cx: &mut Context<Self>| {
                // Only notify if there's actual work: dirty content, active bell, or cursor blinking.
                if this.dirty || this.is_bell_active() || this.should_notify_for_blink() {
                    cx.notify();
                }
            });
            if ok.is_err() {
                break;
            }
        })
        .detach();

        Self {
            terminal,
            focus_handle,
            font: terminal_font,
            font_size,
            cell_width,
            cell_height,
            canvas_origin: gpui::point(px(0.0), px(0.0)),
            title: None,
            bell_at: None,
            cell_measured: false,
            dirty: false,
            cursor_blink_visible: true,
            cursor_blink_epoch: Instant::now(),
            cursor_blink_interval: Duration::from_millis(500),
            is_focused: false,
            option_as_alt: OptionAsAlt::Both,
            last_mouse_grid: None,
            marked_text: None,
            marked_text_selected_range: None,
            last_ime_commit: None,
        }
    }

    /// Measure cell dimensions using the text system. Cached after first successful measurement.
    fn measure_cell(&mut self, window: &mut Window) {
        if self.cell_measured {
            return;
        }
        let text_system = window.text_system();
        let run = TextRun {
            len: 1,
            font: self.font.clone(),
            color: Hsla::white(),
            background_color: None,
            underline: None,
            strikethrough: None,
        };
        let shaped = text_system.shape_line(SharedString::from("M"), self.font_size, &[run], None);
        let w = shaped.width;
        if w > px(0.0) {
            self.cell_width = w;
        }
        // Line height: font_size * ~1.2 for comfortable terminal spacing.
        self.cell_height = self.font_size + px(3.0);
        self.cell_measured = true;
    }

    /// Handle terminal resize when the view bounds change.
    fn resize_if_needed(&mut self, bounds_size: Size<Pixels>) {
        let cols = (f32::from(bounds_size.width) / f32::from(self.cell_width)).floor() as usize;
        let rows = (f32::from(bounds_size.height) / f32::from(self.cell_height)).floor() as usize;

        if cols == 0 || rows == 0 {
            return;
        }

        let current = self.terminal.size();
        if current.cols != cols || current.rows != rows {
            self.terminal.resize(TerminalSize {
                rows,
                cols,
                cell_width: f32::from(self.cell_width),
                cell_height: f32::from(self.cell_height),
            });
        }
    }

    fn handle_key_down(
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
            if last_time.elapsed() < IME_DEDUP_WINDOW && event.keystroke.key == last_text.as_str() {
                // This keystroke is a duplicate of the IME commit. Drop it.
                cx.stop_propagation();
                return;
            }
        }

        // Character keys without special modifiers → let IME handle via
        // replace_text_in_range(). This avoids double-processing: if we wrote
        // to the PTY here, the IME would also write via insertText:.
        if Self::is_ime_candidate(&event.keystroke, self.option_as_alt) {
            return; // Don't stop propagation — let event reach IME.
        }

        // Get the current terminal mode for application cursor key detection.
        let mode = self.terminal.content().mode;

        if let Some(bytes) = input::keystroke_to_bytes(&event.keystroke, mode, self.option_as_alt) {
            // Clear selection when typing.
            self.terminal.with_term_mut(|term| {
                term.selection = None;
            });
            self.terminal.write_to_pty(&bytes);
            cx.stop_propagation();
            cx.notify();
        }
    }

    /// Returns true if the keystroke is a standalone modifier key (no character).
    /// Used to prevent modifiers from destroying IME composition (Ghostty #4634).
    fn is_standalone_modifier(keystroke: &Keystroke) -> bool {
        matches!(
            keystroke.key.as_str(),
            "shift" | "control" | "alt" | "cmd" | "option" | "command"
        )
    }

    /// Returns true if the keystroke should be handled by IME rather than directly.
    ///
    /// Character keys without Ctrl/Alt/Cmd/Fn modifiers go through the IME pipeline
    /// so that composition (e.g. Korean jamo assembly) works correctly.
    fn is_ime_candidate(keystroke: &Keystroke, option_as_alt: OptionAsAlt) -> bool {
        if keystroke.modifiers.platform
            || keystroke.modifiers.control
            || keystroke.modifiers.function
        {
            return false;
        }
        // Alt+key sends ESC prefix when option_as_alt is enabled — bypass IME.
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

    /// Convert pixel position relative to canvas origin into terminal grid coordinates.
    fn pixel_to_grid(&self, position: Point2D<Pixels>) -> Point {
        let col = ((f32::from(position.x) - f32::from(self.canvas_origin.x))
            / f32::from(self.cell_width)) as usize;
        let row = ((f32::from(position.y) - f32::from(self.canvas_origin.y))
            / f32::from(self.cell_height)) as usize;
        let size = self.terminal.size();
        let col = col.min(size.cols.saturating_sub(1));
        let row = row.min(size.rows.saturating_sub(1));
        Point::new(Line(row as i32), Column(col))
    }

    /// Determine which side of a cell the cursor is on (for selection precision).
    fn pixel_to_side(&self, position: Point2D<Pixels>) -> Side {
        let col_frac = ((f32::from(position.x) - f32::from(self.canvas_origin.x))
            / f32::from(self.cell_width))
            % 1.0;
        if col_frac < 0.5 {
            Side::Left
        } else {
            Side::Right
        }
    }

    fn handle_mouse_down(
        &mut self,
        event: &MouseDownEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.focus_handle.focus(window);
        self.reset_cursor_blink();

        let grid_point = self.pixel_to_grid(event.position);
        let mode = self.terminal.content().mode;

        // If mouse mode is active and Shift is not held, report to PTY.
        if mouse::mouse_mode_active(mode, event.modifiers.shift) {
            let cb = mouse::mouse_button_to_cb(event.button, false)
                + mouse::modifier_bits(&event.modifiers);
            let report = mouse::sgr_mouse_report(cb, grid_point, true);
            self.terminal.write_to_pty(&report);
            self.last_mouse_grid = Some(grid_point);
            cx.notify();
            return;
        }

        // Normal selection handling.
        let side = self.pixel_to_side(event.position);
        let display_offset = self.terminal.content().display_offset;
        let abs_point = Point::new(
            Line(grid_point.line.0 - display_offset as i32),
            grid_point.column,
        );

        let selection_type = match event.click_count {
            2 => SelectionType::Semantic,
            3 => SelectionType::Lines,
            _ => SelectionType::Simple,
        };

        self.terminal.with_term_mut(|term| {
            term.selection = Some(Selection::new(selection_type, abs_point, side));
        });

        cx.notify();
    }

    fn handle_mouse_move(
        &mut self,
        event: &MouseMoveEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let grid_point = self.pixel_to_grid(event.position);
        let mode = self.terminal.content().mode;

        // Mouse mode reporting for motion events.
        if mouse::mouse_mode_active(mode, false) {
            let has_button = event.pressed_button.is_some();

            // Mode 1002 (MOUSE_DRAG): report motion only when button pressed + cell changed.
            // Mode 1003 (MOUSE_MOTION): report all motion when cell changed.
            let should_report = if mode.contains(TermMode::MOUSE_MOTION) {
                // Any-event tracking: report all motion when cell changes.
                true
            } else if mode.contains(TermMode::MOUSE_DRAG) && has_button {
                // Button-event tracking: report only when button is held.
                true
            } else {
                false
            };

            if should_report {
                // Deduplicate: only report when the cell actually changed.
                if self.last_mouse_grid != Some(grid_point) {
                    let button = event.pressed_button.unwrap_or(MouseButton::Left);
                    let cb = mouse::mouse_button_to_cb(button, true)
                        + mouse::modifier_bits(&event.modifiers);
                    let report = mouse::sgr_mouse_report(cb, grid_point, true);
                    self.terminal.write_to_pty(&report);
                    self.last_mouse_grid = Some(grid_point);
                    cx.notify();
                }
            }
            return;
        }

        // Normal selection dragging.
        if event.pressed_button != Some(MouseButton::Left) {
            return;
        }

        let side = self.pixel_to_side(event.position);
        let display_offset = self.terminal.content().display_offset;
        let abs_point = Point::new(
            Line(grid_point.line.0 - display_offset as i32),
            grid_point.column,
        );

        self.terminal.with_term_mut(|term| {
            if let Some(ref mut selection) = term.selection {
                selection.update(abs_point, side);
            }
        });

        cx.notify();
    }

    fn handle_mouse_up(
        &mut self,
        event: &MouseUpEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let mode = self.terminal.content().mode;

        // If mouse mode is active, report the release to PTY.
        if mouse::mouse_mode_active(mode, event.modifiers.shift) {
            let grid_point = self.pixel_to_grid(event.position);
            let cb = mouse::mouse_button_to_cb(event.button, false)
                + mouse::modifier_bits(&event.modifiers);
            let report = mouse::sgr_mouse_report(cb, grid_point, false);
            self.terminal.write_to_pty(&report);
            self.last_mouse_grid = None;
            cx.notify();
        }
        // Normal mode: selection is finalized, no action needed.
    }

    fn handle_scroll_wheel(
        &mut self,
        event: &ScrollWheelEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let mode = self.terminal.content().mode;

        // If mouse mode is active and Shift is not held, report scroll to PTY.
        if mouse::mouse_mode_active(mode, event.modifiers.shift) {
            let grid_point = self.pixel_to_grid(event.position);
            let lines = match event.delta {
                ScrollDelta::Lines(l) => l.y.abs().max(1.0) as usize,
                ScrollDelta::Pixels(p) => {
                    (f32::from(p.y).abs() / f32::from(self.cell_height)).max(1.0) as usize
                }
            };
            let up = match event.delta {
                ScrollDelta::Lines(l) => l.y < 0.0,
                ScrollDelta::Pixels(p) => f32::from(p.y) < 0.0,
            };
            let cb = mouse::scroll_button(up) + mouse::modifier_bits(&event.modifiers);
            // Send one report per scroll line (standard behavior).
            for _ in 0..lines {
                let report = mouse::sgr_mouse_report(cb, grid_point, true);
                self.terminal.write_to_pty(&report);
            }
            cx.notify();
            return;
        }

        // Alternate screen + ALTERNATE_SCROLL: convert scroll to cursor keys.
        if mode.contains(TermMode::ALT_SCREEN) && mode.contains(TermMode::ALTERNATE_SCROLL) {
            let lines = match event.delta {
                ScrollDelta::Lines(l) => l.y.abs().max(1.0) as usize,
                ScrollDelta::Pixels(p) => {
                    (f32::from(p.y).abs() / f32::from(self.cell_height)).max(1.0) as usize
                }
            };
            let up = match event.delta {
                ScrollDelta::Lines(l) => l.y < 0.0,
                ScrollDelta::Pixels(p) => f32::from(p.y) < 0.0,
            };
            let key = if up { b"\x1bOA" } else { b"\x1bOB" };
            for _ in 0..lines {
                self.terminal.write_to_pty(key);
            }
            cx.notify();
            return;
        }

        // Normal scrollback.
        let delta = match event.delta {
            ScrollDelta::Lines(lines) => -(lines.y * SCROLL_LINES_PER_TICK as f32) as i32,
            ScrollDelta::Pixels(pixels) => {
                let lines = f32::from(pixels.y) / f32::from(self.cell_height);
                -lines as i32
            }
        };

        if delta != 0 {
            self.terminal.scroll_display(Scroll::Delta(delta));
            cx.notify();
        }
    }

    /// Copy the current selection to the system clipboard.
    fn copy_selection(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = self.terminal.selection_to_string() {
            cx.write_to_clipboard(ClipboardItem::new_string(text));
        }
    }

    /// Paste text from the system clipboard into the terminal.
    fn paste_from_clipboard(&mut self, cx: &mut Context<Self>) {
        if let Some(item) = cx.read_from_clipboard() {
            if let Some(text) = item.text() {
                if !text.is_empty() {
                    // Use bracketed paste mode if terminal supports it.
                    let mode = self.terminal.content().mode;
                    if mode.contains(TermMode::BRACKETED_PASTE) {
                        self.terminal.write_to_pty(b"\x1b[200~");
                        self.terminal.write_to_pty(text.as_bytes());
                        self.terminal.write_to_pty(b"\x1b[201~");
                    } else {
                        self.terminal.write_to_pty(text.as_bytes());
                    }
                }
            }
        }
    }

    /// Process pending terminal events.
    fn process_events(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let mut had_events = false;
        for event in self.terminal.drain_events() {
            had_events = true;
            match event {
                TerminalEvent::PtyWrite(text) => {
                    self.terminal.write_to_pty(text.as_bytes());
                }
                TerminalEvent::Title(title) => {
                    window.set_window_title(&title);
                    self.title = Some(title);
                }
                TerminalEvent::Bell => {
                    // Rate limit: ignore bells while a flash is already active.
                    if !self.is_bell_active() {
                        self.bell_at = Some(Instant::now());
                    }
                }
                TerminalEvent::ProcessExit(code) => {
                    log::info!("child process exited with code {}", code);
                    cx.quit();
                }
                TerminalEvent::Wakeup => {}
                TerminalEvent::CwdChanged(_) => {
                    // CWD is stored internally by CruxTerminal::drain_events().
                    // The view layer can read it via terminal.cwd() when needed.
                }
                TerminalEvent::PromptMark { .. } => {
                    // Prompt marks are stored internally by CruxTerminal::drain_events().
                    // The view layer does not need to handle them.
                }
            }
        }
        // Mark dirty if we received any events.
        if had_events {
            self.dirty = true;
        }
    }

    /// Returns true if the bell flash is currently active.
    fn is_bell_active(&self) -> bool {
        self.bell_at
            .is_some_and(|t| t.elapsed() < BELL_FLASH_DURATION)
    }

    /// Reset cursor blink to visible state (called on user input or click).
    fn reset_cursor_blink(&mut self) {
        self.cursor_blink_visible = true;
        self.cursor_blink_epoch = Instant::now();
    }

    /// Select all terminal content (screen + scrollback).
    fn select_all(&mut self) {
        self.terminal.with_term_mut(|term| {
            let start = Point::new(term.grid().topmost_line(), Column(0));
            let end = Point::new(
                term.grid().bottommost_line(),
                Column(term.grid().columns() - 1),
            );
            let mut sel = Selection::new(SelectionType::Lines, start, Side::Left);
            sel.update(end, Side::Right);
            term.selection = Some(sel);
        });
    }

    /// Check if the terminal's child process is still running.
    pub fn is_process_running(&mut self) -> bool {
        self.terminal.is_process_running()
    }

    /// Get the child process PID.
    pub fn child_pid(&self) -> Option<u32> {
        self.terminal.child_pid()
    }

    /// Write data to the terminal's PTY.
    pub fn write_to_pty(&mut self, data: &[u8]) {
        self.terminal.write_to_pty(data);
    }

    /// Get terminal grid content as text lines.
    pub fn get_text_lines(&self) -> Vec<String> {
        let content = self.terminal.content();
        let mut lines: Vec<String> = Vec::with_capacity(content.rows);
        for row in 0..content.rows {
            let mut line = String::new();
            for cell in &content.cells {
                if cell.point.line.0 == row as i32 {
                    line.push(cell.c);
                }
            }
            lines.push(line.trim_end().to_string());
        }
        lines
    }

    /// Get the terminal size.
    pub fn terminal_size(&self) -> TerminalSize {
        self.terminal.size()
    }

    /// Get a snapshot of the terminal content (for cursor position, etc.).
    pub fn terminal_content_snapshot(&self) -> TerminalContent {
        self.terminal.content()
    }

    /// Scroll to the previous prompt (OSC 133 semantic zone).
    pub fn scroll_to_prev_prompt(&mut self) {
        let content = self.terminal.content();
        let current_line = content.cursor.point.line.0 - content.display_offset as i32;

        let zones = self.terminal.semantic_zones();
        // Find the last prompt zone that starts before the current viewport line.
        if let Some(zone) = zones.iter().rev().find(|z| {
            z.zone_type == crux_terminal::SemanticZoneType::Prompt && z.start_line < current_line
        }) {
            let delta = current_line - zone.start_line;
            if delta > 0 {
                self.terminal.scroll_display(Scroll::Delta(delta));
            }
        }
    }

    /// Scroll to the next prompt (OSC 133 semantic zone).
    pub fn scroll_to_next_prompt(&mut self) {
        let content = self.terminal.content();
        let current_line = content.cursor.point.line.0 - content.display_offset as i32;

        let zones = self.terminal.semantic_zones();
        // Find the first prompt zone that starts after the current viewport line.
        if let Some(zone) = zones.iter().find(|z| {
            z.zone_type == crux_terminal::SemanticZoneType::Prompt && z.start_line > current_line
        }) {
            let delta = zone.start_line - current_line;
            if delta > 0 {
                self.terminal.scroll_display(Scroll::Delta(-delta));
            }
        }
    }

    /// Returns true if we should notify GPUI for cursor blink animation.
    fn should_notify_for_blink(&self) -> bool {
        // Only animate cursor blink when focused (unfocused cursor shows as hollow, no animation)
        if !self.is_focused {
            return false;
        }
        // Only notify when blink state would actually change
        self.calculate_cursor_visible() != self.cursor_blink_visible
    }

    /// Calculate whether the cursor should be visible based on blink cycle.
    fn calculate_cursor_visible(&self) -> bool {
        let elapsed = self.cursor_blink_epoch.elapsed();
        let cycle = (elapsed.as_millis() / self.cursor_blink_interval.as_millis()) % 2;
        cycle == 0
    }
}

impl Focusable for CruxTerminalView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

/// Convert a UTF-16 code-unit offset to a UTF-8 byte offset within `s`.
fn utf16_offset_to_utf8(s: &str, utf16_offset: usize) -> usize {
    let mut utf16_count = 0;
    let mut utf8_offset = 0;
    for ch in s.chars() {
        if utf16_count >= utf16_offset {
            break;
        }
        utf16_count += ch.len_utf16();
        utf8_offset += ch.len_utf8();
    }
    utf8_offset
}

impl EntityInputHandler for CruxTerminalView {
    fn text_for_range(
        &mut self,
        range_utf16: Range<usize>,
        adjusted_range: &mut Option<Range<usize>>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<String> {
        let text = self.marked_text.as_ref()?;
        let start = utf16_offset_to_utf8(text, range_utf16.start).min(text.len());
        let end = utf16_offset_to_utf8(text, range_utf16.end).min(text.len());
        *adjusted_range = Some(range_utf16);
        Some(text[start..end].to_string())
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        let range = if let Some(ref sel) = self.marked_text_selected_range {
            sel.clone()
        } else if let Some(ref text) = self.marked_text {
            let utf16_len: usize = text.chars().map(|c| c.len_utf16()).sum();
            utf16_len..utf16_len
        } else {
            0..0
        };
        Some(UTF16Selection {
            range,
            reversed: false,
        })
    }

    fn marked_text_range(
        &self,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Range<usize>> {
        self.marked_text.as_ref().map(|text| {
            let utf16_len: usize = text.chars().map(|c| c.len_utf16()).sum();
            0..utf16_len
        })
    }

    fn unmark_text(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {
        self.marked_text = None;
        self.marked_text_selected_range = None;
    }

    fn replace_text_in_range(
        &mut self,
        _range: Option<Range<usize>>,
        text: &str,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Clear composition state — this is a commit.
        self.marked_text = None;
        self.marked_text_selected_range = None;

        if !text.is_empty() {
            // Dedup: some CJK input methods fire duplicate insertText: calls
            // within a few milliseconds. Suppress exact duplicates.
            let now = Instant::now();
            if let Some((ref prev_text, prev_time)) = self.last_ime_commit {
                if prev_text == text && now.duration_since(prev_time) < IME_DEDUP_WINDOW {
                    return;
                }
            }
            self.last_ime_commit = Some((text.to_string(), now));

            // NFC normalize: macOS may deliver Korean/CJK text in NFD (decomposed
            // Hangul jamo). Terminals and shells expect NFC (precomposed).
            let normalized: String = text.nfc().collect();

            // Clear selection when typing.
            self.terminal.with_term_mut(|term| {
                term.selection = None;
            });
            // Write committed text to PTY.
            self.terminal.write_to_pty(normalized.as_bytes());
        }

        self.reset_cursor_blink();
        cx.notify();
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        _range: Option<Range<usize>>,
        new_text: &str,
        new_selected_range: Option<Range<usize>>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Store composition (preedit) text — NEVER write to PTY.
        // NFC normalize for correct display of decomposed Hangul jamo.
        if new_text.is_empty() {
            self.marked_text = None;
            self.marked_text_selected_range = None;
        } else {
            let normalized: String = new_text.nfc().collect();
            self.marked_text = Some(normalized);
            self.marked_text_selected_range = new_selected_range;
        }

        self.reset_cursor_blink();
        cx.notify();
    }

    fn bounds_for_range(
        &mut self,
        _range_utf16: Range<usize>,
        element_bounds: Bounds<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<Bounds<Pixels>> {
        // Return cursor cell bounds for IME candidate window positioning.
        let content = self.terminal.content();
        let cursor_row = content.cursor.point.line.0 as f32;
        let cursor_col = content.cursor.point.column.0 as f32;
        let x = element_bounds.origin.x + self.cell_width * cursor_col;
        let y = element_bounds.origin.y + self.cell_height * cursor_row;
        Some(Bounds::new(
            point(x, y),
            size(self.cell_width, self.cell_height),
        ))
    }

    fn character_index_for_point(
        &mut self,
        point: gpui::Point<Pixels>,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<usize> {
        let col = ((f32::from(point.x) - f32::from(self.canvas_origin.x))
            / f32::from(self.cell_width)) as usize;
        Some(col)
    }
}

impl Render for CruxTerminalView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Measure cell metrics.
        self.measure_cell(window);

        // Process pending terminal events.
        self.process_events(window, cx);

        // Get the terminal content snapshot.
        let content = self.terminal.content();
        let focused = self.focus_handle.is_focused(window);
        // On focus loss, commit any active IME composition to PTY.
        // Without this, switching windows mid-composition would discard preedit text.
        if self.is_focused && !focused {
            if let Some(text) = self.marked_text.take() {
                self.marked_text_selected_range = None;
                if !text.is_empty() {
                    let normalized: String = text.nfc().collect();
                    self.terminal.write_to_pty(normalized.as_bytes());
                }
            }
        }
        self.is_focused = focused;
        let bell_active = self.is_bell_active();
        let cursor_visible = self.calculate_cursor_visible();
        self.cursor_blink_visible = cursor_visible;

        // Update dirty flag based on damage state.
        match &content.damage {
            DamageState::Full | DamageState::Partial(_) => self.dirty = true,
            DamageState::None => {}
        }

        // Clear dirty flag after processing.
        self.dirty = false;

        // Capture cell dimensions for the resize canvas.
        let cell_width = self.cell_width;
        let cell_height = self.cell_height;
        let marked_text = self.marked_text.clone();

        // Clone entity and focus handle for the resize/input canvas closures.
        let entity_for_resize = cx.entity().clone();
        let entity_for_input = cx.entity().clone();
        let focus_for_input = self.focus_handle.clone();

        div()
            .id("terminal-view")
            .track_focus(&self.focus_handle)
            .key_context("Terminal")
            .size_full()
            .on_key_down(cx.listener(Self::handle_key_down))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::handle_mouse_down))
            .on_mouse_down(MouseButton::Middle, cx.listener(Self::handle_mouse_down))
            .on_mouse_down(MouseButton::Right, cx.listener(Self::handle_mouse_down))
            .on_mouse_move(cx.listener(Self::handle_mouse_move))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::handle_mouse_up))
            .on_mouse_up(MouseButton::Middle, cx.listener(Self::handle_mouse_up))
            .on_mouse_up(MouseButton::Right, cx.listener(Self::handle_mouse_up))
            .on_scroll_wheel(cx.listener(Self::handle_scroll_wheel))
            .child(
                // Invisible canvas to detect size changes, capture origin, and register IME.
                canvas(
                    move |bounds: Bounds<Pixels>, _window: &mut Window, _cx: &mut App| {
                        (bounds.size, bounds.origin)
                    },
                    move |bounds: Bounds<Pixels>,
                          (size, origin): (Size<Pixels>, Point2D<Pixels>),
                          window: &mut Window,
                          cx: &mut App| {
                        entity_for_resize.update(cx, |this, _cx| {
                            this.resize_if_needed(size);
                            this.canvas_origin = origin;
                        });
                        // Register IME input handler during paint phase.
                        // GPUI routes character keystrokes through this handler,
                        // which calls our EntityInputHandler methods.
                        window.handle_input(
                            &focus_for_input,
                            ElementInputHandler::new(bounds, entity_for_input.clone()),
                            cx,
                        );
                    },
                )
                .absolute()
                .size_full(),
            )
            .child(render_terminal_canvas(
                content,
                cell_width,
                cell_height,
                self.font.clone(),
                self.font_size,
                focused,
                bell_active,
                cursor_visible,
                marked_text,
            ))
    }
}
