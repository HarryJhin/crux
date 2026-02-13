//! CruxTerminalView: GPUI View that owns a CruxTerminal and handles I/O.

use std::ops::Range;
use std::time::{Duration, Instant};

use gpui::*;
use unicode_normalization::UnicodeNormalization;

use crux_config::{ColorConfig, FontConfig};
use crux_terminal::{
    Column, CruxTerminal, DamageState, Dimensions, Line, Point, Scroll, Selection, SelectionType,
    Side, TermMode, TerminalContent, TerminalEvent, TerminalSize,
};

use crate::element::render_terminal_canvas;
use crate::input::OptionAsAlt;
use crate::mouse;

/// Duration for bell visual flash.
const BELL_FLASH_DURATION: Duration = Duration::from_millis(150);

/// Lines to scroll per mouse wheel tick.
const SCROLL_LINES_PER_TICK: i32 = 3;

/// Window for IME event deduplication. Identical insertText: calls within
/// this interval are treated as duplicates (prevents the double-space bug
/// observed in some CJK input methods).
pub(crate) const IME_DEDUP_WINDOW: Duration = Duration::from_millis(10);

/// GPUI View wrapping a terminal emulator with keyboard input and rendering.
pub struct CruxTerminalView {
    pub(crate) terminal: CruxTerminal,
    pub(crate) focus_handle: FocusHandle,
    font: Font,
    font_size: Pixels,
    /// Font configuration; updated via `update_font_config()` for hot-reload support.
    #[allow(dead_code)]
    font_config: FontConfig,
    pub(crate) color_config: ColorConfig,
    pub(crate) cell_width: Pixels,
    pub(crate) cell_height: Pixels,
    /// Origin of the terminal canvas in window coordinates, updated each render.
    pub(crate) canvas_origin: Point2D<Pixels>,
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
    pub(crate) option_as_alt: OptionAsAlt,
    /// Last reported mouse grid position, for motion event deduplication.
    last_mouse_grid: Option<Point>,
    /// IME composition (preedit) text, displayed as overlay at cursor position.
    /// Set by `replace_and_mark_text_in_range`, cleared on commit or `unmark_text`.
    pub(crate) marked_text: Option<String>,
    /// Selected range within composition text (UTF-16 offsets from IME).
    pub(crate) marked_text_selected_range: Option<Range<usize>>,
    /// Last IME commit for deduplication (text, timestamp). Prevents double-space bug (Alacritty #8079).
    pub(crate) last_ime_commit: Option<(String, Instant)>,
    /// Synthetic text buffer for IME recombination.
    ///
    /// Korean/CJK input methods call `attributedSubstringForProposedRange:`
    /// to recall previously committed characters when assembling multi-part
    /// syllables (e.g., ㅇ + ㅏ → 아). Terminals have no text document, so
    /// we maintain a small synthetic buffer of recently committed text.
    /// The IME queries this buffer via `text_for_range` and uses
    /// `replacementRange` in `setMarkedText:` to replace previous characters.
    ///
    /// Cleared when the user presses Enter, arrow keys, or other non-text keys.
    pub(crate) ime_buffer: String,
    /// Timestamp when IME composition started, for stale preedit detection.
    pub(crate) marked_text_timestamp: Option<Instant>,
    /// Whether Vim IME auto-switch is enabled.
    vim_ime_switch: bool,
    /// The input source that was active before switching to ASCII.
    saved_input_source: Option<String>,
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

    /// Returns whether IME is currently composing (has active preedit text).
    pub fn is_composing(&self) -> bool {
        self.marked_text.is_some()
    }

    /// Enable or disable Vim IME auto-switch (cursor shape triggers IME change).
    pub fn set_vim_ime_switch(&mut self, enabled: bool) {
        self.vim_ime_switch = enabled;
    }

    /// Update font configuration and recalculate cell metrics (font size, line height).
    /// TODO: Call this when config hot-reload is wired into the app.
    #[allow(dead_code)]
    pub(crate) fn update_font_config(&mut self, config: FontConfig) {
        self.font_config = config;
        self.font_size = px(self.font_config.size);
        self.font = font(&self.font_config.family);
        // Mark cell metrics as needing remeasurement on next layout.
        self.cell_measured = false;
    }

    pub fn new(cx: &mut Context<Self>) -> Self {
        use crux_config::TerminalConfig;
        Self::new_with_options(
            None,
            None,
            None,
            FontConfig::default(),
            ColorConfig::default(),
            TerminalConfig::default(),
            cx,
        )
    }

    /// Create a new terminal view with optional cwd, command, and env.
    ///
    /// The `terminal_config` parameter provides shell, shell_args, env, and scrollback_lines.
    /// If `command` is provided (from IPC), it overrides the config shell.
    /// If `env` is provided (from IPC), it is merged with config env (IPC env takes precedence).
    pub fn new_with_options(
        cwd: Option<&str>,
        command: Option<&[String]>,
        env: Option<&std::collections::HashMap<String, String>>,
        font_config: FontConfig,
        color_config: ColorConfig,
        terminal_config: crux_config::TerminalConfig,
        cx: &mut Context<Self>,
    ) -> Self {
        let focus_handle = cx.focus_handle();

        let terminal_font = font(&font_config.family);
        let font_size = px(font_config.size);

        // Default cell metrics; will be recalculated on first layout.
        let cell_width = px(8.4);
        let cell_height = px(17.0);

        let size = TerminalSize {
            rows: 24,
            cols: 80,
            cell_width: f32::from(cell_width),
            cell_height: f32::from(cell_height),
            scrollback_lines: terminal_config.scrollback_lines,
        };

        // Merge environment variables: config env + IPC env (IPC takes precedence).
        let mut merged_env = terminal_config.env.clone();
        if let Some(ipc_env) = env {
            merged_env.extend(ipc_env.iter().map(|(k, v)| (k.clone(), v.clone())));
        }

        // Use command from IPC if provided, otherwise use config shell.
        let shell = if command.is_none() {
            terminal_config.shell.clone()
        } else {
            None
        };

        let terminal = match CruxTerminal::new(
            shell,
            Some(&terminal_config.shell_args),
            size,
            cwd,
            command,
            Some(&merged_env),
        ) {
            Ok(term) => term,
            Err(e) => {
                log::error!("Failed to create terminal: {}. Using default shell.", e);
                // Fall back to default shell without custom command
                CruxTerminal::new(None, Some(&terminal_config.shell_args), size, cwd, None, Some(&merged_env))
                    .unwrap_or_else(|err| {
                        panic!(
                            "Failed to create terminal even with default shell: {}. \
                            This likely means no usable shell was found (/bin/bash, /bin/zsh, /bin/sh). \
                            Original error: {}",
                            err, e
                        )
                    })
            }
        };

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
            font_config,
            color_config,
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
            ime_buffer: String::new(),
            marked_text_timestamp: None,
            vim_ime_switch: false,
            saved_input_source: None,
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
        // Use actual font metrics (ascent + descent) for line height.
        // ShapedLine derefs to LineLayout which provides ascent/descent.
        // Add small leading (2px) for comfortable terminal spacing.
        let ascent = shaped.ascent;
        let descent = shaped.descent;
        self.cell_height = (ascent + descent + px(2.0)).ceil();
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
                scrollback_lines: current.scrollback_lines,
            });
        }
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
        let content = self.terminal.content();
        let mode = content.mode;

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
        let display_offset = content.display_offset;
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
        let content = self.terminal.content();
        let mode = content.mode;

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
        let display_offset = content.display_offset;
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
    pub(crate) fn copy_selection(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        if let Some(text) = self.terminal.selection_to_string() {
            cx.write_to_clipboard(ClipboardItem::new_string(text));
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
                    // Process exit should not quit the entire app in multi-pane scenarios.
                    // The app layer or user decides when to close individual panes.
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
                TerminalEvent::ClipboardSet { data } => {
                    cx.write_to_clipboard(ClipboardItem::new_string(data));
                }
                TerminalEvent::CursorShapeChanged {
                    old_shape,
                    new_shape,
                } => {
                    if self.vim_ime_switch {
                        use crux_terminal::CursorShape;
                        let entering_normal = matches!(new_shape, CursorShape::Block)
                            && !matches!(old_shape, CursorShape::Block);
                        let leaving_normal = matches!(old_shape, CursorShape::Block)
                            && !matches!(new_shape, CursorShape::Block);
                        if entering_normal {
                            #[cfg(target_os = "macos")]
                            {
                                self.saved_input_source = crate::ime_switch::current_input_source();
                                crate::ime_switch::switch_to_ascii();
                            }
                        } else if leaving_normal {
                            #[cfg(target_os = "macos")]
                            if let Some(ref source_id) = self.saved_input_source.take() {
                                crate::ime_switch::switch_to_input_source(source_id);
                            }
                        }
                    }
                }
                TerminalEvent::Graphics { .. } => {
                    // Graphics events are handled by the image manager (future).
                }
            }
        }
        // Mark dirty if we received any events.
        if had_events {
            self.dirty = true;
        }

        // Force-commit stale IME composition after 5 seconds.
        if let Some(ts) = self.marked_text_timestamp {
            if ts.elapsed() > Duration::from_secs(5) {
                log::warn!("[IME] force-committing stale composition after 5s timeout");
                if let Some(text) = self.marked_text.take() {
                    let normalized: String = text.nfc().collect();
                    self.terminal.write_to_pty(normalized.as_bytes());
                }
                self.marked_text_selected_range = None;
                self.marked_text_timestamp = None;
                cx.notify();
            }
        }
    }

    /// Returns true if the bell flash is currently active.
    fn is_bell_active(&self) -> bool {
        self.bell_at
            .is_some_and(|t| t.elapsed() < BELL_FLASH_DURATION)
    }

    /// Reset cursor blink to visible state (called on user input or click).
    pub(crate) fn reset_cursor_blink(&mut self) {
        self.cursor_blink_visible = true;
        self.cursor_blink_epoch = Instant::now();
    }

    /// Select all terminal content (screen + scrollback).
    pub(crate) fn select_all(&mut self) {
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
        // Use shared helper from crux-terminal.
        crux_terminal::extract_text_lines(&content)
    }

    /// Get terminal grid content as text lines from an existing content snapshot.
    ///
    /// This avoids redundant FairMutex acquisition when the caller already has
    /// a content snapshot.
    pub fn get_text_lines_from_content(&self, content: &TerminalContent) -> Vec<String> {
        crux_terminal::extract_text_lines(content)
    }

    /// Get the terminal size.
    pub fn terminal_size(&self) -> TerminalSize {
        self.terminal.size()
    }

    /// Get the currently selected text, if any.
    pub fn selection_to_string(&self) -> Option<String> {
        self.terminal.selection_to_string()
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
        // Build virtual document: [ime_buffer][marked_text]
        // The Korean IM calls attributedSubstringForProposedRange: to recall
        // previously committed characters for syllable recombination.
        let buf_utf16_len: usize = self.ime_buffer.chars().map(|c| c.len_utf16()).sum();
        let marked_utf16_len: usize = self
            .marked_text
            .as_ref()
            .map(|t| t.chars().map(|c| c.len_utf16()).sum())
            .unwrap_or(0);
        let total_utf16_len = buf_utf16_len + marked_utf16_len;

        if range_utf16.start >= total_utf16_len {
            return None;
        }

        // Build the virtual document string.
        let mut doc = self.ime_buffer.clone();
        if let Some(ref mt) = self.marked_text {
            doc.push_str(mt);
        }

        let range_start = range_utf16.start;
        let range_end = range_utf16.end;
        let start = utf16_offset_to_utf8(&doc, range_start).min(doc.len());
        let end = utf16_offset_to_utf8(&doc, range_end).min(doc.len());
        *adjusted_range = Some(range_utf16);
        log::trace!(
            "[IME] text_for_range({:?}) -> {:?} (buf={:?}, marked={:?})",
            range_start..range_end,
            &doc[start..end],
            self.ime_buffer,
            self.marked_text,
        );
        Some(doc[start..end].to_string())
    }

    fn selected_text_range(
        &mut self,
        _ignore_disabled_input: bool,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) -> Option<UTF16Selection> {
        // Cursor position in the virtual document [ime_buffer][marked_text].
        let buf_utf16_len: usize = self.ime_buffer.chars().map(|c| c.len_utf16()).sum();
        let range = if let Some(ref sel) = self.marked_text_selected_range {
            // Offset selection into the virtual document.
            (buf_utf16_len + sel.start)..(buf_utf16_len + sel.end)
        } else if let Some(ref text) = self.marked_text {
            let marked_utf16_len: usize = text.chars().map(|c| c.len_utf16()).sum();
            let pos = buf_utf16_len + marked_utf16_len;
            pos..pos
        } else {
            // Cursor at end of buffer (after last committed char).
            buf_utf16_len..buf_utf16_len
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
        log::trace!(
            "[IME] marked_text_range called, marked_text={:?}, ime_buffer={:?}",
            self.marked_text,
            self.ime_buffer
        );
        // Return Some only when there's real composition (marked text).
        //
        // GPUI routes keystrokes based on is_composing (window.rs:1696-1712):
        //   - is_composing=true  → IME receives the key FIRST via handleEvent:
        //   - is_composing=false → GPUI dispatch first, IME fallback (line 1762)
        //
        // When not composing, character keys that our handle_key_down doesn't
        // stop_propagation for will fall through to [inputContext handleEvent:]
        // at line 1762, so the IME still receives them.
        //
        // Returning None (→ markedRange={NSNotFound,0}) when there's no real
        // composition tells the Korean IM "ready for new composition", causing
        // it to call setMarkedText: (compose) instead of insertText: (commit)
        // for the first consonant.
        if let Some(ref text) = self.marked_text {
            if !text.is_empty() {
                let buf_utf16_len: usize = self.ime_buffer.chars().map(|c| c.len_utf16()).sum();
                let marked_utf16_len: usize = text.chars().map(|c| c.len_utf16()).sum();
                return Some(buf_utf16_len..(buf_utf16_len + marked_utf16_len));
            }
        }
        None
    }

    fn unmark_text(&mut self, _window: &mut Window, _cx: &mut Context<Self>) {
        self.marked_text = None;
        self.marked_text_selected_range = None;
        self.marked_text_timestamp = None;
    }

    fn replace_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        text: &str,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        log::info!(
            "[IME] replace_text_in_range called, text={:?}, range={:?}, buf={:?}",
            text,
            range_utf16,
            self.ime_buffer
        );
        // Clear composition state — this is a commit.
        self.marked_text = None;
        self.marked_text_selected_range = None;
        self.marked_text_timestamp = None;

        if !text.is_empty() {
            // Dedup: some CJK input methods fire duplicate insertText: calls
            // within a few milliseconds. Only suppress exact duplicates that
            // have NO replacement range — commits with a range are part of
            // Korean syllable recombination and must always be processed.
            let now = Instant::now();
            if range_utf16.is_none() {
                if let Some((ref prev_text, prev_time)) = self.last_ime_commit {
                    if prev_text == text && now.duration_since(prev_time) < IME_DEDUP_WINDOW {
                        return;
                    }
                }
            }
            self.last_ime_commit = Some((text.to_string(), now));

            // NFC normalize: macOS may deliver Korean/CJK text in NFD (decomposed
            // Hangul jamo). Terminals and shells expect NFC (precomposed).
            let normalized: String = text.nfc().collect();

            // Handle replacement range: the IM may replace previously committed
            // buffer characters (e.g., Korean recombination ㅇ+ㅏ → 아).
            let buf_utf16_len: usize = self.ime_buffer.chars().map(|c| c.len_utf16()).sum();
            if let Some(ref range) = range_utf16 {
                if range.start < buf_utf16_len {
                    // Replacement covers buffer characters → send DEL to PTY
                    // for each character being replaced, then update buffer.
                    let replace_start = utf16_offset_to_utf8(&self.ime_buffer, range.start);
                    let replace_end =
                        utf16_offset_to_utf8(&self.ime_buffer, range.end.min(buf_utf16_len));
                    let chars_to_delete =
                        self.ime_buffer[replace_start..replace_end].chars().count();
                    // Send DEL (\x7f) for each character to erase from shell input.
                    for _ in 0..chars_to_delete {
                        self.terminal.write_to_pty(&[0x7f]);
                    }
                    self.ime_buffer
                        .replace_range(replace_start..replace_end, "");
                }
            }

            // Clear selection when typing.
            self.terminal.with_term_mut(|term| {
                term.selection = None;
            });

            // Control characters (backspace, DEL, escape, etc.) must be
            // forwarded to PTY but NEVER stored in the IME buffer.
            // Storing them corrupts text_for_range queries and breaks
            // Korean syllable recombination.
            let is_control = normalized.chars().all(|c| c.is_control());

            // Always write to PTY.
            self.terminal.write_to_pty(normalized.as_bytes());

            if is_control {
                // Backspace/DEL: pop the last char from the buffer to keep
                // it in sync with the shell's visible input line.
                if normalized.contains('\u{8}') || normalized.contains('\x7f') {
                    self.ime_buffer.pop();
                }
                // Other control chars (e.g. ESC) just pass through to PTY.
            } else {
                // Printable text: append to buffer for IME recombination.
                self.ime_buffer.push_str(&normalized);
                self.trim_ime_buffer();
            }
        }

        self.reset_cursor_blink();
        cx.notify();
    }

    fn replace_and_mark_text_in_range(
        &mut self,
        range_utf16: Option<Range<usize>>,
        new_text: &str,
        new_selected_range: Option<Range<usize>>,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        log::info!("[IME] replace_and_mark_text_in_range called, text={:?}, range={:?}, selected={:?}, buf={:?}", new_text, range_utf16, new_selected_range, self.ime_buffer);
        // Handle replacement range: the Korean IM may replace a previously
        // committed consonant with a combined syllable (e.g., replace "ㅇ"
        // in the buffer with composition "아").
        let buf_utf16_len: usize = self.ime_buffer.chars().map(|c| c.len_utf16()).sum();
        if let Some(ref range) = range_utf16 {
            if range.start < buf_utf16_len {
                let replace_start = utf16_offset_to_utf8(&self.ime_buffer, range.start);
                let replace_end =
                    utf16_offset_to_utf8(&self.ime_buffer, range.end.min(buf_utf16_len));
                let chars_to_delete = self.ime_buffer[replace_start..replace_end].chars().count();
                for _ in 0..chars_to_delete {
                    self.terminal.write_to_pty(&[0x7f]);
                }
                self.ime_buffer
                    .replace_range(replace_start..replace_end, "");
            }
        }

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
        self.marked_text_timestamp = if self.marked_text.is_some() {
            Some(Instant::now())
        } else {
            None
        };

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

#[cfg(test)]
mod tests {
    use super::utf16_offset_to_utf8;

    #[test]
    fn test_utf16_offset_ascii() {
        // ASCII: 1 UTF-16 unit = 1 UTF-8 byte
        assert_eq!(utf16_offset_to_utf8("hello", 0), 0);
        assert_eq!(utf16_offset_to_utf8("hello", 3), 3);
        assert_eq!(utf16_offset_to_utf8("hello", 5), 5);
    }

    #[test]
    fn test_utf16_offset_korean() {
        // Korean: 1 UTF-16 unit = 3 UTF-8 bytes
        let s = "안녕"; // 2 chars, 6 UTF-8 bytes, 2 UTF-16 units
        assert_eq!(utf16_offset_to_utf8(s, 0), 0);
        assert_eq!(utf16_offset_to_utf8(s, 1), 3);
        assert_eq!(utf16_offset_to_utf8(s, 2), 6);
    }

    #[test]
    fn test_utf16_offset_emoji() {
        // Emoji (surrogate pair): 2 UTF-16 units = 4 UTF-8 bytes
        let s = "😀"; // 1 char, 4 UTF-8 bytes, 2 UTF-16 units
        assert_eq!(utf16_offset_to_utf8(s, 0), 0);
        assert_eq!(utf16_offset_to_utf8(s, 2), 4);
    }

    #[test]
    fn test_utf16_offset_empty() {
        assert_eq!(utf16_offset_to_utf8("", 0), 0);
    }

    #[test]
    fn test_utf16_offset_mixed() {
        let s = "a안b"; // 'a'=1+1, '안'=3+1, 'b'=1+1 → 5 UTF-8 bytes, 3 UTF-16 units
        assert_eq!(utf16_offset_to_utf8(s, 0), 0);
        assert_eq!(utf16_offset_to_utf8(s, 1), 1); // after 'a'
        assert_eq!(utf16_offset_to_utf8(s, 2), 4); // after '안'
        assert_eq!(utf16_offset_to_utf8(s, 3), 5); // after 'b'
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
        self.terminal.with_term_mut(|t| t.is_focused = focused);
        let bell_active = self.is_bell_active();
        let cursor_visible = self.calculate_cursor_visible();
        self.cursor_blink_visible = cursor_visible;

        // Update dirty flag based on damage state.
        match &content.damage {
            DamageState::Full | DamageState::Partial(_) => self.dirty = true,
            DamageState::None => {}
        }

        // Clear dirty flag now that we are rendering this frame.
        self.dirty = false;

        // Capture cell dimensions for the resize canvas.
        let cell_width = self.cell_width;
        let cell_height = self.cell_height;
        let marked_text = self.marked_text.clone();
        if marked_text.is_some() {
            log::debug!(
                "[IME] render: passing marked_text={:?} to canvas",
                marked_text
            );
        }

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
            .on_drop(
                cx.listener(|this: &mut Self, paths: &ExternalPaths, _window, _cx| {
                    let escaped: Vec<String> = paths
                        .paths()
                        .iter()
                        .map(|p| shell_escape::escape(p.to_string_lossy()).to_string())
                        .collect();
                    let text = escaped.join(" ");
                    this.write_to_pty_with_bracketed_paste(text.as_bytes());
                }),
            )
            .drag_over::<ExternalPaths>(|style, _, _, _| {
                style.border_2().border_color(Hsla {
                    h: 0.58,
                    s: 0.7,
                    l: 0.5,
                    a: 0.8,
                })
            })
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
            .child(render_terminal_canvas(crate::element::RenderConfig {
                content,
                cell_width,
                cell_height,
                font: self.font.clone(),
                font_size: self.font_size,
                focused,
                bell_active,
                cursor_visible,
                marked_text,
                color_config: self.color_config.clone(),
            }))
    }
}
