use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use gpui::*;
use gpui_component::dock::{DockArea, DockAreaState, DockItem, DockPlacement, PanelView, StackPanel, TabPanel, ToggleZoom};
use gpui_component::Placement;

use crux_ipc::IpcCommand;
use crux_protocol::{PaneEvent, PaneId};

use crate::actions::*;
use crate::dock::terminal_panel::CruxTerminalPanel;

/// Top-level application view managing the DockArea with terminal panels.
pub struct CruxApp {
    dock_area: Entity<DockArea>,
    /// Kept for socket cleanup on drop.
    _socket_path: Option<std::path::PathBuf>,
    pane_registry: HashMap<PaneId, Entity<CruxTerminalPanel>>,
    next_pane_id: AtomicU64,
    /// Buffer of pane lifecycle events for future consumers (IPC notifications, etc.).
    pane_events: Vec<PaneEvent>,
    /// Tracks which pane was split from which parent pane.
    pane_parents: HashMap<PaneId, PaneId>,
    /// Background MCP server process.
    mcp_process: Option<std::process::Child>,
}

impl CruxApp {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        // Start IPC server.
        let (socket_path, ipc_rx) = match crux_ipc::start_ipc() {
            Ok((path, rx, _cancel_token)) => {
                log::info!("IPC server started at {}", path.display());
                // SAFETY: Called during app initialization before any background threads are spawned.
                // No concurrent readers of this environment variable exist at this point.
                unsafe { std::env::set_var("CRUX_SOCKET", &path) };
                (Some(path), Some(rx))
            }
            Err(e) => {
                log::error!("Failed to start IPC server: {}", e);
                (None, None)
            }
        };

        // Spawn MCP server if IPC server started successfully.
        let mcp_process = if let Some(socket_path) = &socket_path {
            Self::spawn_mcp_server(socket_path)
        } else {
            None
        };

        let dock_area = cx.new(|cx| DockArea::new("crux-dock", Some(1), window, cx));

        // Create the initial terminal panel and register it.
        let mut pane_registry = HashMap::new();
        let pane_id = PaneId(0);
        let weak_dock = dock_area.downgrade();
        let initial_tab =
            cx.new(|cx| CruxTerminalPanel::new(pane_id, None, None, None, window, cx));
        pane_registry.insert(pane_id, initial_tab.clone());

        let dock_item = DockItem::tab(initial_tab, &weak_dock, window, cx);
        dock_area.update(cx, |area, cx| {
            area.set_center(dock_item, window, cx);
        });

        // Move IPC command processing to an async task instead of polling in render().
        if let Some(mut ipc_cmd_rx) = ipc_rx {
            cx.spawn_in(window, async move |this: WeakEntity<Self>, cx| {
                while let Some(cmd) = ipc_cmd_rx.recv().await {
                    let result = cx.update(|window, app| {
                        if let Some(entity) = this.upgrade() {
                            entity.update(app, |self_, ctx| {
                                self_.handle_ipc_command(cmd, window, ctx);
                                ctx.notify();
                            });
                        }
                    });
                    if result.is_err() {
                        break; // Window or entity dropped
                    }
                }
            })
            .detach();
        }

        Self {
            dock_area,
            _socket_path: socket_path,
            pane_registry,
            next_pane_id: AtomicU64::new(1),
            pane_events: Vec::new(),
            pane_parents: HashMap::new(),
            mcp_process,
        }
    }

    /// Attempt to spawn the crux-mcp binary next to the current executable.
    fn spawn_mcp_server(socket_path: &std::path::Path) -> Option<std::process::Child> {
        // Find the crux-mcp binary next to the current executable.
        let exe_path = match std::env::current_exe() {
            Ok(path) => path,
            Err(e) => {
                log::warn!("Failed to get current executable path: {}", e);
                return None;
            }
        };

        let mcp_binary = exe_path.parent()?.join("crux-mcp");
        if !mcp_binary.exists() {
            log::info!("MCP server binary not found at {}, skipping auto-launch", mcp_binary.display());
            return None;
        }

        // Spawn crux-mcp with --http mode (stdio doesn't work for child processes).
        match std::process::Command::new(&mcp_binary)
            .arg("--http")
            .arg("--socket")
            .arg(socket_path)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::inherit())
            .spawn()
        {
            Ok(child) => {
                log::info!("MCP server spawned at {} (PID {})", mcp_binary.display(), child.id());
                Some(child)
            }
            Err(e) => {
                log::warn!("Failed to spawn MCP server: {}", e);
                None
            }
        }
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
    fn focused_tab_panel(&self, window: &Window, cx: &App) -> Option<Entity<TabPanel>> {
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
        let pane_id = self.allocate_pane_id();
        if let Some(tab_panel) = self.focused_tab_panel(window, cx) {
            let panel = cx.new(|cx| CruxTerminalPanel::new(pane_id, None, None, None, window, cx));
            self.pane_registry.insert(pane_id, panel.clone());
            let panel_view: Arc<dyn PanelView> = Arc::new(panel);
            tab_panel.update(cx, |tp, cx| {
                tp.add_panel(panel_view, window, cx);
            });
        } else {
            let panel = cx.new(|cx| CruxTerminalPanel::new(pane_id, None, None, None, window, cx));
            self.pane_registry.insert(pane_id, panel.clone());
            let panel_view: Arc<dyn PanelView> = Arc::new(panel);
            self.dock_area.update(cx, |area, cx| {
                area.add_panel(panel_view, DockPlacement::Center, None, window, cx);
            });
        }
        self.emit_pane_event(PaneEvent::Created { pane_id });
    }

    fn action_close_tab(&mut self, _: &CloseTab, window: &mut Window, cx: &mut Context<Self>) {
        self.close_active_tab(false, window, cx);
    }

    fn action_force_close_tab(
        &mut self,
        _: &ForceCloseTab,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.close_active_tab(true, window, cx);
    }

    fn close_active_tab(&mut self, force: bool, window: &mut Window, cx: &mut Context<Self>) {
        let Some(tab_panel) = self.focused_tab_panel(window, cx) else {
            return;
        };

        let closing_pane_id = self.active_pane_id(window, cx);

        if !force {
            if let Some(pane_id) = closing_pane_id {
                if let Some(panel) = self.pane_registry.get(&pane_id).cloned() {
                    let running = panel.update(cx, |p, cx| p.is_process_running(cx));
                    if running {
                        log::warn!(
                            "Tab has a running process (pane {}), use Cmd+Shift+W to force close",
                            pane_id
                        );
                        return;
                    }
                }
            }
        }

        tab_panel.update(cx, |tp, cx| {
            if let Some(panel) = tp.active_panel(cx) {
                tp.remove_panel(panel, window, cx);
            }
        });

        if let Some(pane_id) = closing_pane_id {
            self.pane_registry.remove(&pane_id);
            self.pane_parents.remove(&pane_id);
            self.emit_pane_event(PaneEvent::Closed { pane_id });
        }
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

    fn action_select_tab(&mut self, index: usize, window: &mut Window, cx: &mut Context<Self>) {
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

    fn action_split_right(&mut self, _: &SplitRight, window: &mut Window, cx: &mut Context<Self>) {
        self.split_pane(Placement::Right, window, cx);
    }

    fn action_split_down(&mut self, _: &SplitDown, window: &mut Window, cx: &mut Context<Self>) {
        self.split_pane(Placement::Bottom, window, cx);
    }

    fn action_window_split_right(
        &mut self,
        _: &WindowSplitRight,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.window_split(Placement::Right, window, cx);
    }

    fn action_window_split_down(
        &mut self,
        _: &WindowSplitDown,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        self.window_split(Placement::Bottom, window, cx);
    }

    fn window_split(&mut self, placement: Placement, window: &mut Window, cx: &mut Context<Self>) {
        let pane_id = self.allocate_pane_id();
        let panel = cx.new(|cx| CruxTerminalPanel::new(pane_id, None, None, None, window, cx));
        self.pane_registry.insert(pane_id, panel.clone());

        let panel_view: Arc<dyn PanelView> = Arc::new(panel);
        let dock_placement = match placement {
            Placement::Right => DockPlacement::Right,
            Placement::Bottom => DockPlacement::Bottom,
            Placement::Left => DockPlacement::Left,
            // DockArea has no Top placement; fall back to Bottom.
            Placement::Top => DockPlacement::Bottom,
        };

        self.dock_area.update(cx, |area, cx| {
            area.add_panel(panel_view, dock_placement, None, window, cx);
        });

        self.emit_pane_event(PaneEvent::Created { pane_id });
    }

    fn split_pane(&mut self, placement: Placement, window: &mut Window, cx: &mut Context<Self>) {
        let Some(tab_panel) = self.focused_tab_panel(window, cx) else {
            return;
        };

        // Record the parent (the currently active pane being split from).
        let parent_pane_id = self.active_pane_id(window, cx);

        let pane_id = self.allocate_pane_id();
        let panel = cx.new(|cx| CruxTerminalPanel::new(pane_id, None, None, None, window, cx));
        self.pane_registry.insert(pane_id, panel.clone());
        let panel_view: Arc<dyn PanelView> = Arc::new(panel);

        tab_panel.update(cx, |tp, cx| {
            tp.add_panel_at(panel_view, placement, None, window, cx);
        });

        if let Some(parent_id) = parent_pane_id {
            self.pane_parents.insert(pane_id, parent_id);
        }
        self.emit_pane_event(PaneEvent::Created { pane_id });
    }

    fn action_zoom_pane(&mut self, _: &ZoomPane, window: &mut Window, cx: &mut Context<Self>) {
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

    fn action_prev_prompt(&mut self, _: &PrevPrompt, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(id) = self.active_pane_id(window, cx) {
            if let Some(panel) = self.pane_registry.get(&id).cloned() {
                panel.update(cx, |p, cx| {
                    p.scroll_to_prev_prompt(cx);
                });
            }
        }
    }

    fn action_next_prompt(&mut self, _: &NextPrompt, window: &mut Window, cx: &mut Context<Self>) {
        if let Some(id) = self.active_pane_id(window, cx) {
            if let Some(panel) = self.pane_registry.get(&id).cloned() {
                panel.update(cx, |p, cx| {
                    p.scroll_to_next_prompt(cx);
                });
            }
        }
    }

    fn cycle_pane_focus(&mut self, direction: isize, window: &mut Window, cx: &mut Context<Self>) {
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

        // Emit focused event for the newly focused pane.
        if let Some(pane_id) = self.active_pane_id(window, cx) {
            self.emit_pane_event(PaneEvent::Focused { pane_id });
        }
    }

    // -- IPC integration ---------------------------------------------------

    /// Allocate the next pane ID (atomic, safe for future non-&mut-self use).
    fn allocate_pane_id(&self) -> PaneId {
        let id = self.next_pane_id.fetch_add(1, Ordering::Relaxed);
        PaneId(id)
    }

    /// Push a pane lifecycle event into the buffer.
    fn emit_pane_event(&mut self, event: PaneEvent) {
        self.pane_events.push(event);
    }

    /// Drain all buffered pane events for consumption.
    #[allow(dead_code)]
    fn drain_pane_events(&mut self) -> Vec<PaneEvent> {
        std::mem::take(&mut self.pane_events)
    }

    /// Get the parent pane that a given pane was split from.
    #[allow(dead_code)]
    fn pane_parent(&self, pane_id: PaneId) -> Option<PaneId> {
        self.pane_parents.get(&pane_id).copied()
    }

    /// Get all panes that were split from a given parent pane.
    #[allow(dead_code)]
    fn pane_children(&self, pane_id: PaneId) -> Vec<PaneId> {
        self.pane_parents
            .iter()
            .filter_map(|(child, parent)| {
                if *parent == pane_id {
                    Some(*child)
                } else {
                    None
                }
            })
            .collect()
    }

    fn handle_ipc_command(&mut self, cmd: IpcCommand, window: &mut Window, cx: &mut Context<Self>) {
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
                let panel = cx.new(|cx| {
                    CruxTerminalPanel::new(
                        pane_id,
                        params.cwd.as_deref(),
                        params.command.as_deref(),
                        params.env.as_ref(),
                        window,
                        cx,
                    )
                });
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
                self.emit_pane_event(PaneEvent::Created { pane_id });

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
                let pane_id = params.pane_id.or_else(|| self.active_pane_id(window, cx));

                if let Some(id) = pane_id {
                    if let Some(panel) = self.pane_registry.get(&id).cloned() {
                        let text = params.text.as_bytes().to_vec();
                        let bracketed = params.bracketed_paste;
                        let len = text.len();
                        panel.update(cx, |p, cx| {
                            p.write_to_pty(&text, bracketed, cx);
                        });
                        let _ =
                            reply.send(Ok(crux_protocol::SendTextResult { bytes_written: len }));
                    } else {
                        let _ = reply.send(Err(anyhow::anyhow!("pane {} not found", id)));
                    }
                } else {
                    let _ = reply.send(Err(anyhow::anyhow!("no active pane")));
                }
            }

            IpcCommand::GetText { params, reply } => {
                let pane_id = params.pane_id.or_else(|| self.active_pane_id(window, cx));

                if let Some(id) = pane_id {
                    if let Some(panel) = self.pane_registry.get(&id) {
                        let (lines, cursor_row, cursor_col) = panel.read(cx).get_text(cx);
                        let result = crux_protocol::GetTextResult {
                            lines,
                            first_line: 0,
                            cursor_row,
                            cursor_col,
                        };
                        let _ = reply.send(Ok(result));
                    } else {
                        let _ = reply.send(Err(anyhow::anyhow!("pane {} not found", id)));
                    }
                } else {
                    let _ = reply.send(Err(anyhow::anyhow!("no active pane")));
                }
            }

            IpcCommand::GetSelection { params, reply } => {
                let pane_id = params.pane_id.or_else(|| self.active_pane_id(window, cx));

                if let Some(id) = pane_id {
                    if let Some(panel) = self.pane_registry.get(&id) {
                        let text = panel.read(cx).get_selection(cx);
                        let result = crux_protocol::GetSelectionResult {
                            has_selection: text.is_some(),
                            text,
                        };
                        let _ = reply.send(Ok(result));
                    } else {
                        let _ = reply.send(Err(anyhow::anyhow!("pane {} not found", id)));
                    }
                } else {
                    let _ = reply.send(Err(anyhow::anyhow!("no active pane")));
                }
            }

            IpcCommand::GetSnapshot { params, reply } => {
                let pane_id = params.pane_id.or_else(|| self.active_pane_id(window, cx));

                if let Some(id) = pane_id {
                    if let Some(panel) = self.pane_registry.get(&id) {
                        let result = panel.read(cx).get_snapshot(cx);
                        let _ = reply.send(Ok(result));
                    } else {
                        let _ = reply.send(Err(anyhow::anyhow!("pane {} not found", id)));
                    }
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
                            cursor_y: content.cursor.point.line.0 as u32,
                            tty: None,
                            pid: None,
                        }
                    })
                    .collect();
                let _ = reply.send(Ok(crux_protocol::ListPanesResult { panes }));
            }

            IpcCommand::ResizePane { params, reply } => {
                let result = self.handle_resize_pane(params.pane_id, params.width, params.height, window, cx);
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
                    self.emit_pane_event(PaneEvent::Closed {
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
        }
    }

    // -- IPC helpers -------------------------------------------------------

    /// Find the active pane ID (the one with focus).
    fn active_pane_id(&self, window: &Window, cx: &App) -> Option<PaneId> {
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
            return Err(anyhow::anyhow!("at least one of width or height must be specified"));
        }

        let panel_entity = self.pane_registry.get(&pane_id)
            .ok_or_else(|| anyhow::anyhow!("pane {} not found", pane_id))?;
        let target_view: Arc<dyn PanelView> = Arc::new(panel_entity.clone());

        let items = self.dock_area.read(cx).items().clone();

        // Try width resize (horizontal split axis)
        if let Some(w) = width {
            if let Some((stack_panel, ix)) = Self::find_stack_panel_containing(&items, &target_view, cx) {
                stack_panel.update(cx, |sp, cx| {
                    sp.resize_panel_at(ix, px(w), window, cx);
                });
            }
        }

        // Try height resize (vertical split axis)
        if let Some(h) = height {
            if let Some((stack_panel, ix)) = Self::find_stack_panel_containing(&items, &target_view, cx) {
                stack_panel.update(cx, |sp, cx| {
                    sp.resize_panel_at(ix, px(h), window, cx);
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
        match item {
            DockItem::Split { items, view, .. } => {
                // Check each child to see if it contains the target pane.
                for (ix, child) in items.iter().enumerate() {
                    if Self::dock_item_contains_pane(child, target, cx) {
                        // If the child directly contains the pane (e.g. it's a Tabs),
                        // return this split + index.
                        match child {
                            DockItem::Tabs { .. } | DockItem::Panel { .. } => {
                                return Some((view.clone(), ix));
                            }
                            DockItem::Split { .. } => {
                                // Recurse into nested splits — the pane might be deeper.
                                if let Some(result) = Self::find_stack_panel_containing(child, target, cx) {
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
    fn dock_item_contains_pane(item: &DockItem, target: &Arc<dyn PanelView>, _cx: &App) -> bool {
        match item {
            DockItem::Tabs { items, .. } => {
                items.iter().any(|panel| panel.view() == target.view())
            }
            DockItem::Split { items, .. } => {
                items.iter().any(|child| Self::dock_item_contains_pane(child, target, _cx))
            }
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

    // -- Session save/load -------------------------------------------------

    /// Default session file path: `~/.config/crux/session.json`.
    fn default_session_path() -> std::path::PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
        std::path::PathBuf::from(home)
            .join(".config")
            .join("crux")
            .join("session.json")
    }

    /// Save the current DockArea layout to a JSON file.
    fn handle_session_save(
        &self,
        path: Option<String>,
        cx: &App,
    ) -> anyhow::Result<crux_protocol::SessionSaveResult> {
        let file_path = match path {
            Some(p) => std::path::PathBuf::from(p),
            None => Self::default_session_path(),
        };

        let state = self.dock_area.read(cx).dump(cx);
        let json = serde_json::to_string_pretty(&state)?;

        if let Some(parent) = file_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&file_path, json)?;

        log::info!("Session saved to {}", file_path.display());
        Ok(crux_protocol::SessionSaveResult {
            path: file_path.to_string_lossy().to_string(),
        })
    }

    /// Load a DockArea layout from a JSON file and reconstruct panels.
    fn handle_session_load(
        &mut self,
        path: Option<String>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> anyhow::Result<crux_protocol::SessionLoadResult> {
        let file_path = match path {
            Some(p) => std::path::PathBuf::from(p),
            None => Self::default_session_path(),
        };

        let json = std::fs::read_to_string(&file_path)?;
        let state: DockAreaState = serde_json::from_str(&json)?;

        self.dock_area.update(cx, |area, cx| {
            area.load(state, window, cx)
        })?;

        // Re-populate pane_registry by walking the new DockItem tree.
        self.pane_registry.clear();
        self.pane_parents.clear();
        let items = self.dock_area.read(cx).items().clone();
        self.collect_panes_from_dock_item(&items, cx);

        // Set next_pane_id to max+1.
        let max_id = self.pane_registry.keys().map(|id| id.0).max().unwrap_or(0);
        self.next_pane_id.store(max_id + 1, Ordering::Relaxed);

        let pane_count = self.pane_registry.len() as u32;
        log::info!(
            "Session loaded from {} ({} panes)",
            file_path.display(),
            pane_count
        );
        Ok(crux_protocol::SessionLoadResult { pane_count })
    }

    /// Walk the DockItem tree and collect all CruxTerminalPanel entities
    /// into the pane_registry.
    fn collect_panes_from_dock_item(&mut self, item: &DockItem, cx: &App) {
        match item {
            DockItem::Tabs { items, .. } => {
                for panel_view in items {
                    if let Ok(terminal_panel) = panel_view.view().downcast::<CruxTerminalPanel>() {
                        let pane_id = terminal_panel.read(cx).pane_id();
                        self.pane_registry.insert(pane_id, terminal_panel);
                    }
                }
            }
            DockItem::Split { items, .. } => {
                for child in items {
                    self.collect_panes_from_dock_item(child, cx);
                }
            }
            DockItem::Panel { view, .. } => {
                if let Ok(terminal_panel) = view.view().downcast::<CruxTerminalPanel>() {
                    let pane_id = terminal_panel.read(cx).pane_id();
                    self.pane_registry.insert(pane_id, terminal_panel);
                }
            }
            _ => {}
        }
    }
}

impl Render for CruxApp {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .id("crux-app")
            .size_full()
            .on_action(cx.listener(Self::action_new_tab))
            .on_action(cx.listener(Self::action_close_tab))
            .on_action(cx.listener(Self::action_force_close_tab))
            .on_action(cx.listener(Self::action_next_tab))
            .on_action(cx.listener(Self::action_prev_tab))
            .on_action(cx.listener(Self::action_split_right))
            .on_action(cx.listener(Self::action_split_down))
            .on_action(cx.listener(Self::action_window_split_right))
            .on_action(cx.listener(Self::action_window_split_down))
            .on_action(cx.listener(Self::action_zoom_pane))
            .on_action(cx.listener(Self::action_focus_next_pane))
            .on_action(cx.listener(Self::action_focus_prev_pane))
            .on_action(cx.listener(Self::action_prev_prompt))
            .on_action(cx.listener(Self::action_next_prompt))
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

impl Drop for CruxApp {
    fn drop(&mut self) {
        if let Some(mut child) = self.mcp_process.take() {
            let pid = child.id();
            log::info!("Terminating MCP server (PID {pid})...");
            // Step 1: Send SIGTERM for graceful shutdown
            unsafe {
                libc::kill(pid as i32, libc::SIGTERM);
            }
            // Step 2: Wait up to 2 seconds
            for _ in 0..40 {
                match child.try_wait() {
                    Ok(Some(status)) => {
                        log::info!("MCP server exited gracefully: {status}");
                        return;
                    }
                    Ok(None) => std::thread::sleep(std::time::Duration::from_millis(50)),
                    Err(e) => {
                        log::warn!("Failed to check MCP server status: {e}");
                        break;
                    }
                }
            }
            // Step 3: Force kill
            log::warn!("MCP server did not exit gracefully, sending SIGKILL");
            let _ = child.kill();
            match child.wait() {
                Ok(status) => log::info!("MCP server force-killed: {status}"),
                Err(e) => log::warn!("Failed to wait for MCP server: {e}"),
            }
        }
    }
}
