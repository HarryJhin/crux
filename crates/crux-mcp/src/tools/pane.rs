use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::*;
use rmcp::{schemars, tool, tool_router, ErrorData as McpError};

use crate::server::CruxMcpServer;

#[derive(Debug, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct CreatePaneParams {
    /// Split direction: right, left, top, bottom
    #[schemars(description = "Split direction: right, left, top, bottom")]
    pub direction: Option<String>,
    /// Working directory for the new pane
    pub cwd: Option<String>,
    /// Command to run in the new pane
    pub command: Option<String>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct ClosePaneParams {
    /// Pane ID to close
    pub pane_id: u64,
    /// Force close without confirmation
    pub force: Option<bool>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct FocusPaneParams {
    /// Pane ID to focus
    pub pane_id: u64,
}

#[allow(dead_code)]
#[derive(Debug, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct ResizePaneParams {
    /// Pane ID to resize
    pub pane_id: u64,
    /// Number of rows
    pub rows: u32,
    /// Number of columns
    pub cols: u32,
}

pub(crate) fn router() -> rmcp::handler::server::router::tool::ToolRouter<CruxMcpServer> {
    CruxMcpServer::pane_tools()
}

#[tool_router(router = pane_tools)]
impl CruxMcpServer {
    /// Create a new terminal pane by splitting an existing one.
    #[tool(description = "Create a new terminal pane by splitting the current pane")]
    async fn crux_create_pane(
        &self,
        Parameters(params): Parameters<CreatePaneParams>,
    ) -> Result<CallToolResult, McpError> {
        let ipc = self.ipc.clone();
        let result = tokio::task::spawn_blocking(move || {
            let mut p = serde_json::json!({
                "direction": params.direction.unwrap_or_else(|| "right".into()),
            });
            if let Some(cwd) = params.cwd {
                p["cwd"] = serde_json::Value::String(cwd);
            }
            if let Some(command) = params.command {
                p["command"] = serde_json::json!([command]);
            }
            ipc.call(crux_protocol::method::PANE_SPLIT, p)
        })
        .await
        .map_err(|e| McpError::internal_error(format!("task join error: {e}"), None))?
        .map_err(|e| McpError::internal_error(format!("IPC error: {e}"), None))?;

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&result).unwrap_or_else(|_| result.to_string()),
        )]))
    }

    /// Close a terminal pane.
    #[tool(description = "Close a terminal pane by its ID")]
    async fn crux_close_pane(
        &self,
        Parameters(params): Parameters<ClosePaneParams>,
    ) -> Result<CallToolResult, McpError> {
        let ipc = self.ipc.clone();
        let result = tokio::task::spawn_blocking(move || {
            let p = serde_json::json!({
                "pane_id": params.pane_id,
                "force": params.force.unwrap_or(false),
            });
            ipc.call(crux_protocol::method::PANE_CLOSE, p)
        })
        .await
        .map_err(|e| McpError::internal_error(format!("task join error: {e}"), None))?
        .map_err(|e| McpError::internal_error(format!("IPC error: {e}"), None))?;

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&result).unwrap_or_else(|_| "pane closed".into()),
        )]))
    }

    /// Focus (activate) a terminal pane.
    #[tool(description = "Focus a terminal pane by its ID")]
    async fn crux_focus_pane(
        &self,
        Parameters(params): Parameters<FocusPaneParams>,
    ) -> Result<CallToolResult, McpError> {
        let ipc = self.ipc.clone();
        let result = tokio::task::spawn_blocking(move || {
            let p = serde_json::json!({ "pane_id": params.pane_id });
            ipc.call(crux_protocol::method::PANE_ACTIVATE, p)
        })
        .await
        .map_err(|e| McpError::internal_error(format!("task join error: {e}"), None))?
        .map_err(|e| McpError::internal_error(format!("IPC error: {e}"), None))?;

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&result).unwrap_or_else(|_| "pane focused".into()),
        )]))
    }

    /// List all terminal panes.
    #[tool(description = "List all terminal panes with their IDs, sizes, and status")]
    async fn crux_list_panes(&self) -> Result<CallToolResult, McpError> {
        let ipc = self.ipc.clone();
        let result = tokio::task::spawn_blocking(move || {
            ipc.call(crux_protocol::method::PANE_LIST, serde_json::json!({}))
        })
        .await
        .map_err(|e| McpError::internal_error(format!("task join error: {e}"), None))?
        .map_err(|e| McpError::internal_error(format!("IPC error: {e}"), None))?;

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&result).unwrap_or_else(|_| result.to_string()),
        )]))
    }

    /// Resize a terminal pane.
    #[tool(description = "Resize a terminal pane (not yet supported)")]
    async fn crux_resize_pane(
        &self,
        Parameters(_params): Parameters<ResizePaneParams>,
    ) -> Result<CallToolResult, McpError> {
        Ok(CallToolResult::success(vec![Content::text(
            "pane resize is not yet supported",
        )]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_pane_params_serde() {
        let params = CreatePaneParams {
            direction: Some("right".into()),
            cwd: Some("/tmp".into()),
            command: Some("ls".into()),
        };
        let json = serde_json::to_string(&params).unwrap();
        let parsed: CreatePaneParams = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.direction, Some("right".into()));
        assert_eq!(parsed.cwd, Some("/tmp".into()));
        assert_eq!(parsed.command, Some("ls".into()));
    }

    #[test]
    fn test_create_pane_params_optional_fields() {
        let params = CreatePaneParams {
            direction: None,
            cwd: None,
            command: None,
        };
        let json = serde_json::to_string(&params).unwrap();
        let parsed: CreatePaneParams = serde_json::from_str(&json).unwrap();
        assert!(parsed.direction.is_none());
        assert!(parsed.cwd.is_none());
        assert!(parsed.command.is_none());
    }

    #[test]
    fn test_close_pane_params_serde() {
        let params = ClosePaneParams {
            pane_id: 42,
            force: Some(true),
        };
        let json = serde_json::to_string(&params).unwrap();
        let parsed: ClosePaneParams = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.pane_id, 42);
        assert_eq!(parsed.force, Some(true));
    }

    #[test]
    fn test_close_pane_params_force_default() {
        let json = r#"{"pane_id": 99}"#;
        let parsed: ClosePaneParams = serde_json::from_str(json).unwrap();
        assert_eq!(parsed.pane_id, 99);
        assert_eq!(parsed.force, None);
    }

    #[test]
    fn test_focus_pane_params_serde() {
        let params = FocusPaneParams { pane_id: 123 };
        let json = serde_json::to_string(&params).unwrap();
        let parsed: FocusPaneParams = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.pane_id, 123);
    }

    #[test]
    fn test_resize_pane_params_serde() {
        let params = ResizePaneParams {
            pane_id: 1,
            rows: 24,
            cols: 80,
        };
        let json = serde_json::to_string(&params).unwrap();
        let parsed: ResizePaneParams = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.pane_id, 1);
        assert_eq!(parsed.rows, 24);
        assert_eq!(parsed.cols, 80);
    }

}
