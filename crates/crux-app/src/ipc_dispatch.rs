use std::sync::Arc;

use gpui::*;
use gpui_component::dock::{DockItem, PanelView, StackPanel, TabPanel};
use gpui_component::Placement;

use crux_ipc::IpcCommand;
use crux_protocol::PaneId;

use crate::app::CruxApp;
use crate::dock::terminal_panel::CruxTerminalPanel;

impl CruxApp {
    /// Resolve a pane ID from an optional parameter, falling back to the active pane.
    /// Returns the `(PaneId, Entity<CruxTerminalPanel>)` pair or `None`.
    pub(crate) fn resolve_pane(
        &self,
        pane_id: Option<PaneId>,
        window: &Window,
        cx: &App,
    ) -> Option<(PaneId, Entity<CruxTerminalPanel>)> {
        let id = pane_id.or_else(|| self.active_pane_id(window, cx))?;
        let panel = self.pane_registry.get(&id)?.clone();
        Some((id, panel))
    }

    pub(crate) fn handle_ipc_command(
        &mut self,
        cmd: IpcCommand,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match cmd {
            IpcCommand::Handshake { params: _, reply } => {
                let result = crux_protocol::HandshakeResult {
                    server_name: "crux".into(),
                    server_version: env!("CARGO_PKG_VERSION").into(),
                    protocol_version: "1.0".into(),
                    supported_capabilities: vec!["pane".into()],
                };
                let _ = reply.send(Ok(result));
            }

            IpcCommand::SplitPane { params, reply } => {
                // Determine the parent pane (target or currently active).
                let parent_pane_id = params
                    .target_pane_id
                    .or_else(|| self.active_pane_id(window, cx));

                let pane_id = self.allocate_pane_id();
                let panel = self.create_terminal_panel(
                    pane_id,
                    params.cwd.as_deref(),
                    params.command.as_deref(),
                    params.env.as_ref(),
                    window,
                    cx,
                );
                self.pane_registry.insert(pane_id, panel.clone());

                // Find the target tab panel.
                let target_tp = if let Some(target_id) = params.target_pane_id {
                    self.find_tab_panel_for_pane(target_id, window, cx)
                } else {
                    self.focused_tab_panel(window, cx)
                };

                let placement = match params.direction {
                    crux_protocol::SplitDirection::Right => Placement::Right,
                    crux_protocol::SplitDirection::Left => Placement::Left,
                    crux_protocol::SplitDirection::Top => Placement::Top,
                    crux_protocol::SplitDirection::Bottom => Placement::Bottom,
                };

                if let Some(tp) = target_tp {
                    let panel_view: Arc<dyn PanelView> = Arc::new(panel.clone());
                    tp.update(cx, |tp, cx| {
                        tp.add_panel_at(panel_view, placement, None, window, cx);
                    });
                }

                if let Some(parent_id) = parent_pane_id {
                    self.pane_parents.insert(pane_id, parent_id);
                }
                self.emit_pane_event(crux_protocol::PaneEvent::Created { pane_id });

                let size = panel.read(cx).terminal_view_size(cx);
                let result = crux_protocol::SplitPaneResult {
                    pane_id,
                    window_id: crux_protocol::WindowId(0),
                    tab_id: crux_protocol::TabId(0),
                    size: crux_protocol::PaneSize {
                        rows: size.0,
                        cols: size.1,
                    },
                    tty: None,
                };
                let _ = reply.send(Ok(result));
            }

            IpcCommand::SendText { params, reply } => {
                if let Some((_id, panel)) = self.resolve_pane(params.pane_id, window, cx) {
                    let text = params.text.as_bytes().to_vec();
                    let bracketed = params.bracketed_paste;
                    let len = text.len();
                    panel.update(cx, |p, cx| {
                        p.write_to_pty(&text, bracketed, cx);
                    });
                    let _ = reply.send(Ok(crux_protocol::SendTextResult { bytes_written: len }));
                } else if let Some(id) = params.pane_id {
                    let _ = reply.send(Err(anyhow::anyhow!("pane {} not found", id)));
                } else {
                    let _ = reply.send(Err(anyhow::anyhow!("no active pane")));
                }
            }

            IpcCommand::GetText { params, reply } => {
                if let Some((_id, panel)) = self.resolve_pane(params.pane_id, window, cx) {
                    let (lines, cursor_row, cursor_col) = panel.read(cx).get_text(cx);
                    let result = crux_protocol::GetTextResult {
                        lines,
                        first_line: 0,
                        cursor_row,
                        cursor_col,
                    };
                    let _ = reply.send(Ok(result));
                } else if let Some(id) = params.pane_id {
                    let _ = reply.send(Err(anyhow::anyhow!("pane {} not found", id)));
                } else {
                    let _ = reply.send(Err(anyhow::anyhow!("no active pane")));
                }
            }

            IpcCommand::GetSelection { params, reply } => {
                if let Some((_id, panel)) = self.resolve_pane(params.pane_id, window, cx) {
                    let text = panel.read(cx).get_selection(cx);
                    let result = crux_protocol::GetSelectionResult {
                        has_selection: text.is_some(),
                        text,
                    };
                    let _ = reply.send(Ok(result));
                } else if let Some(id) = params.pane_id {
                    let _ = reply.send(Err(anyhow::anyhow!("pane {} not found", id)));
                } else {
                    let _ = reply.send(Err(anyhow::anyhow!("no active pane")));
                }
            }

            IpcCommand::GetSnapshot { params, reply } => {
                if let Some((_id, panel)) = self.resolve_pane(params.pane_id, window, cx) {
                    let result = panel.read(cx).get_snapshot(cx);
                    let _ = reply.send(Ok(result));
                } else if let Some(id) = params.pane_id {
                    let _ = reply.send(Err(anyhow::anyhow!("pane {} not found", id)));
                } else {
                    let _ = reply.send(Err(anyhow::anyhow!("no active pane")));
                }
            }

            IpcCommand::ListPanes { reply } => {
                let panes: Vec<crux_protocol::PaneInfo> = self
                    .pane_registry
                    .iter()
                    .map(|(id, panel)| {
                        let p = panel.read(cx);
                        let view = p.terminal_view().read(cx);
                        let size = view.terminal_size();
                        let content = view.terminal_content_snapshot();
                        crux_protocol::PaneInfo {
                            pane_id: *id,
                            window_id: crux_protocol::WindowId(0),
                            tab_id: crux_protocol::TabId(0),
                            size: crux_protocol::PaneSize {
                                rows: size.rows as u32,
                                cols: size.cols as u32,
                            },
                            title: view.title().unwrap_or("").to_string(),
                            cwd: view.cwd().map(|s| s.to_string()),
                            is_active: self.is_pane_active(*id, window, cx),
                            is_zoomed: false,
                            cursor_x: content.cursor.point.column.0 as u32,
                            cursor_y: content.cursor.point.line.0.max(0) as u32,
                            tty: None,
                            pid: None,
                        }
                    })
                    .collect();
                let _ = reply.send(Ok(crux_protocol::ListPanesResult { panes }));
            }

            IpcCommand::ResizePane { params, reply } => {
                let result = self.handle_resize_pane(
                    params.pane_id,
                    params.width,
                    params.height,
                    window,
                    cx,
                );
                let _ = reply.send(result);
            }

            IpcCommand::ActivatePane { params, reply } => {
                if let Some(panel) = self.pane_registry.get(&params.pane_id) {
                    let fh = panel.read(cx).focus_handle(cx);
                    fh.focus(window);
                    let _ = reply.send(Ok(()));
                } else {
                    let _ = reply.send(Err(anyhow::anyhow!("pane {} not found", params.pane_id)));
                }
            }

            IpcCommand::ClosePane { params, reply } => {
                if let Some(panel) = self.pane_registry.get(&params.pane_id).cloned() {
                    // When force is false, check if the process is still running.
                    if !params.force {
                        let running = panel.update(cx, |p, cx| p.is_process_running(cx));
                        if running {
                            let _ = reply.send(Err(anyhow::anyhow!(
                                "pane {} has a running process, use force: true to close",
                                params.pane_id
                            )));
                            return;
                        }
                    }

                    self.pane_registry.remove(&params.pane_id);
                    self.pane_parents.remove(&params.pane_id);
                    let items = self.dock_area.read(cx).items().clone();
                    let tab_panels = Self::collect_tab_panels(&items);
                    let panel_view: Arc<dyn PanelView> = Arc::new(panel);
                    for tp in tab_panels {
                        tp.update(cx, |tp, cx| {
                            tp.remove_panel(panel_view.clone(), window, cx);
                        });
                    }
                    self.emit_pane_event(crux_protocol::PaneEvent::Closed {
                        pane_id: params.pane_id,
                    });
                    let _ = reply.send(Ok(()));
                } else {
                    let _ = reply.send(Err(anyhow::anyhow!("pane {} not found", params.pane_id)));
                }
            }

            IpcCommand::WindowCreate { params: _, reply } => {
                // Single-window mode: return the existing window.
                let result = crux_protocol::WindowCreateResult {
                    window_id: crux_protocol::WindowId(0),
                };
                let _ = reply.send(Ok(result));
            }

            IpcCommand::WindowList { reply } => {
                let pane_count = self.pane_registry.len() as u32;
                let window_info = crux_protocol::WindowInfo {
                    window_id: crux_protocol::WindowId(0),
                    title: "Crux".to_string(),
                    pane_count,
                    is_focused: true,
                };
                let result = crux_protocol::WindowListResult {
                    windows: vec![window_info],
                };
                let _ = reply.send(Ok(result));
            }

            IpcCommand::SessionSave { params, reply } => {
                let result = self.handle_session_save(params.path, cx);
                let _ = reply.send(result);
            }

            IpcCommand::SessionLoad { params, reply } => {
                let result = self.handle_session_load(params.path, window, cx);
                let _ = reply.send(result);
            }

            IpcCommand::ClipboardRead { params, reply } => {
                #[cfg(target_os = "macos")]
                {
                    if let Some(mtm) = objc2_foundation::MainThreadMarker::new() {
                        let result = match crux_clipboard::Clipboard::read(mtm) {
                            Ok(content) => {
                                let read_result = match content {
                                    crux_clipboard::ClipboardContent::Text(text) => {
                                        if params.content_type == crux_protocol::ClipboardContentType::Image {
                                            Err(anyhow::anyhow!("no image in clipboard"))
                                        } else {
                                            Ok(crux_protocol::ClipboardReadResult::Text { text })
                                        }
                                    }
                                    crux_clipboard::ClipboardContent::Html(html) => {
                                        Ok(crux_protocol::ClipboardReadResult::Html { html })
                                    }
                                    crux_clipboard::ClipboardContent::Image { png_data } => {
                                        match crux_clipboard::save_image_to_temp(&png_data) {
                                            Ok(path) => {
                                                Ok(crux_protocol::ClipboardReadResult::Image {
                                                    image_path: path.to_string_lossy().to_string(),
                                                })
                                            }
                                            Err(e) => Err(anyhow::anyhow!("{e}")),
                                        }
                                    }
                                    crux_clipboard::ClipboardContent::FilePaths(paths) => {
                                        Ok(crux_protocol::ClipboardReadResult::FilePaths {
                                            paths: paths
                                                .iter()
                                                .map(|p| p.to_string_lossy().to_string())
                                                .collect(),
                                        })
                                    }
                                };
                                read_result
                            }
                            Err(e) => Err(anyhow::anyhow!("{e}")),
                        };
                        let _ = reply.send(result);
                    } else {
                        let _ = reply.send(Err(anyhow::anyhow!("not on main thread")));
                    }
                }
                #[cfg(not(target_os = "macos"))]
                {
                    let _ = params;
                    let _ = reply.send(Err(anyhow::anyhow!(
                        "clipboard not supported on this platform"
                    )));
                }
            }

            IpcCommand::ClipboardWrite { params, reply } => {
                #[cfg(target_os = "macos")]
                {
                    if let Some(mtm) = objc2_foundation::MainThreadMarker::new() {
                        let result = match params.content_type {
                            crux_protocol::ClipboardContentType::Text => {
                                if let Some(text) = &params.text {
                                    crux_clipboard::Clipboard::write_text(text, mtm)
                                        .map_err(|e| anyhow::anyhow!("{e}"))
                                } else {
                                    Err(anyhow::anyhow!(
                                        "text field required for content_type 'text'"
                                    ))
                                }
                            }
                            crux_protocol::ClipboardContentType::Image => {
                                if let Some(path) = &params.image_path {
                                    let path_obj = std::path::Path::new(path);
                                    let ext = path_obj.extension().and_then(|e| e.to_str()).unwrap_or("");
                                    if !matches!(ext.to_lowercase().as_str(), "png" | "jpg" | "jpeg" | "gif" | "bmp" | "webp" | "tiff") {
                                        Err(anyhow::anyhow!("image path must have a valid image extension"))
                                    } else {
                                        match std::fs::read(path) {
                                            Ok(data) => {
                                                crux_clipboard::Clipboard::write_image(&data, mtm)
                                                    .map_err(|e| anyhow::anyhow!("{e}"))
                                            }
                                            Err(e) => Err(anyhow::anyhow!("failed to read image: {e}")),
                                        }
                                    }
                                } else {
                                    Err(anyhow::anyhow!(
                                        "image_path field required for content_type 'image'"
                                    ))
                                }
                            }
                            crux_protocol::ClipboardContentType::Auto => Err(anyhow::anyhow!("content_type 'auto' not supported for clipboard write"))
                        };
                        let _ = reply.send(result);
                    } else {
                        let _ = reply.send(Err(anyhow::anyhow!("not on main thread")));
                    }
                }
                #[cfg(not(target_os = "macos"))]
                {
                    let _ = params;
                    let _ = reply.send(Err(anyhow::anyhow!(
                        "clipboard not supported on this platform"
                    )));
                }
            }

            IpcCommand::ImeGetState { reply } => {
                let composing = self
                    .active_pane_id(window, cx)
                    .and_then(|id| self.pane_registry.get(&id))
                    .map(|panel| panel.read(cx).is_composing(cx))
                    .unwrap_or(false);

                #[cfg(target_os = "macos")]
                let input_source = crux_terminal_view::ime_switch::current_input_source();
                #[cfg(not(target_os = "macos"))]
                let input_source = None;

                let state = crux_protocol::ImeStateResult {
                    composing,
                    preedit_text: None, // Privacy: never expose composition text via IPC
                    input_source,
                };
                let _ = reply.send(Ok(state));
            }

            IpcCommand::ImeSetInputSource { params, reply } => {
                #[cfg(target_os = "macos")]
                {
                    let success = crux_terminal_view::ime_switch::switch_to_input_source(
                        &params.input_source,
                    );
                    if success {
                        let _ = reply.send(Ok(()));
                    } else {
                        let _ = reply.send(Err(anyhow::anyhow!(
                            "input source not found: {}",
                            params.input_source
                        )));
                    }
                }
                #[cfg(not(target_os = "macos"))]
                {
                    let _ = params;
                    let _ = reply.send(Err(anyhow::anyhow!(
                        "IME switching not supported on this platform"
                    )));
                }
            }

            IpcCommand::EventsPoll { reply } => {
                let events = self.drain_pane_events();
                let _ = reply.send(Ok(crux_protocol::EventsPollResult { events }));
            }
        }
    }

    // -- IPC helpers -------------------------------------------------------

    /// Find the active pane ID (the one with focus).
    pub(crate) fn active_pane_id(&self, window: &Window, cx: &App) -> Option<PaneId> {
        for (id, panel) in &self.pane_registry {
            let fh = panel.read(cx).focus_handle(cx);
            if fh.contains_focused(window, cx) {
                return Some(*id);
            }
        }
        // Fallback: return the first pane.
        self.pane_registry.keys().next().copied()
    }

    /// Check if a pane currently has focus.
    fn is_pane_active(&self, pane_id: PaneId, window: &Window, cx: &App) -> bool {
        if let Some(panel) = self.pane_registry.get(&pane_id) {
            let fh = panel.read(cx).focus_handle(cx);
            fh.contains_focused(window, cx)
        } else {
            false
        }
    }

    /// Handle a pane resize request by navigating the DockItem tree to find
    /// the StackPanel that contains the target pane and resizing it.
    fn handle_resize_pane(
        &self,
        pane_id: PaneId,
        width: Option<f32>,
        height: Option<f32>,
        window: &mut Window,
        cx: &mut App,
    ) -> anyhow::Result<()> {
        if width.is_none() && height.is_none() {
            return Err(anyhow::anyhow!(
                "at least one of width or height must be specified"
            ));
        }

        let panel_entity = self
            .pane_registry
            .get(&pane_id)
            .ok_or_else(|| anyhow::anyhow!("pane {} not found", pane_id))?;
        let target_view: Arc<dyn PanelView> = Arc::new(panel_entity.clone());

        let items = self.dock_area.read(cx).items().clone();

        if width.is_some() || height.is_some() {
            if let Some((stack_panel, ix)) =
                Self::find_stack_panel_containing(&items, &target_view, cx)
            {
                stack_panel.update(cx, |sp, cx| {
                    if let Some(w) = width {
                        sp.resize_panel_at(ix, px(w), window, cx);
                    }
                    if let Some(h) = height {
                        sp.resize_panel_at(ix, px(h), window, cx);
                    }
                });
            }
        }

        Ok(())
    }

    /// Walk the DockItem tree to find which StackPanel (DockItem::Split) contains
    /// a TabPanel that holds the target pane. Returns the StackPanel entity and the
    /// index of the child item within it.
    fn find_stack_panel_containing(
        item: &DockItem,
        target: &Arc<dyn PanelView>,
        cx: &App,
    ) -> Option<(Entity<StackPanel>, usize)> {
        Self::find_stack_panel_containing_recursive(item, target, cx, 0)
    }

    fn find_stack_panel_containing_recursive(
        item: &DockItem,
        target: &Arc<dyn PanelView>,
        cx: &App,
        depth: usize,
    ) -> Option<(Entity<StackPanel>, usize)> {
        if depth > crate::app::MAX_DOCK_DEPTH {
            log::warn!("find_stack_panel_containing: max depth {} exceeded, stopping recursion", crate::app::MAX_DOCK_DEPTH);
            return None;
        }
        match item {
            DockItem::Split { items, view, .. } => {
                // Check each child to see if it contains the target pane.
                for (ix, child) in items.iter().enumerate() {
                    if Self::dock_item_contains_pane(child, target, cx, 0) {
                        // If the child directly contains the pane (e.g. it's a Tabs),
                        // return this split + index.
                        match child {
                            DockItem::Tabs { .. } | DockItem::Panel { .. } => {
                                return Some((view.clone(), ix));
                            }
                            DockItem::Split { .. } => {
                                // Recurse into nested splits — the pane might be deeper.
                                if let Some(result) =
                                    Self::find_stack_panel_containing_recursive(child, target, cx, depth + 1)
                                {
                                    return Some(result);
                                }
                                // If not found deeper, the target is directly in this split.
                                return Some((view.clone(), ix));
                            }
                            _ => {}
                        }
                    }
                }
                None
            }
            _ => None,
        }
    }

    /// Check if a DockItem (recursively) contains the target pane view.
    fn dock_item_contains_pane(item: &DockItem, target: &Arc<dyn PanelView>, _cx: &App, depth: usize) -> bool {
        if depth > crate::app::MAX_DOCK_DEPTH {
            log::warn!("dock_item_contains_pane: max depth {} exceeded, stopping recursion", crate::app::MAX_DOCK_DEPTH);
            return false;
        }
        match item {
            DockItem::Tabs { items, .. } => items.iter().any(|panel| panel.view() == target.view()),
            DockItem::Split { items, .. } => items
                .iter()
                .any(|child| Self::dock_item_contains_pane(child, target, _cx, depth + 1)),
            DockItem::Panel { view, .. } => view.view() == target.view(),
            _ => false,
        }
    }

    /// Find the TabPanel that contains the given pane.
    fn find_tab_panel_for_pane(
        &self,
        pane_id: PaneId,
        window: &Window,
        cx: &App,
    ) -> Option<Entity<TabPanel>> {
        let panel_entity = self.pane_registry.get(&pane_id)?;
        let target_view = (Arc::new(panel_entity.clone()) as Arc<dyn PanelView>).view();

        let items = self.dock_area.read(cx).items();
        let tab_panels = Self::collect_tab_panels(items);

        // Check if the target pane is the active panel in any TabPanel.
        for tp in &tab_panels {
            if let Some(active) = tp.read(cx).active_panel(cx) {
                if active.view() == target_view {
                    return Some(tp.clone());
                }
            }
        }

        // Check by focus handle — if the target pane is focused,
        // find which TabPanel contains that focus.
        let panel_fh = panel_entity.read(cx).focus_handle(cx);
        if panel_fh.contains_focused(window, cx) {
            for tp in &tab_panels {
                let tp_fh = tp.read(cx).focus_handle(cx);
                if tp_fh.contains_focused(window, cx) {
                    return Some(tp.clone());
                }
            }
        }

        // Fallback to the focused tab panel.
        self.focused_tab_panel(window, cx)
    }
}

#[cfg(test)]
mod tests {
    use crux_protocol::*;

    #[test]
    fn test_protocol_types_have_correct_fields() {
        let handshake = HandshakeParams {
            client_name: "test".into(),
            client_version: "1.0".into(),
            protocol_version: "1.0".into(),
            capabilities: vec![],
        };
        assert_eq!(handshake.client_name, "test");
        assert_eq!(handshake.client_version, "1.0");

        let split = SplitPaneParams {
            direction: SplitDirection::Right,
            target_pane_id: None,
            size: None,
            cwd: None,
            command: None,
            env: None,
        };
        assert!(matches!(split.direction, SplitDirection::Right));

        let send_text = SendTextParams {
            pane_id: None,
            text: "hello".into(),
            bracketed_paste: false,
        };
        assert_eq!(send_text.text, "hello");
        assert!(!send_text.bracketed_paste);

        let get_text = GetTextParams {
            pane_id: None,
            start_line: None,
            end_line: None,
            include_escapes: false,
        };
        assert!(get_text.pane_id.is_none());
    }

    #[test]
    fn test_id_types() {
        let pane_id = PaneId(42);
        assert_eq!(pane_id.0, 42);

        let window_id = WindowId(1);
        assert_eq!(window_id.0, 1);

        let tab_id = TabId(2);
        assert_eq!(tab_id.0, 2);
    }

    #[test]
    fn test_split_direction_variants() {
        let _directions = [
            SplitDirection::Right,
            SplitDirection::Left,
            SplitDirection::Top,
            SplitDirection::Bottom,
        ];
    }

    #[test]
    fn test_pane_size_construction() {
        let size = PaneSize { rows: 24, cols: 80 };
        assert_eq!(size.rows, 24);
        assert_eq!(size.cols, 80);
    }

    #[test]
    fn test_clipboard_params() {
        let read_params = ClipboardReadParams {
            content_type: crux_protocol::ClipboardContentType::Text,
        };
        assert_eq!(read_params.content_type, crux_protocol::ClipboardContentType::Text);

        let write_params = ClipboardWriteParams {
            content_type: crux_protocol::ClipboardContentType::Text,
            text: Some("hello".into()),
            image_path: None,
        };
        assert_eq!(write_params.content_type, crux_protocol::ClipboardContentType::Text);
        assert_eq!(write_params.text.as_deref(), Some("hello"));
        assert!(write_params.image_path.is_none());
    }

    #[test]
    fn test_ime_set_input_source_params() {
        let params = ImeSetInputSourceParams {
            input_source: "com.apple.inputmethod.Korean.2SetKorean".into(),
        };
        assert_eq!(
            params.input_source,
            "com.apple.inputmethod.Korean.2SetKorean"
        );
    }
}
