use gpui::*;
use gpui_component::dock::{Panel, PanelEvent, PanelState};

use crux_terminal_view::CruxTerminalView;

/// A DockArea panel that wraps a `CruxTerminalView`.
///
/// This is a thin wrapper: all terminal logic lives in `CruxTerminalView`.
/// `CruxTerminalPanel` adapts it to the `Panel` trait for tab/split management.
pub struct CruxTerminalPanel {
    title: SharedString,
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
            title: "Terminal".into(),
            focus_handle,
            terminal_view,
        }
    }
}

impl Panel for CruxTerminalPanel {
    fn panel_name(&self) -> &'static str {
        "CruxTerminalPanel"
    }

    fn title(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        self.title.clone()
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
