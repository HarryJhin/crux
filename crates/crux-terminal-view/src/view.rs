//! CruxTerminalView: GPUI View that owns a CruxTerminal and handles I/O.

use std::time::{Duration, Instant};

use gpui::*;

use crux_terminal::{
    Column, CruxTerminal, Line, Point, Scroll, Selection, SelectionType, Side, TermMode,
    TerminalEvent, TerminalSize,
};

use crate::element::render_terminal_canvas;
use crate::input;

const FONT_FAMILY: &str = "Menlo";
const FONT_SIZE: f32 = 14.0;

/// Duration for bell visual flash.
const BELL_FLASH_DURATION: Duration = Duration::from_millis(150);

/// Lines to scroll per mouse wheel tick.
const SCROLL_LINES_PER_TICK: i32 = 3;

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
}

/// Alias for GPUI's 2D point to avoid confusion with alacritty's grid Point.
type Point2D<T> = gpui::Point<T>;

impl CruxTerminalView {
    pub fn new(cx: &mut Context<Self>) -> Self {
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

        let terminal = CruxTerminal::new(None, size).expect("failed to create terminal");

        // Periodic refresh at ~60fps to pick up PTY output.
        cx.spawn(async |this: WeakEntity<Self>, cx: &mut AsyncApp| loop {
            cx.background_executor()
                .timer(Duration::from_millis(16))
                .await;
            let ok = this.update(cx, |_this: &mut Self, cx: &mut Context<Self>| {
                cx.notify();
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
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        // Handle Cmd+V for paste before forwarding to terminal.
        if event.keystroke.modifiers.platform && event.keystroke.key.as_str() == "v" {
            self.paste_from_clipboard(cx);
            return;
        }

        // Handle Cmd+C for copy before forwarding to terminal.
        if event.keystroke.modifiers.platform && event.keystroke.key.as_str() == "c" {
            self.copy_selection(window, cx);
            return;
        }

        // Get the current terminal mode for application cursor key detection.
        let mode = self.terminal.content().mode;

        if let Some(bytes) = input::keystroke_to_bytes(&event.keystroke, mode) {
            // Clear selection when typing.
            self.terminal.with_term_mut(|term| {
                term.selection = None;
            });
            self.terminal.write_to_pty(&bytes);
            cx.notify();
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

        let grid_point = self.pixel_to_grid(event.position);
        let side = self.pixel_to_side(event.position);
        let display_offset = self.terminal.content().display_offset;

        // Convert viewport point to absolute terminal point for selection.
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
        if event.pressed_button != Some(MouseButton::Left) {
            return;
        }

        let grid_point = self.pixel_to_grid(event.position);
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
        _event: &MouseUpEvent,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) {
        // Selection is finalized — keep it in place until next click or typing clears it.
    }

    fn handle_scroll_wheel(
        &mut self,
        event: &ScrollWheelEvent,
        _window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let delta = match event.delta {
            ScrollDelta::Lines(lines) => {
                // Negative y = scroll up (show history), positive = scroll down.
                -(lines.y * SCROLL_LINES_PER_TICK as f32) as i32
            }
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
    fn process_events(&mut self, window: &mut Window) {
        for event in self.terminal.drain_events() {
            match event {
                TerminalEvent::PtyWrite(text) => {
                    self.terminal.write_to_pty(text.as_bytes());
                }
                TerminalEvent::Title(title) => {
                    window.set_window_title(&title);
                    self.title = Some(title);
                }
                TerminalEvent::Bell => {
                    self.bell_at = Some(Instant::now());
                }
                TerminalEvent::ProcessExit(code) => {
                    log::info!("child process exited with code {}", code);
                    // TODO: close window or show exit indicator in Phase 2
                }
                TerminalEvent::Wakeup => {}
            }
        }
    }

    /// Returns true if the bell flash is currently active.
    fn is_bell_active(&self) -> bool {
        self.bell_at
            .is_some_and(|t| t.elapsed() < BELL_FLASH_DURATION)
    }
}

impl Focusable for CruxTerminalView {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for CruxTerminalView {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        // Measure cell metrics.
        self.measure_cell(window);

        // Process pending terminal events.
        self.process_events(window);

        // Get the terminal content snapshot.
        let content = self.terminal.content();
        let focused = self.focus_handle.is_focused(window);
        let bell_active = self.is_bell_active();

        // Capture cell dimensions for the resize canvas.
        let cell_width = self.cell_width;
        let cell_height = self.cell_height;
        let entity = cx.entity().clone();

        div()
            .id("terminal-view")
            .track_focus(&self.focus_handle)
            .key_context("Terminal")
            .size_full()
            .on_key_down(cx.listener(Self::handle_key_down))
            .on_mouse_down(MouseButton::Left, cx.listener(Self::handle_mouse_down))
            .on_mouse_move(cx.listener(Self::handle_mouse_move))
            .on_mouse_up(MouseButton::Left, cx.listener(Self::handle_mouse_up))
            .on_scroll_wheel(cx.listener(Self::handle_scroll_wheel))
            .child(
                // Invisible canvas to detect size changes and capture origin.
                canvas(
                    move |bounds: Bounds<Pixels>, _window: &mut Window, _cx: &mut App| {
                        (bounds.size, bounds.origin)
                    },
                    move |_bounds: Bounds<Pixels>,
                          (size, origin): (Size<Pixels>, Point2D<Pixels>),
                          _window: &mut Window,
                          cx: &mut App| {
                        entity.update(cx, |this, _cx| {
                            this.resize_if_needed(size);
                            this.canvas_origin = origin;
                        });
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
            ))
    }
}
