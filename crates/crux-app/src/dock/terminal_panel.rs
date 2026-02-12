use std::collections::HashMap;
use std::path::Path;

use gpui::*;
use gpui_component::dock::{Panel, PanelEvent, PanelState};

use crux_protocol::PaneId;
use crux_terminal_view::CruxTerminalView;

/// A DockArea panel that wraps a `CruxTerminalView`.
///
/// This is a thin wrapper: all terminal logic lives in `CruxTerminalView`.
/// `CruxTerminalPanel` adapts it to the `Panel` trait for tab/split management.
pub struct CruxTerminalPanel {
    /// IPC pane identifier. Used by CLI commands to target this panel.
    #[allow(dead_code)]
    pane_id: PaneId,
    focus_handle: FocusHandle,
    terminal_view: Entity<CruxTerminalView>,
}

impl CruxTerminalPanel {
    pub fn new(
        pane_id: PaneId,
        cwd: Option<&str>,
        command: Option<&[String]>,
        env: Option<&HashMap<String, String>>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let focus_handle = cx.focus_handle();

        // Safety: GPUI runs on the macOS main thread exclusively. Panel creation
        // and PTY spawning are synchronous within a single GPUI update cycle,
        // so no concurrent panel creation can interleave between set_var and
        // the child process inheriting the env. This is safe as long as GPUI
        // remains single-threaded (which is a macOS AppKit requirement).
        std::env::set_var("CRUX_PANE", pane_id.0.to_string());
        std::env::set_var("TERM_PROGRAM", "Crux");

        let terminal_view = cx.new(|cx| CruxTerminalView::new_with_options(cwd, command, env, cx));

        // Focus the inner terminal view so key events reach the PTY.
        let inner_focus = terminal_view.read(cx).focus_handle(cx);
        inner_focus.focus(window);

        Self {
            pane_id,
            focus_handle,
            terminal_view,
        }
    }

    /// Returns the pane ID assigned to this terminal panel.
    #[allow(dead_code)]
    pub fn pane_id(&self) -> PaneId {
        self.pane_id
    }

    /// Returns a reference to the inner terminal view entity.
    pub fn terminal_view(&self) -> &Entity<CruxTerminalView> {
        &self.terminal_view
    }

    /// Get the terminal size as (rows, cols).
    pub fn terminal_view_size(&self, cx: &App) -> (u32, u32) {
        let view = self.terminal_view.read(cx);
        let size = view.terminal_size();
        (size.rows as u32, size.cols as u32)
    }

    /// Write data to the terminal's PTY, optionally using bracketed paste mode.
    pub fn write_to_pty(&mut self, data: &[u8], bracketed_paste: bool, cx: &mut Context<Self>) {
        self.terminal_view.update(cx, |view, _cx| {
            if bracketed_paste {
                view.write_to_pty(b"\x1b[200~");
                view.write_to_pty(data);
                view.write_to_pty(b"\x1b[201~");
            } else {
                view.write_to_pty(data);
            }
        });
    }

    /// Get the terminal text content and cursor position.
    pub fn get_text(&self, cx: &App) -> (Vec<String>, u32, u32) {
        let view = self.terminal_view.read(cx);
        let lines = view.get_text_lines();
        let content = view.terminal_content_snapshot();
        (
            lines,
            content.cursor.point.line.0 as u32,
            content.cursor.point.column.0 as u32,
        )
    }

    /// Scroll to the previous prompt in the terminal scrollback.
    pub fn scroll_to_prev_prompt(&self, cx: &mut Context<Self>) {
        self.terminal_view.update(cx, |view, _cx| {
            view.scroll_to_prev_prompt();
        });
    }

    /// Scroll to the next prompt in the terminal scrollback.
    pub fn scroll_to_next_prompt(&self, cx: &mut Context<Self>) {
        self.terminal_view.update(cx, |view, _cx| {
            view.scroll_to_next_prompt();
        });
    }

    /// Compute the display title from the terminal state.
    ///
    /// Priority: OSC title > CWD basename > "Terminal"
    pub fn display_title(&self, cx: &App) -> SharedString {
        let view = self.terminal_view.read(cx);

        // 1. Use OSC title if set by the shell/program.
        if let Some(title) = view.title() {
            if !title.is_empty() {
                return SharedString::from(title.to_string());
            }
        }

        // 2. Use the last component of the CWD reported via OSC 7.
        if let Some(cwd) = view.cwd() {
            let basename = Path::new(cwd)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(cwd);
            if !basename.is_empty() {
                return SharedString::from(basename.to_string());
            }
        }

        // 3. Default fallback.
        "Terminal".into()
    }
}

impl Panel for CruxTerminalPanel {
    fn panel_name(&self) -> &'static str {
        "CruxTerminalPanel"
    }

    fn title(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        self.display_title(cx)
    }

    fn closable(&self, _cx: &App) -> bool {
        true
    }

    fn dump(&self, _cx: &App) -> PanelState {
        PanelState::new(self)
    }

    fn inner_padding(&self, _cx: &App) -> bool {
        false
    }
}

impl EventEmitter<PanelEvent> for CruxTerminalPanel {}

impl Focusable for CruxTerminalPanel {
    fn focus_handle(&self, _cx: &App) -> FocusHandle {
        self.focus_handle.clone()
    }
}

impl Render for CruxTerminalPanel {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("terminal-panel")
            .track_focus(&self.focus_handle)
            .size_full()
            .child(self.terminal_view.clone())
    }
}
