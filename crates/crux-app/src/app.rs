use std::collections::{HashMap, VecDeque};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use gpui::*;
use gpui_component::dock::{
    DockArea, DockAreaState, DockItem, DockPlacement, PanelView, TabPanel, ToggleZoom,
};
use gpui_component::Placement;

use crux_config::CruxConfig;
use crux_protocol::{PaneEvent, PaneId};

use crate::actions::*;
use crate::dock::terminal_panel::CruxTerminalPanel;

/// Top-level application view managing the DockArea with terminal panels.
pub struct CruxApp {
    pub(crate) dock_area: Entity<DockArea>,
    /// Kept for socket cleanup on drop.
    _socket_path: Option<std::path::PathBuf>,
    /// IPC server cancellation token for graceful shutdown.
    ipc_cancel: Option<crux_ipc::CancellationToken>,
    pub(crate) pane_registry: HashMap<PaneId, Entity<CruxTerminalPanel>>,
    next_pane_id: AtomicU64,
    /// Buffer of pane lifecycle events for future consumers (IPC notifications, etc.).
    pane_events: VecDeque<PaneEvent>,
    /// Tracks which pane was split from which parent pane.
    pub(crate) pane_parents: HashMap<PaneId, PaneId>,
    /// Background MCP server process.
    mcp_process: Option<std::process::Child>,
    /// Application configuration loaded from config file.
    pub(crate) config: CruxConfig,
}

impl CruxApp {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        // Load configuration early.
        let config = CruxConfig::load().unwrap_or_else(|e| {
            log::warn!("Failed to load config: {}, using defaults", e);
            CruxConfig::default()
        });

        // Start IPC server.
        let (socket_path, ipc_rx, ipc_cancel) = match crux_ipc::start_ipc() {
            Ok((path, rx, cancel_token)) => {
                log::info!("IPC server started at {}", path.display());
                // SAFETY: Called during app initialization before any background threads are spawned.
                // No concurrent readers of this environment variable exist at this point.
                unsafe { std::env::set_var("CRUX_SOCKET", &path) };
                (Some(path), Some(rx), Some(cancel_token))
            }
            Err(e) => {
                log::error!("Failed to start IPC server: {}", e);
                (None, None, None)
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
            cx.new(|cx| {
                CruxTerminalPanel::new(
                    pane_id,
                    None,
                    None,
                    None,
                    config.font.clone(),
                    config.colors.clone(),
                    config.terminal.clone(),
                    window,
                    cx,
                )
            });
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
            ipc_cancel,
            pane_registry,
            next_pane_id: AtomicU64::new(1),
            pane_events: VecDeque::new(),
            pane_parents: HashMap::new(),
            mcp_process,
            config,
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
            log::info!(
                "MCP server binary not found at {}, skipping auto-launch",
                mcp_binary.display()
            );
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
                log::info!(
                    "MCP server spawned at {} (PID {})",
                    mcp_binary.display(),
                    child.id()
                );
                Some(child)
            }
            Err(e) => {
                log::warn!("Failed to spawn MCP server: {}", e);
                None
            }
        }
    }

    // -- Helpers --------------------------------------------------------

    /// Get a reference to the application configuration.
    #[allow(dead_code)]
    pub fn config(&self) -> &CruxConfig {
        &self.config
    }

    /// Collect all TabPanel entities from the DockItem tree in depth-first order.
    pub(crate) fn collect_tab_panels(item: &DockItem) -> Vec<Entity<TabPanel>> {
        let mut result = Vec::new();
        Self::collect_tab_panels_recursive(item, &mut result, 0);
        result
    }

    fn collect_tab_panels_recursive(item: &DockItem, out: &mut Vec<Entity<TabPanel>>, depth: usize) {
        const MAX_DOCK_DEPTH: usize = 100;
        if depth > MAX_DOCK_DEPTH {
            log::warn!("collect_tab_panels_recursive: max depth {} exceeded, stopping recursion", MAX_DOCK_DEPTH);
            return;
        }
        match item {
            DockItem::Tabs { view, .. } => {
                out.push(view.clone());
            }
            DockItem::Split { items, .. } => {
                for child in items {
                    Self::collect_tab_panels_recursive(child, out, depth + 1);
                }
            }
            _ => {}
        }
    }

    /// Find the focused TabPanel among all center tab panels.
    pub(crate) fn focused_tab_panel(&self, window: &Window, cx: &App) -> Option<Entity<TabPanel>> {
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
            let panel = cx.new(|cx| {
                CruxTerminalPanel::new(
                    pane_id,
                    None,
                    None,
                    None,
                    self.config.font.clone(),
                    self.config.colors.clone(),
                    self.config.terminal.clone(),
                    window,
                    cx,
                )
            });
            self.pane_registry.insert(pane_id, panel.clone());
            let panel_view: Arc<dyn PanelView> = Arc::new(panel);
            tab_panel.update(cx, |tp, cx| {
                tp.add_panel(panel_view, window, cx);
            });
        } else {
            let panel = cx.new(|cx| {
                CruxTerminalPanel::new(
                    pane_id,
                    None,
                    None,
                    None,
                    self.config.font.clone(),
                    self.config.colors.clone(),
                    self.config.terminal.clone(),
                    window,
                    cx,
                )
            });
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
        let panel = cx.new(|cx| {
                CruxTerminalPanel::new(
                    pane_id,
                    None,
                    None,
                    None,
                    self.config.font.clone(),
                    self.config.colors.clone(),
                    self.config.terminal.clone(),
                    window,
                    cx,
                )
            });
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
        let panel = cx.new(|cx| {
                CruxTerminalPanel::new(
                    pane_id,
                    None,
                    None,
                    None,
                    self.config.font.clone(),
                    self.config.colors.clone(),
                    self.config.terminal.clone(),
                    window,
                    cx,
                )
            });
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
    pub(crate) fn allocate_pane_id(&self) -> PaneId {
        let id = self.next_pane_id.fetch_add(1, Ordering::Relaxed);
        PaneId(id)
    }

    /// Push a pane lifecycle event into the buffer.
    pub(crate) fn emit_pane_event(&mut self, event: PaneEvent) {
        const MAX_PANE_EVENTS: usize = 10_000;
        if self.pane_events.len() >= MAX_PANE_EVENTS {
            log::warn!("pane event buffer full ({}), dropping oldest event", MAX_PANE_EVENTS);
            self.pane_events.pop_front();
        }
        self.pane_events.push_back(event);
    }

    /// Drain all buffered pane events for consumption.
    pub(crate) fn drain_pane_events(&mut self) -> Vec<PaneEvent> {
        self.pane_events.drain(..).collect()
    }

    /// Get the parent pane that a given pane was split from.
    // TODO: Used by future pane tree navigation (Phase 2 split pane features)
    #[allow(dead_code)]
    fn pane_parent(&self, pane_id: PaneId) -> Option<PaneId> {
        self.pane_parents.get(&pane_id).copied()
    }

    /// Get all panes that were split from a given parent pane.
    // TODO: Used by future pane tree navigation (Phase 2 split pane features)
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

    // IPC command dispatch is in ipc_dispatch.rs

    // -- Session save/load -------------------------------------------------

    /// Default session file path: `~/.config/crux/session.json`.
    fn default_session_path() -> std::path::PathBuf {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        std::path::PathBuf::from(home)
            .join(".config")
            .join("crux")
            .join("session.json")
    }

    /// Save the current DockArea layout to a JSON file.
    pub(crate) fn handle_session_save(
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
    pub(crate) fn handle_session_load(
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

        self.dock_area
            .update(cx, |area, cx| area.load(state, window, cx))?;

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
        self.collect_panes_from_dock_item_recursive(item, cx, 0);
    }

    fn collect_panes_from_dock_item_recursive(&mut self, item: &DockItem, cx: &App, depth: usize) {
        const MAX_DOCK_DEPTH: usize = 100;
        if depth > MAX_DOCK_DEPTH {
            log::warn!("collect_panes_from_dock_item: max depth {} exceeded, stopping recursion", MAX_DOCK_DEPTH);
            return;
        }
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
                    self.collect_panes_from_dock_item_recursive(child, cx, depth + 1);
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
        // Cancel IPC server gracefully
        if let Some(cancel_token) = self.ipc_cancel.take() {
            log::info!("Cancelling IPC server...");
            cancel_token.cancel();
        }

        if let Some(mut child) = self.mcp_process.take() {
            let pid = child.id();
            log::info!("Terminating MCP server (PID {pid})...");
            // Step 1: Send SIGTERM for graceful shutdown
            // Safe cast: PIDs on macOS/Linux are always within i32 range.
            let pid_i32 = i32::try_from(pid).expect("PID exceeds i32::MAX");
            unsafe {
                libc::kill(pid_i32, libc::SIGTERM);
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
