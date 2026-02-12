use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::*;
use rmcp::{schemars, tool, tool_router, ErrorData as McpError};

use super::extract_lines;
use crate::server::CruxMcpServer;

#[derive(Debug, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct PaneIdParam {
    /// Pane ID (uses active pane if omitted)
    pub pane_id: Option<u64>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
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
    #[tool(description = "Get the currently selected text in a terminal pane")]
    async fn crux_get_selection(
        &self,
        Parameters(params): Parameters<PaneIdParam>,
    ) -> Result<CallToolResult, McpError> {
        let ipc = self.ipc.clone();
        let result = tokio::task::spawn_blocking(move || {
            ipc.call(
                crux_protocol::method::PANE_GET_SELECTION,
                serde_json::json!({ "pane_id": params.pane_id }),
            )
        })
        .await
        .map_err(|e| McpError::internal_error(format!("task join error: {e}"), None))?
        .map_err(|e| McpError::internal_error(format!("IPC error: {e}"), None))?;

        let text = result
            .get("text")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let has_selection = result
            .get("has_selection")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        if has_selection {
            Ok(CallToolResult::success(vec![Content::text(text)]))
        } else {
            Ok(CallToolResult::success(vec![Content::text(
                "no text is currently selected",
            )]))
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pane_id_param_serde() {
        let params = PaneIdParam { pane_id: Some(42) };
        let json = serde_json::to_string(&params).unwrap();
        let parsed: PaneIdParam = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.pane_id, Some(42));
    }

    #[test]
    fn test_pane_id_param_none() {
        let params = PaneIdParam { pane_id: None };
        let json = serde_json::to_string(&params).unwrap();
        let parsed: PaneIdParam = serde_json::from_str(&json).unwrap();
        assert!(parsed.pane_id.is_none());
    }

    #[test]
    fn test_scrollback_params_serde() {
        let params = ScrollbackParams {
            pane_id: Some(1),
            offset: Some(-100),
            limit: Some(50),
        };
        let json = serde_json::to_string(&params).unwrap();
        let parsed: ScrollbackParams = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.pane_id, Some(1));
        assert_eq!(parsed.offset, Some(-100));
        assert_eq!(parsed.limit, Some(50));
    }

    #[test]
    fn test_scrollback_params_all_none() {
        let params = ScrollbackParams {
            pane_id: None,
            offset: None,
            limit: None,
        };
        let json = serde_json::to_string(&params).unwrap();
        let parsed: ScrollbackParams = serde_json::from_str(&json).unwrap();
        assert!(parsed.pane_id.is_none());
        assert!(params.offset.is_none());
        assert!(params.limit.is_none());
    }

    #[test]
    fn test_find_pane_by_id() {
        let list_result = serde_json::json!({
            "panes": [
                {"pane_id": 1, "is_active": false},
                {"pane_id": 42, "is_active": true},
                {"pane_id": 99, "is_active": false}
            ]
        });
        let pane = find_pane(&list_result, Some(42)).unwrap();
        assert_eq!(pane.get("pane_id").unwrap().as_u64(), Some(42));
    }

    #[test]
    fn test_find_pane_active_when_no_id() {
        let list_result = serde_json::json!({
            "panes": [
                {"pane_id": 1, "is_active": false},
                {"pane_id": 42, "is_active": true},
                {"pane_id": 99, "is_active": false}
            ]
        });
        let pane = find_pane(&list_result, None).unwrap();
        assert_eq!(pane.get("pane_id").unwrap().as_u64(), Some(42));
        assert_eq!(pane.get("is_active").unwrap().as_bool(), Some(true));
    }

    #[test]
    fn test_find_pane_first_when_no_active() {
        let list_result = serde_json::json!({
            "panes": [
                {"pane_id": 1, "is_active": false},
                {"pane_id": 2, "is_active": false}
            ]
        });
        let pane = find_pane(&list_result, None).unwrap();
        assert_eq!(pane.get("pane_id").unwrap().as_u64(), Some(1));
    }

    #[test]
    fn test_find_pane_not_found() {
        let list_result = serde_json::json!({
            "panes": [
                {"pane_id": 1, "is_active": false},
                {"pane_id": 2, "is_active": false}
            ]
        });
        let result = find_pane(&list_result, Some(999));
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("pane 999 not found"));
    }

    #[test]
    fn test_find_pane_empty_list() {
        let list_result = serde_json::json!({
            "panes": []
        });
        let result = find_pane(&list_result, None);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("no panes available"));
    }

    #[test]
    fn test_find_pane_invalid_format() {
        let list_result = serde_json::json!({
            "panes": "not an array"
        });
        let result = find_pane(&list_result, None);
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(err_msg.contains("unexpected pane list format"));
    }

    #[test]
    fn test_find_pane_missing_panes_field() {
        let list_result = serde_json::json!({
            "data": []
        });
        let result = find_pane(&list_result, None);
        assert!(result.is_err());
    }

    #[test]
    fn test_extract_lines_with_lines() {
        let result = serde_json::json!({
            "lines": ["line 1", "line 2", "line 3"]
        });
        let output = extract_lines(&result);
        assert_eq!(output, "line 1\nline 2\nline 3");
    }

    #[test]
    fn test_extract_lines_without_lines() {
        let result = serde_json::json!({
            "data": "value"
        });
        let output = extract_lines(&result);
        assert!(output.contains("\"data\""));
    }
}
