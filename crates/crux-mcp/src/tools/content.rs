use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::*;
use rmcp::{schemars, tool, tool_router, ErrorData as McpError};

use super::extract_lines;
use crate::server::CruxMcpServer;

#[derive(Debug, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct ContentPaneIdParam {
    /// Pane ID (uses active pane if omitted)
    pub pane_id: Option<u64>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_content_pane_id_param_serde() {
        let params = ContentPaneIdParam { pane_id: Some(42) };
        let json = serde_json::to_string(&params).unwrap();
        let parsed: ContentPaneIdParam = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.pane_id, Some(42));
    }

    #[test]
    fn test_content_pane_id_param_none() {
        let params = ContentPaneIdParam { pane_id: None };
        let json = serde_json::to_string(&params).unwrap();
        let parsed: ContentPaneIdParam = serde_json::from_str(&json).unwrap();
        assert!(parsed.pane_id.is_none());
    }

    #[test]
    fn test_scrollback_text_params_serde() {
        let params = ScrollbackTextParams {
            pane_id: Some(1),
            offset: Some(-50),
            limit: Some(25),
        };
        let json = serde_json::to_string(&params).unwrap();
        let parsed: ScrollbackTextParams = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.pane_id, Some(1));
        assert_eq!(parsed.offset, Some(-50));
        assert_eq!(parsed.limit, Some(25));
    }

    #[test]
    fn test_scrollback_text_params_all_none() {
        let params = ScrollbackTextParams {
            pane_id: None,
            offset: None,
            limit: None,
        };
        let json = serde_json::to_string(&params).unwrap();
        let parsed: ScrollbackTextParams = serde_json::from_str(&json).unwrap();
        assert!(parsed.pane_id.is_none());
        assert!(parsed.offset.is_none());
        assert!(parsed.limit.is_none());
    }

    #[test]
    fn test_scrollback_text_params_positive_offset() {
        let params = ScrollbackTextParams {
            pane_id: Some(99),
            offset: Some(100),
            limit: Some(10),
        };
        let json = serde_json::to_string(&params).unwrap();
        let parsed: ScrollbackTextParams = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.offset, Some(100));
    }

    #[test]
    fn test_extract_lines_with_lines_array() {
        let result = serde_json::json!({
            "lines": ["first line", "second line", "third line"]
        });
        let output = extract_lines(&result);
        assert_eq!(output, "first line\nsecond line\nthird line");
    }

    #[test]
    fn test_extract_lines_empty_array() {
        let result = serde_json::json!({
            "lines": []
        });
        let output = extract_lines(&result);
        assert_eq!(output, "");
    }

    #[test]
    fn test_extract_lines_single_line() {
        let result = serde_json::json!({
            "lines": ["only line"]
        });
        let output = extract_lines(&result);
        assert_eq!(output, "only line");
    }

    #[test]
    fn test_extract_lines_with_non_string_elements() {
        let result = serde_json::json!({
            "lines": ["line 1", 42, null, "line 2"]
        });
        let output = extract_lines(&result);
        // Only string elements should be included
        assert_eq!(output, "line 1\nline 2");
    }

    #[test]
    fn test_extract_lines_no_lines_field() {
        let result = serde_json::json!({
            "other_field": "value"
        });
        let output = extract_lines(&result);
        // Should return pretty-printed JSON
        assert!(output.contains("\"other_field\""));
        assert!(output.contains("\"value\""));
    }

    #[test]
    fn test_extract_lines_lines_not_array() {
        let result = serde_json::json!({
            "lines": "string instead of array"
        });
        let output = extract_lines(&result);
        // Should fall back to pretty-printed JSON
        assert!(output.contains("\"lines\""));
        assert!(output.contains("\"string instead of array\""));
    }

}
