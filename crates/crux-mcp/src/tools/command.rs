use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::*;
use rmcp::{schemars, tool, tool_router, ErrorData as McpError};

use crate::server::CruxMcpServer;

#[derive(Debug, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct ExecuteCommandParams {
    /// Pane ID (uses active pane if omitted)
    pub pane_id: Option<u64>,
    /// Shell command to execute
    pub command: String,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct SendKeysParams {
    /// Pane ID (uses active pane if omitted)
    pub pane_id: Option<u64>,
    /// Key name or sequence (e.g. 'enter', 'ctrl-c', 'up')
    pub keys: String,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct SendTextParams {
    /// Pane ID (uses active pane if omitted)
    pub pane_id: Option<u64>,
    /// Text to send
    pub text: String,
    /// Wrap text in bracketed paste escape sequences
    pub bracketed_paste: Option<bool>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct GetOutputParams {
    /// Pane ID (uses active pane if omitted)
    pub pane_id: Option<u64>,
    /// Number of recent lines to retrieve
    pub lines: Option<u32>,
}

#[derive(Debug, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct WaitForOutputParams {
    /// Pane ID (uses active pane if omitted)
    pub pane_id: Option<u64>,
    /// Regex pattern to match against terminal output
    pub pattern: String,
    /// Timeout in milliseconds (default: 10000)
    pub timeout_ms: Option<u64>,
}

pub(crate) fn router() -> rmcp::handler::server::router::tool::ToolRouter<CruxMcpServer> {
    CruxMcpServer::command_tools()
}

#[tool_router(router = command_tools)]
impl CruxMcpServer {
    /// Execute a command in a terminal pane and capture output.
    #[tool(description = "Execute a shell command in a terminal pane and return the output")]
    async fn crux_execute_command(
        &self,
        Parameters(params): Parameters<ExecuteCommandParams>,
    ) -> Result<CallToolResult, McpError> {
        let ipc = self.ipc.clone();
        let result = tokio::task::spawn_blocking(move || {
            let send_params = serde_json::json!({
                "pane_id": params.pane_id,
                "text": format!("{}\n", params.command),
                "bracketed_paste": false,
            });
            ipc.call(crux_protocol::method::PANE_SEND_TEXT, send_params)?;

            std::thread::sleep(std::time::Duration::from_millis(100));

            let get_params = serde_json::json!({ "pane_id": params.pane_id });
            ipc.call(crux_protocol::method::PANE_GET_TEXT, get_params)
        })
        .await
        .map_err(|e| McpError::internal_error(format!("task join error: {e}"), None))?
        .map_err(|e| McpError::internal_error(format!("IPC error: {e}"), None))?;

        let output = extract_lines(&result);
        Ok(CallToolResult::success(vec![Content::text(output)]))
    }

    /// Send key sequences to a terminal pane.
    #[tool(
        description = "Send special key sequences to a terminal pane. Supports: enter, tab, escape, ctrl-c, ctrl-d, ctrl-z, up, down, left, right, backspace."
    )]
    async fn crux_send_keys(
        &self,
        Parameters(params): Parameters<SendKeysParams>,
    ) -> Result<CallToolResult, McpError> {
        let text = translate_keys(&params.keys);
        let ipc = self.ipc.clone();
        let result = tokio::task::spawn_blocking(move || {
            let p = serde_json::json!({
                "pane_id": params.pane_id,
                "text": text,
                "bracketed_paste": false,
            });
            ipc.call(crux_protocol::method::PANE_SEND_TEXT, p)
        })
        .await
        .map_err(|e| McpError::internal_error(format!("task join error: {e}"), None))?
        .map_err(|e| McpError::internal_error(format!("IPC error: {e}"), None))?;

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&result).unwrap_or_else(|_| "keys sent".into()),
        )]))
    }

    /// Send raw text to a terminal pane.
    #[tool(description = "Send raw text directly to a terminal pane's PTY input")]
    async fn crux_send_text(
        &self,
        Parameters(params): Parameters<SendTextParams>,
    ) -> Result<CallToolResult, McpError> {
        let ipc = self.ipc.clone();
        let result = tokio::task::spawn_blocking(move || {
            let p = serde_json::json!({
                "pane_id": params.pane_id,
                "text": params.text,
                "bracketed_paste": params.bracketed_paste.unwrap_or(false),
            });
            ipc.call(crux_protocol::method::PANE_SEND_TEXT, p)
        })
        .await
        .map_err(|e| McpError::internal_error(format!("task join error: {e}"), None))?
        .map_err(|e| McpError::internal_error(format!("IPC error: {e}"), None))?;

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&result).unwrap_or_else(|_| "text sent".into()),
        )]))
    }

    /// Get recent output from a terminal pane.
    #[tool(description = "Get recent output lines from a terminal pane")]
    async fn crux_get_output(
        &self,
        Parameters(params): Parameters<GetOutputParams>,
    ) -> Result<CallToolResult, McpError> {
        let ipc = self.ipc.clone();
        let result = tokio::task::spawn_blocking(move || {
            let mut p = serde_json::json!({ "pane_id": params.pane_id });
            if let Some(n) = params.lines {
                p["start_line"] = serde_json::json!(-(n as i32));
            }
            ipc.call(crux_protocol::method::PANE_GET_TEXT, p)
        })
        .await
        .map_err(|e| McpError::internal_error(format!("task join error: {e}"), None))?
        .map_err(|e| McpError::internal_error(format!("IPC error: {e}"), None))?;

        let output = extract_lines(&result);
        Ok(CallToolResult::success(vec![Content::text(output)]))
    }

    /// Wait for specific output to appear in a terminal pane.
    #[tool(description = "Wait for output matching a regex pattern to appear in a terminal pane")]
    async fn crux_wait_for_output(
        &self,
        Parameters(params): Parameters<WaitForOutputParams>,
    ) -> Result<CallToolResult, McpError> {
        let re = regex::Regex::new(&params.pattern)
            .map_err(|e| McpError::invalid_params(format!("invalid regex: {e}"), None))?;

        let timeout = std::time::Duration::from_millis(params.timeout_ms.unwrap_or(10000));
        let poll_interval = std::time::Duration::from_millis(200);
        let ipc = self.ipc.clone();
        let pane_id = params.pane_id;

        let result = tokio::task::spawn_blocking(move || {
            let start = std::time::Instant::now();
            loop {
                let p = serde_json::json!({ "pane_id": pane_id });
                let text_result = ipc.call(crux_protocol::method::PANE_GET_TEXT, p)?;

                if let Some(lines) = text_result.get("lines").and_then(|v| v.as_array()) {
                    for line in lines {
                        if let Some(s) = line.as_str() {
                            if re.is_match(s) {
                                return Ok(serde_json::json!({
                                    "matched": true,
                                    "line": s,
                                    "elapsed_ms": start.elapsed().as_millis() as u64,
                                }));
                            }
                        }
                    }
                }

                if start.elapsed() >= timeout {
                    return Ok(serde_json::json!({
                        "matched": false,
                        "elapsed_ms": start.elapsed().as_millis() as u64,
                        "error": "timeout waiting for pattern",
                    }));
                }

                std::thread::sleep(poll_interval);
            }
        })
        .await
        .map_err(|e| McpError::internal_error(format!("task join error: {e}"), None))?
        .map_err(|e: anyhow::Error| McpError::internal_error(format!("IPC error: {e}"), None))?;

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&result).unwrap_or_else(|_| result.to_string()),
        )]))
    }
}

fn translate_keys(keys: &str) -> String {
    match keys.to_lowercase().as_str() {
        "enter" | "return" => "\n".to_string(),
        "tab" => "\t".to_string(),
        "escape" | "esc" => "\x1b".to_string(),
        "ctrl-c" => "\x03".to_string(),
        "ctrl-d" => "\x04".to_string(),
        "ctrl-z" => "\x1a".to_string(),
        "ctrl-l" => "\x0c".to_string(),
        "ctrl-a" => "\x01".to_string(),
        "ctrl-e" => "\x05".to_string(),
        "ctrl-u" => "\x15".to_string(),
        "ctrl-k" => "\x0b".to_string(),
        "ctrl-w" => "\x17".to_string(),
        "up" => "\x1b[A".to_string(),
        "down" => "\x1b[B".to_string(),
        "right" => "\x1b[C".to_string(),
        "left" => "\x1b[D".to_string(),
        "home" => "\x1b[H".to_string(),
        "end" => "\x1b[F".to_string(),
        "backspace" => "\x7f".to_string(),
        "delete" => "\x1b[3~".to_string(),
        "page-up" => "\x1b[5~".to_string(),
        "page-down" => "\x1b[6~".to_string(),
        other => other.to_string(),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_execute_command_params_serde() {
        let params = ExecuteCommandParams {
            pane_id: Some(42),
            command: "echo hello".into(),
        };
        let json = serde_json::to_string(&params).unwrap();
        let parsed: ExecuteCommandParams = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.pane_id, Some(42));
        assert_eq!(parsed.command, "echo hello");
    }

    #[test]
    fn test_send_keys_params_serde() {
        let params = SendKeysParams {
            pane_id: None,
            keys: "enter".into(),
        };
        let json = serde_json::to_string(&params).unwrap();
        let parsed: SendKeysParams = serde_json::from_str(&json).unwrap();
        assert!(parsed.pane_id.is_none());
        assert_eq!(parsed.keys, "enter");
    }

    #[test]
    fn test_send_text_params_serde() {
        let params = SendTextParams {
            pane_id: Some(1),
            text: "hello world".into(),
            bracketed_paste: Some(true),
        };
        let json = serde_json::to_string(&params).unwrap();
        let parsed: SendTextParams = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.pane_id, Some(1));
        assert_eq!(parsed.text, "hello world");
        assert_eq!(parsed.bracketed_paste, Some(true));
    }

    #[test]
    fn test_get_output_params_serde() {
        let params = GetOutputParams {
            pane_id: Some(99),
            lines: Some(50),
        };
        let json = serde_json::to_string(&params).unwrap();
        let parsed: GetOutputParams = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.pane_id, Some(99));
        assert_eq!(parsed.lines, Some(50));
    }

    #[test]
    fn test_wait_for_output_params_serde() {
        let params = WaitForOutputParams {
            pane_id: Some(1),
            pattern: "^complete$".into(),
            timeout_ms: Some(5000),
        };
        let json = serde_json::to_string(&params).unwrap();
        let parsed: WaitForOutputParams = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.pane_id, Some(1));
        assert_eq!(parsed.pattern, "^complete$");
        assert_eq!(parsed.timeout_ms, Some(5000));
    }

    #[test]
    fn test_translate_keys_enter() {
        assert_eq!(translate_keys("enter"), "\n");
        assert_eq!(translate_keys("return"), "\n");
        assert_eq!(translate_keys("ENTER"), "\n");
    }

    #[test]
    fn test_translate_keys_tab() {
        assert_eq!(translate_keys("tab"), "\t");
    }

    #[test]
    fn test_translate_keys_escape() {
        assert_eq!(translate_keys("escape"), "\x1b");
        assert_eq!(translate_keys("esc"), "\x1b");
    }

    #[test]
    fn test_translate_keys_ctrl_sequences() {
        assert_eq!(translate_keys("ctrl-c"), "\x03");
        assert_eq!(translate_keys("ctrl-d"), "\x04");
        assert_eq!(translate_keys("ctrl-z"), "\x1a");
        assert_eq!(translate_keys("ctrl-a"), "\x01");
        assert_eq!(translate_keys("ctrl-e"), "\x05");
    }

    #[test]
    fn test_translate_keys_arrows() {
        assert_eq!(translate_keys("up"), "\x1b[A");
        assert_eq!(translate_keys("down"), "\x1b[B");
        assert_eq!(translate_keys("right"), "\x1b[C");
        assert_eq!(translate_keys("left"), "\x1b[D");
    }

    #[test]
    fn test_translate_keys_home_end() {
        assert_eq!(translate_keys("home"), "\x1b[H");
        assert_eq!(translate_keys("end"), "\x1b[F");
    }

    #[test]
    fn test_translate_keys_backspace_delete() {
        assert_eq!(translate_keys("backspace"), "\x7f");
        assert_eq!(translate_keys("delete"), "\x1b[3~");
    }

    #[test]
    fn test_translate_keys_page_up_down() {
        assert_eq!(translate_keys("page-up"), "\x1b[5~");
        assert_eq!(translate_keys("page-down"), "\x1b[6~");
    }

    #[test]
    fn test_translate_keys_unknown() {
        assert_eq!(translate_keys("x"), "x");
        assert_eq!(translate_keys("unknown-key"), "unknown-key");
    }

    #[test]
    fn test_extract_lines_with_lines_array() {
        let result = serde_json::json!({
            "lines": ["line 1", "line 2", "line 3"]
        });
        let output = extract_lines(&result);
        assert_eq!(output, "line 1\nline 2\nline 3");
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
    fn test_extract_lines_mixed_types() {
        let result = serde_json::json!({
            "lines": ["line 1", 123, "line 3", null]
        });
        let output = extract_lines(&result);
        // Only strings are extracted
        assert_eq!(output, "line 1\nline 3");
    }

    #[test]
    fn test_extract_lines_no_lines_field() {
        let result = serde_json::json!({
            "data": "some value"
        });
        let output = extract_lines(&result);
        // Should return pretty-printed JSON
        assert!(output.contains("\"data\""));
        assert!(output.contains("\"some value\""));
    }

    #[test]
    fn test_extract_lines_lines_not_array() {
        let result = serde_json::json!({
            "lines": "not an array"
        });
        let output = extract_lines(&result);
        // Should fall back to pretty-printed JSON
        assert!(output.contains("\"lines\""));
    }
}
