use std::path::Path;

use gpui::*;
use gpui_component::dock::{Panel, PanelEvent, PanelState};

use crux_terminal_view::CruxTerminalView;

/// A DockArea panel that wraps a `CruxTerminalView`.
///
/// This is a thin wrapper: all terminal logic lives in `CruxTerminalView`.
/// `CruxTerminalPanel` adapts it to the `Panel` trait for tab/split management.
pub struct CruxTerminalPanel {
    focus_handle: FocusHandle,
    terminal_view: Entity<CruxTerminalView>,
}

impl CruxTerminalPanel {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let focus_handle = cx.focus_handle();
        let terminal_view = cx.new(CruxTerminalView::new);

        // Focus the inner terminal view so key events reach the PTY.
        let inner_focus = terminal_view.read(cx).focus_handle(cx);
        inner_focus.focus(window);

        Self {
            focus_handle,
            terminal_view,
        }
    }

    /// Compute the display title from the terminal state.
    ///
    /// Priority: OSC title > CWD basename > "Terminal"
    fn display_title(&self, cx: &App) -> SharedString {
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
