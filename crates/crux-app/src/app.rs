use std::sync::Arc;

use gpui::*;
use gpui_component::dock::{
    DockArea, DockItem, DockPlacement, PanelView, TabPanel, ToggleZoom,
};
use gpui_component::Placement;

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

        Self { dock_area }
    }

    // -- Helpers --------------------------------------------------------

    /// Collect all TabPanel entities from the DockItem tree in depth-first order.
    fn collect_tab_panels(item: &DockItem) -> Vec<Entity<TabPanel>> {
        let mut result = Vec::new();
        Self::collect_tab_panels_recursive(item, &mut result);
        result
    }

    fn collect_tab_panels_recursive(item: &DockItem, out: &mut Vec<Entity<TabPanel>>) {
        match item {
            DockItem::Tabs { view, .. } => {
                out.push(view.clone());
            }
            DockItem::Split { items, .. } => {
                for child in items {
                    Self::collect_tab_panels_recursive(child, out);
                }
            }
            _ => {}
        }
    }

    /// Find the focused TabPanel among all center tab panels.
    fn focused_tab_panel(
        &self,
        window: &Window,
        cx: &App,
    ) -> Option<Entity<TabPanel>> {
        let items = self.dock_area.read(cx).items();
        let tab_panels = Self::collect_tab_panels(items);

        // Find the one whose focus handle currently contains the focused element.
        for tp in &tab_panels {
            let fh = tp.read(cx).focus_handle(cx);
            if fh.contains_focused(window, cx) {
                return Some(tp.clone());
            }
        }

        // Fallback: return the first tab panel.
        tab_panels.into_iter().next()
    }

    // -- Action handlers ------------------------------------------------

    fn action_new_tab(&mut self, _: &NewTab, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(tab_panel) = self.focused_tab_panel(window, cx) {
            let panel = cx.new(|cx| CruxTerminalPanel::new(window, cx));
            let panel_view: Arc<dyn PanelView> = Arc::new(panel);
            tab_panel.update(cx, |tp, cx| {
                tp.add_panel(panel_view, window, cx);
            });
        } else {
            let panel = cx.new(|cx| CruxTerminalPanel::new(window, cx));
            let panel_view: Arc<dyn PanelView> = Arc::new(panel);
            self.dock_area.update(cx, |area, cx| {
                area.add_panel(panel_view, DockPlacement::Center, None, window, cx);
            });
        }
    }

    fn action_close_tab(&mut self, _: &CloseTab, window: &mut Window, cx: &mut Context<Self>) {
        let Some(tab_panel) = self.focused_tab_panel(window, cx) else {
            return;
        };

        tab_panel.update(cx, |tp, cx| {
            if let Some(panel) = tp.active_panel(cx) {
                tp.remove_panel(panel, window, cx);
            }
        });
    }

    fn action_next_tab(&mut self, _: &NextTab, window: &mut Window, cx: &mut Context<Self>) {
        let Some(tab_panel) = self.focused_tab_panel(window, cx) else {
            return;
        };

        tab_panel.update(cx, |tp, cx| {
            let count = tp.panels_count();
            if count == 0 {
                return;
            }
            let next = (tp.active_index() + 1) % count;
            tp.set_active_ix(next, window, cx);
        });
    }

    fn action_prev_tab(&mut self, _: &PrevTab, window: &mut Window, cx: &mut Context<Self>) {
        let Some(tab_panel) = self.focused_tab_panel(window, cx) else {
            return;
        };

        tab_panel.update(cx, |tp, cx| {
            let count = tp.panels_count();
            if count == 0 {
                return;
            }
            let prev = if tp.active_index() == 0 {
                count - 1
            } else {
                tp.active_index() - 1
            };
            tp.set_active_ix(prev, window, cx);
        });
    }

    fn action_select_tab(
        &mut self,
        index: usize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(tab_panel) = self.focused_tab_panel(window, cx) else {
            return;
        };

        tab_panel.update(cx, |tp, cx| {
            let count = tp.panels_count();
            if count == 0 {
                return;
            }
            let ix = index.min(count - 1);
            tp.set_active_ix(ix, window, cx);
        });
    }

    fn action_split_right(
        &mut self,
        _: &SplitRight,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.split_pane(Placement::Right, window, cx);
    }

    fn action_split_down(
        &mut self,
        _: &SplitDown,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.split_pane(Placement::Bottom, window, cx);
    }

    fn split_pane(
        &mut self,
        placement: Placement,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(tab_panel) = self.focused_tab_panel(window, cx) else {
            return;
        };

        let panel = cx.new(|cx| CruxTerminalPanel::new(window, cx));
        let panel_view: Arc<dyn PanelView> = Arc::new(panel);

        tab_panel.update(cx, |tp, cx| {
            tp.add_panel_at(panel_view, placement, None, window, cx);
        });
    }

    fn action_zoom_pane(
        &mut self,
        _: &ZoomPane,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(tab_panel) = self.focused_tab_panel(window, cx) else {
            return;
        };

        tab_panel.update(cx, |tp, cx| {
            tp.on_action_toggle_zoom(&ToggleZoom, window, cx);
        });
    }

    fn action_focus_next_pane(
        &mut self,
        _: &FocusNextPane,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.cycle_pane_focus(1, window, cx);
    }

    fn action_focus_prev_pane(
        &mut self,
        _: &FocusPrevPane,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.cycle_pane_focus(-1, window, cx);
    }

    fn cycle_pane_focus(
        &mut self,
        direction: isize,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let items = self.dock_area.read(cx).items().clone();
        let tab_panels = Self::collect_tab_panels(&items);
        let count = tab_panels.len();
        if count <= 1 {
            return;
        }

        let current_ix = tab_panels
            .iter()
            .position(|tp| {
                let fh = tp.read(cx).focus_handle(cx);
                fh.contains_focused(window, cx)
            })
            .unwrap_or(0);

        let next_ix = if direction > 0 {
            (current_ix + 1) % count
        } else if current_ix == 0 {
            count - 1
        } else {
            current_ix - 1
        };

        let target = &tab_panels[next_ix];
        let fh = target.read(cx).focus_handle(cx);
        fh.focus(window);
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
            .on_action(cx.listener(Self::action_focus_next_pane))
            .on_action(cx.listener(Self::action_focus_prev_pane))
            .on_action(cx.listener(|this: &mut Self, _: &SelectTab1, window, cx| {
                this.action_select_tab(0, window, cx);
            }))
            .on_action(cx.listener(|this: &mut Self, _: &SelectTab2, window, cx| {
                this.action_select_tab(1, window, cx);
            }))
            .on_action(cx.listener(|this: &mut Self, _: &SelectTab3, window, cx| {
                this.action_select_tab(2, window, cx);
            }))
            .on_action(cx.listener(|this: &mut Self, _: &SelectTab4, window, cx| {
                this.action_select_tab(3, window, cx);
            }))
            .on_action(cx.listener(|this: &mut Self, _: &SelectTab5, window, cx| {
                this.action_select_tab(4, window, cx);
            }))
            .on_action(cx.listener(|this: &mut Self, _: &SelectTab6, window, cx| {
                this.action_select_tab(5, window, cx);
            }))
            .on_action(cx.listener(|this: &mut Self, _: &SelectTab7, window, cx| {
                this.action_select_tab(6, window, cx);
            }))
            .on_action(cx.listener(|this: &mut Self, _: &SelectTab8, window, cx| {
                this.action_select_tab(7, window, cx);
            }))
            .on_action(cx.listener(|this: &mut Self, _: &SelectTab9, window, cx| {
                this.action_select_tab(8, window, cx);
            }))
            .child(self.dock_area.clone())
    }
}
