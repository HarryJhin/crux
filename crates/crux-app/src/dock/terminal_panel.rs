use std::collections::HashMap;
use std::path::Path;

use gpui::*;
use gpui_component::dock::{register_panel, Panel, PanelEvent, PanelInfo, PanelState};

use crux_protocol::PaneId;
use crux_terminal_view::CruxTerminalView;

/// Register `CruxTerminalPanel` in the global PanelRegistry so that
/// `DockArea::load` can reconstruct terminal panels from saved state.
pub fn register(cx: &mut App) {
    register_panel(
        cx,
        "CruxTerminalPanel",
        |_dock_area, _state, info, window, cx| {
            let (pane_id, cwd) = match info {
                PanelInfo::Panel(json) => {
                    let id = json.get("pane_id").and_then(|v| v.as_u64()).unwrap_or(0);
                    let cwd = json
                        .get("cwd")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());
                    (PaneId(id), cwd)
                }
                _ => (PaneId(0), None),
            };
            Box::new(
                cx.new(|cx| {
                    CruxTerminalPanel::new(pane_id, cwd.as_deref(), None, None, window, cx)
                }),
            )
        },
    );
}

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

        // Build environment variables for the child process.
        // Merge user-provided env with CRUX_PANE and TERM_PROGRAM.
        let mut child_env = env.cloned().unwrap_or_default();
        child_env.insert("CRUX_PANE".to_string(), pane_id.0.to_string());
        child_env.insert("TERM_PROGRAM".to_string(), "Crux".to_string());

        let terminal_view = cx.new(|cx| CruxTerminalView::new_with_options(cwd, command, Some(&child_env), cx));

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
    pub fn pane_id(&self) -> PaneId {
        self.pane_id
    }

    /// Returns a reference to the inner terminal view entity.
    pub fn terminal_view(&self) -> &Entity<CruxTerminalView> {
        &self.terminal_view
    }

    /// Returns whether IME is currently composing (has active preedit text).
    pub fn is_composing(&self, cx: &App) -> bool {
        self.terminal_view.read(cx).is_composing()
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

    /// Get the currently selected text, if any.
    pub fn get_selection(&self, cx: &App) -> Option<String> {
        self.terminal_view.read(cx).selection_to_string()
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

    /// Get a full snapshot of the terminal state (text + metadata).
    pub fn get_snapshot(&self, cx: &App) -> crux_protocol::GetSnapshotResult {
        let view = self.terminal_view.read(cx);
        let lines = view.get_text_lines();
        let content = view.terminal_content_snapshot();
        let size = view.terminal_size();
        let cursor_shape = format!("{:?}", content.cursor.shape);
        crux_protocol::GetSnapshotResult {
            lines,
            rows: size.rows as u32,
            cols: size.cols as u32,
            cursor_row: content.cursor.point.line.0,
            cursor_col: content.cursor.point.column.0 as u32,
            cursor_shape,
            display_offset: content.display_offset as u32,
            has_selection: content.selection.is_some(),
            title: view.title().map(|s| s.to_string()),
            cwd: view.cwd().map(|s| s.to_string()),
        }
    }

    /// Scroll to the previous prompt in the terminal scrollback.
    pub fn scroll_to_prev_prompt(&self, cx: &mut Context<Self>) {
        self.terminal_view.update(cx, |view, _cx| {
            view.scroll_to_prev_prompt();
        });
    }

    /// Check if the terminal's child process is still running.
    pub fn is_process_running(&mut self, cx: &mut Context<Self>) -> bool {
        self.terminal_view
            .update(cx, |view, _cx| view.is_process_running())
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

    fn dump(&self, cx: &App) -> PanelState {
        let view = self.terminal_view.read(cx);
        let info_json = serde_json::json!({
            "pane_id": self.pane_id.0,
            "cwd": view.cwd(),
            "title": view.title(),
        });
        let mut state = PanelState::new(self);
        state.info = PanelInfo::panel(info_json);
        state
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
