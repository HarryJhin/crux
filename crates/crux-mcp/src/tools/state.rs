use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::*;
use rmcp::{schemars, tool, tool_router, ErrorData as McpError};

use crate::server::CruxMcpServer;

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct PaneIdParam {
    /// Pane ID (uses active pane if omitted)
    pub pane_id: Option<u64>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct ScrollbackParams {
    /// Pane ID (uses active pane if omitted)
    pub pane_id: Option<u64>,
    /// Starting line offset (negative for scrollback)
    pub offset: Option<i32>,
    /// Number of lines to retrieve
    pub limit: Option<i32>,
}

pub(crate) fn router() -> rmcp::handler::server::router::tool::ToolRouter<CruxMcpServer> {
    CruxMcpServer::state_tools()
}

#[tool_router(router = state_tools)]
impl CruxMcpServer {
    /// Get the current working directory of a terminal pane.
    #[tool(description = "Get the current working directory of a terminal pane")]
    async fn crux_get_current_directory(
        &self,
        Parameters(params): Parameters<PaneIdParam>,
    ) -> Result<CallToolResult, McpError> {
        let ipc = self.ipc.clone();
        let result = tokio::task::spawn_blocking(move || {
            ipc.call(crux_protocol::method::PANE_LIST, serde_json::json!({}))
        })
        .await
        .map_err(|e| McpError::internal_error(format!("task join error: {e}"), None))?
        .map_err(|e| McpError::internal_error(format!("IPC error: {e}"), None))?;

        let pane = find_pane(&result, params.pane_id)?;
        let cwd = pane
            .get("cwd")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");

        Ok(CallToolResult::success(vec![Content::text(cwd)]))
    }

    /// Get the running process info for a terminal pane.
    #[tool(description = "Get the running process PID in a terminal pane")]
    async fn crux_get_running_process(
        &self,
        Parameters(params): Parameters<PaneIdParam>,
    ) -> Result<CallToolResult, McpError> {
        let ipc = self.ipc.clone();
        let result = tokio::task::spawn_blocking(move || {
            ipc.call(crux_protocol::method::PANE_LIST, serde_json::json!({}))
        })
        .await
        .map_err(|e| McpError::internal_error(format!("task join error: {e}"), None))?
        .map_err(|e| McpError::internal_error(format!("IPC error: {e}"), None))?;

        let pane = find_pane(&result, params.pane_id)?;
        let pid = pane.get("pid");

        let output = match pid {
            Some(serde_json::Value::Number(n)) => format!("PID: {n}"),
            Some(serde_json::Value::Null) | None => "no foreground process info available".into(),
            Some(v) => format!("PID: {v}"),
        };

        Ok(CallToolResult::success(vec![Content::text(output)]))
    }

    /// Get full state of a terminal pane.
    #[tool(
        description = "Get the full state of a terminal pane including size, title, cursor position"
    )]
    async fn crux_get_pane_state(
        &self,
        Parameters(params): Parameters<PaneIdParam>,
    ) -> Result<CallToolResult, McpError> {
        let ipc = self.ipc.clone();
        let result = tokio::task::spawn_blocking(move || {
            ipc.call(crux_protocol::method::PANE_LIST, serde_json::json!({}))
        })
        .await
        .map_err(|e| McpError::internal_error(format!("task join error: {e}"), None))?
        .map_err(|e| McpError::internal_error(format!("IPC error: {e}"), None))?;

        let pane = find_pane(&result, params.pane_id)?;

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&pane).unwrap_or_else(|_| pane.to_string()),
        )]))
    }

    /// Get the currently selected text in a terminal pane.
    #[tool(description = "Get the currently selected text in a terminal pane (not yet supported)")]
    async fn crux_get_selection(
        &self,
        Parameters(_params): Parameters<PaneIdParam>,
    ) -> Result<CallToolResult, McpError> {
        Ok(CallToolResult::success(vec![Content::text(
            "selection retrieval is not yet supported",
        )]))
    }

    /// Get scrollback buffer content from a terminal pane.
    #[tool(
        description = "Get scrollback buffer content from a terminal pane with optional line range"
    )]
    async fn crux_get_scrollback(
        &self,
        Parameters(params): Parameters<ScrollbackParams>,
    ) -> Result<CallToolResult, McpError> {
        let ipc = self.ipc.clone();
        let result = tokio::task::spawn_blocking(move || {
            let mut p = serde_json::json!({ "pane_id": params.pane_id });
            if let Some(start) = params.offset {
                p["start_line"] = serde_json::json!(start);
            }
            if let Some((s, l)) = params.offset.zip(params.limit) {
                p["end_line"] = serde_json::json!(s + l);
            }
            ipc.call(crux_protocol::method::PANE_GET_TEXT, p)
        })
        .await
        .map_err(|e| McpError::internal_error(format!("task join error: {e}"), None))?
        .map_err(|e| McpError::internal_error(format!("IPC error: {e}"), None))?;

        let output = extract_lines(&result);
        Ok(CallToolResult::success(vec![Content::text(output)]))
    }
}

fn find_pane(
    list_result: &serde_json::Value,
    pane_id: Option<u64>,
) -> Result<serde_json::Value, McpError> {
    let panes = list_result
        .get("panes")
        .and_then(|v| v.as_array())
        .ok_or_else(|| McpError::internal_error("unexpected pane list format", None))?;

    if let Some(id) = pane_id {
        panes
            .iter()
            .find(|p| {
                p.get("pane_id")
                    .and_then(|v| v.as_u64())
                    .is_some_and(|pid| pid == id)
            })
            .cloned()
            .ok_or_else(|| McpError::invalid_params(format!("pane {id} not found"), None))
    } else {
        panes
            .iter()
            .find(|p| {
                p.get("is_active")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false)
            })
            .or_else(|| panes.first())
            .cloned()
            .ok_or_else(|| McpError::internal_error("no panes available", None))
    }
}

fn extract_lines(result: &serde_json::Value) -> String {
    if let Some(lines) = result.get("lines").and_then(|v| v.as_array()) {
        lines
            .iter()
            .filter_map(|l| l.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    } else {
        serde_json::to_string_pretty(result).unwrap_or_else(|_| result.to_string())
    }
}
