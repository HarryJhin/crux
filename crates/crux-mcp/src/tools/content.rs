use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::*;
use rmcp::{schemars, tool, tool_router, ErrorData as McpError};

use super::{extract_lines, extract_lines_raw, PaneIdParam, ScrollbackParams};
use crate::server::CruxMcpServer;

#[derive(Debug, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct SessionPathParam {
    /// File path for the session file (uses default if omitted)
    pub path: Option<String>,
}

pub(crate) fn router() -> rmcp::handler::server::router::tool::ToolRouter<CruxMcpServer> {
    CruxMcpServer::content_tools()
}

#[tool_router(router = content_tools)]
impl CruxMcpServer {
    /// Get text with ANSI escape sequences from a terminal pane.
    #[tool(
        description = "Get terminal text including ANSI escape sequences for color and formatting"
    )]
    async fn crux_get_formatted_output(
        &self,
        Parameters(params): Parameters<PaneIdParam>,
    ) -> Result<CallToolResult, McpError> {
        let p = serde_json::json!({
            "pane_id": params.pane_id,
            "include_escapes": true,
        });
        let result = self
            .ipc_call(crux_protocol::method::PANE_GET_TEXT, p)
            .await?;

        let output = extract_lines_raw(&result);
        Ok(CallToolResult::success(vec![Content::text(output)]))
    }

    /// Get scrollback text from a terminal pane with line range.
    #[tool(description = "Get scrollback text from a terminal pane with optional line range")]
    async fn crux_get_scrollback_text(
        &self,
        Parameters(params): Parameters<ScrollbackParams>,
    ) -> Result<CallToolResult, McpError> {
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
        let result = self
            .ipc_call(crux_protocol::method::PANE_GET_TEXT, p)
            .await?;

        let output = extract_lines(&result);
        Ok(CallToolResult::success(vec![Content::text(output)]))
    }

    /// Take a logical screenshot of a terminal pane (text + metadata).
    #[tool(
        description = "Take a snapshot of a terminal pane returning text content, cursor position, dimensions, and metadata as JSON"
    )]
    async fn crux_screenshot_pane(
        &self,
        Parameters(params): Parameters<PaneIdParam>,
    ) -> Result<CallToolResult, McpError> {
        let p = serde_json::json!({
            "pane_id": params.pane_id,
        });
        let result = self
            .ipc_call(crux_protocol::method::PANE_GET_SNAPSHOT, p)
            .await?;

        let output = serde_json::to_string_pretty(&result).unwrap_or_else(|_| result.to_string());
        Ok(CallToolResult::success(vec![Content::text(output)]))
    }

    /// Save the current terminal session layout to a file.
    #[tool(
        description = "Save the current terminal session layout to a JSON file. Uses ~/.config/crux/session.json by default."
    )]
    async fn crux_save_session(
        &self,
        Parameters(params): Parameters<SessionPathParam>,
    ) -> Result<CallToolResult, McpError> {
        let p = serde_json::json!({
            "path": params.path,
        });
        let result = self
            .ipc_call(crux_protocol::method::SESSION_SAVE, p)
            .await?;

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&result).unwrap_or_else(|_| "session saved".into()),
        )]))
    }

    /// Load a terminal session layout from a file, restoring panes and splits.
    #[tool(
        description = "Load a terminal session layout from a JSON file, restoring panes and splits. Uses ~/.config/crux/session.json by default."
    )]
    async fn crux_load_session(
        &self,
        Parameters(params): Parameters<SessionPathParam>,
    ) -> Result<CallToolResult, McpError> {
        let p = serde_json::json!({
            "path": params.path,
        });
        let result = self
            .ipc_call(crux_protocol::method::SESSION_LOAD, p)
            .await?;

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&result).unwrap_or_else(|_| "session loaded".into()),
        )]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
