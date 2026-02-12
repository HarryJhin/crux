use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::*;
use rmcp::{schemars, tool, tool_router, ErrorData as McpError};

use crate::server::CruxMcpServer;

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct ContentPaneIdParam {
    /// Pane ID (uses active pane if omitted)
    pub pane_id: Option<u64>,
}

#[derive(Debug, serde::Deserialize, schemars::JsonSchema)]
pub struct ScrollbackTextParams {
    /// Pane ID (uses active pane if omitted)
    pub pane_id: Option<u64>,
    /// Starting line offset (negative for scrollback)
    pub offset: Option<i32>,
    /// Number of lines to retrieve
    pub limit: Option<i32>,
}

pub(crate) fn router() -> rmcp::handler::server::router::tool::ToolRouter<CruxMcpServer> {
    CruxMcpServer::content_tools()
}

#[tool_router(router = content_tools)]
impl CruxMcpServer {
    /// Get raw text content from a terminal pane.
    #[tool(description = "Get the raw text content from a terminal pane's visible area")]
    async fn crux_get_raw_text(
        &self,
        Parameters(params): Parameters<ContentPaneIdParam>,
    ) -> Result<CallToolResult, McpError> {
        let ipc = self.ipc.clone();
        let result = tokio::task::spawn_blocking(move || {
            let p = serde_json::json!({
                "pane_id": params.pane_id,
                "include_escapes": false,
            });
            ipc.call(crux_protocol::method::PANE_GET_TEXT, p)
        })
        .await
        .map_err(|e| McpError::internal_error(format!("task join error: {e}"), None))?
        .map_err(|e| McpError::internal_error(format!("IPC error: {e}"), None))?;

        let output = extract_lines(&result);
        Ok(CallToolResult::success(vec![Content::text(output)]))
    }

    /// Get text with ANSI escape sequences from a terminal pane.
    #[tool(
        description = "Get terminal text including ANSI escape sequences for color and formatting"
    )]
    async fn crux_get_formatted_output(
        &self,
        Parameters(params): Parameters<ContentPaneIdParam>,
    ) -> Result<CallToolResult, McpError> {
        let ipc = self.ipc.clone();
        let result = tokio::task::spawn_blocking(move || {
            let p = serde_json::json!({
                "pane_id": params.pane_id,
                "include_escapes": true,
            });
            ipc.call(crux_protocol::method::PANE_GET_TEXT, p)
        })
        .await
        .map_err(|e| McpError::internal_error(format!("task join error: {e}"), None))?
        .map_err(|e| McpError::internal_error(format!("IPC error: {e}"), None))?;

        let output = extract_lines(&result);
        Ok(CallToolResult::success(vec![Content::text(output)]))
    }

    /// Get scrollback text from a terminal pane with line range.
    #[tool(description = "Get scrollback text from a terminal pane with optional line range")]
    async fn crux_get_scrollback_text(
        &self,
        Parameters(params): Parameters<ScrollbackTextParams>,
    ) -> Result<CallToolResult, McpError> {
        let ipc = self.ipc.clone();
        let result = tokio::task::spawn_blocking(move || {
            let mut p = serde_json::json!({
                "pane_id": params.pane_id,
                "include_escapes": false,
            });
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

    /// Take a screenshot of a terminal pane.
    #[tool(description = "Take a screenshot of a terminal pane (not yet supported)")]
    async fn crux_screenshot_pane(
        &self,
        Parameters(_params): Parameters<ContentPaneIdParam>,
    ) -> Result<CallToolResult, McpError> {
        Ok(CallToolResult::success(vec![Content::text(
            "screenshot capture is not yet supported, requires GPUI rendering pipeline",
        )]))
    }

    /// Save or restore a terminal session.
    #[tool(description = "Save or restore a terminal session (not yet supported)")]
    async fn crux_save_restore_session(&self) -> Result<CallToolResult, McpError> {
        Ok(CallToolResult::success(vec![Content::text(
            "session save/restore is not yet supported",
        )]))
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
