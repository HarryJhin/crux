use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::*;
use rmcp::{schemars, tool, tool_router, ErrorData as McpError};

use crate::server::CruxMcpServer;
use crate::tools::extract_lines;

use super::common::validate_command;

#[derive(Debug, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct ExecuteCommandParams {
    /// Pane ID (uses active pane if omitted)
    pub pane_id: Option<u64>,
    /// Shell command to execute
    pub command: String,
}

pub(crate) fn router() -> rmcp::handler::server::router::tool::ToolRouter<CruxMcpServer> {
    CruxMcpServer::execute_command_tools()
}

#[tool_router(router = execute_command_tools)]
impl CruxMcpServer {
    /// Execute a command in a terminal pane and capture output.
    #[tool(description = "Execute a shell command in a terminal pane and return the output")]
    async fn crux_execute_command(
        &self,
        Parameters(params): Parameters<ExecuteCommandParams>,
    ) -> Result<CallToolResult, McpError> {
        // Validate command before execution
        if let Err(reason) = validate_command(&params.command) {
            return Err(McpError::invalid_params(reason, None));
        }

        // Capture output before sending command to detect new output
        let before = self
            .ipc_call(
                crux_protocol::method::PANE_GET_TEXT,
                serde_json::json!({ "pane_id": params.pane_id }),
            )
            .await?;
        let before_len = before
            .get("lines")
            .and_then(|v| v.as_array())
            .map(|a| a.len())
            .unwrap_or(0);

        let send_params = serde_json::json!({
            "pane_id": params.pane_id,
            "text": format!("{}\n", params.command),
            "bracketed_paste": false,
        });
        self.ipc_call(crux_protocol::method::PANE_SEND_TEXT, send_params)
            .await?;

        // Poll for new output with timeout
        let timeout = std::time::Duration::from_millis(500);
        let poll_interval = std::time::Duration::from_millis(50);
        let start = std::time::Instant::now();

        let result = loop {
            tokio::time::sleep(poll_interval).await;

            let after = self
                .ipc_call(
                    crux_protocol::method::PANE_GET_TEXT,
                    serde_json::json!({ "pane_id": params.pane_id }),
                )
                .await?;
            let after_len = after
                .get("lines")
                .and_then(|v| v.as_array())
                .map(|a| a.len())
                .unwrap_or(0);

            if after_len > before_len || start.elapsed() >= timeout {
                break after;
            }
        };

        let output = extract_lines(&result);
        Ok(CallToolResult::success(vec![Content::text(output)]))
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
}
