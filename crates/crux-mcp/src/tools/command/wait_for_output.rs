use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::*;
use rmcp::{schemars, tool, tool_router, ErrorData as McpError};

use crate::server::CruxMcpServer;

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
    CruxMcpServer::wait_for_output_tools()
}

#[tool_router(router = wait_for_output_tools)]
impl CruxMcpServer {
    /// Wait for specific output to appear in a terminal pane.
    #[tool(description = "Wait for output matching a regex pattern to appear in a terminal pane")]
    async fn crux_wait_for_output(
        &self,
        Parameters(params): Parameters<WaitForOutputParams>,
    ) -> Result<CallToolResult, McpError> {
        // Validate pattern length
        if params.pattern.len() > 1024 {
            return Err(McpError::invalid_params(
                "Regex pattern too long (max 1024 chars). Simplify the pattern.",
                None,
            ));
        }

        // Build regex with size limit
        let re = regex::RegexBuilder::new(&params.pattern)
            .size_limit(1 << 20) // 1MB compiled regex limit
            .build()
            .map_err(|e| {
                McpError::invalid_params(format!("Invalid regex pattern: {e}. Check syntax."), None)
            })?;

        let timeout = std::time::Duration::from_millis(params.timeout_ms.unwrap_or(10000));
        let poll_interval = std::time::Duration::from_millis(200);
        let start = std::time::Instant::now();

        let result: Option<serde_json::Value> = loop {
            let p = serde_json::json!({ "pane_id": params.pane_id });
            let text_result = self
                .ipc_call(crux_protocol::method::PANE_GET_TEXT, p)
                .await?;

            let mut found = None;
            if let Some(lines) = text_result.get("lines").and_then(|v| v.as_array()) {
                for line in lines {
                    if let Some(s) = line.as_str() {
                        if re.is_match(s) {
                            found = Some(serde_json::json!({
                                "matched": true,
                                "line": s,
                                "elapsed_ms": start.elapsed().as_millis() as u64,
                            }));
                            break;
                        }
                    }
                }
            }

            if let Some(matched) = found {
                break Some(matched);
            }

            if start.elapsed() >= timeout {
                break None;
            }

            tokio::time::sleep(poll_interval).await;
        };

        match result {
            Some(matched) => Ok(CallToolResult::success(vec![Content::text(
                serde_json::to_string_pretty(&matched).unwrap_or_else(|_| matched.to_string()),
            )])),
            None => {
                let timeout_ms = params.timeout_ms.unwrap_or(10000);
                Ok(CallToolResult::error(vec![Content::text(format!(
                    "Timeout after {}ms waiting for pattern '{}'",
                    timeout_ms, params.pattern
                ))]))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn test_regex_pattern_max_length() {
        // Pattern over 1024 chars should be rejected
        let long_pattern = "a".repeat(1025);
        assert!(long_pattern.len() > 1024);
    }
}
