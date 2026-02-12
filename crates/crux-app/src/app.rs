use std::sync::Arc;

use gpui::*;
use gpui_component::dock::{DockArea, DockItem};

use crate::actions::*;
use crate::dock::terminal_panel::CruxTerminalPanel;

/// Top-level application view managing the DockArea with terminal panels.
pub struct CruxApp {
    dock_area: Entity<DockArea>,
}

impl CruxApp {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let dock_area = cx.new(|cx| DockArea::new("crux-dock", Some(1), window, cx));

        // Create the initial terminal panel and set it as center.
        let weak_dock = dock_area.downgrade();
        let initial_tab = cx.new(|cx| CruxTerminalPanel::new(window, cx));
        let dock_item = DockItem::tab(initial_tab, &weak_dock, window, cx);
        dock_area.update(cx, |area, cx| {
            area.set_center(dock_item, window, cx);
        });

        // Register action handlers on this view.
        Self { dock_area }
    }

    fn action_new_tab(&mut self, _: &NewTab, window: &mut Window, cx: &mut Context<Self>) {
        let panel = cx.new(|cx| CruxTerminalPanel::new(window, cx));
        let panel_view: Arc<dyn gpui_component::dock::PanelView> = Arc::new(panel);
        self.dock_area.update(cx, |area, cx| {
            area.add_panel(
                panel_view,
                gpui_component::dock::DockPlacement::Center,
                None,
                window,
                cx,
            );
        });
    }

    fn action_close_tab(&mut self, _: &CloseTab, _window: &mut Window, _cx: &mut Context<Self>) {
        log::debug!("CloseTab action triggered (not yet implemented)");
    }

    fn action_next_tab(&mut self, _: &NextTab, _window: &mut Window, _cx: &mut Context<Self>) {
        log::debug!("NextTab action triggered (not yet implemented)");
    }

    fn action_prev_tab(&mut self, _: &PrevTab, _window: &mut Window, _cx: &mut Context<Self>) {
        log::debug!("PrevTab action triggered (not yet implemented)");
    }

    fn action_split_right(
        &mut self,
        _: &SplitRight,
        _window: &mut Window,
        _cx: &mut Context<Self>,
    ) {
        log::debug!("SplitRight action triggered (not yet implemented)");
    }

    fn action_split_down(&mut self, _: &SplitDown, _window: &mut Window, _cx: &mut Context<Self>) {
        log::debug!("SplitDown action triggered (not yet implemented)");
    }

    fn action_zoom_pane(&mut self, _: &ZoomPane, _window: &mut Window, _cx: &mut Context<Self>) {
        log::debug!("ZoomPane action triggered (not yet implemented)");
    }
}

impl Render for CruxApp {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("crux-app")
            .size_full()
            .on_action(cx.listener(Self::action_new_tab))
            .on_action(cx.listener(Self::action_close_tab))
            .on_action(cx.listener(Self::action_next_tab))
            .on_action(cx.listener(Self::action_prev_tab))
            .on_action(cx.listener(Self::action_split_right))
            .on_action(cx.listener(Self::action_split_down))
            .on_action(cx.listener(Self::action_zoom_pane))
            .child(self.dock_area.clone())
    }
}
