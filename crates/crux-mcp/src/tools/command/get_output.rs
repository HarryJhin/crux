use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::*;
use rmcp::{schemars, tool, tool_router, ErrorData as McpError};

use crate::server::CruxMcpServer;
use crate::tools::extract_lines;

#[derive(Debug, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct GetOutputParams {
    /// Pane ID (uses active pane if omitted)
    pub pane_id: Option<u64>,
    /// Number of recent lines to retrieve
    pub lines: Option<u32>,
}

pub(crate) fn router() -> rmcp::handler::server::router::tool::ToolRouter<CruxMcpServer> {
    CruxMcpServer::get_output_tools()
}

#[tool_router(router = get_output_tools)]
impl CruxMcpServer {
    /// Get recent output from a terminal pane.
    #[tool(description = "Get recent output lines from a terminal pane")]
    async fn crux_get_output(
        &self,
        Parameters(params): Parameters<GetOutputParams>,
    ) -> Result<CallToolResult, McpError> {
        let mut p = serde_json::json!({ "pane_id": params.pane_id });
        if let Some(n) = params.lines {
            p["start_line"] = serde_json::json!(-(n as i32));
        }
        let result = self
            .ipc_call(crux_protocol::method::PANE_GET_TEXT, p)
            .await?;

        let output = extract_lines(&result);
        Ok(CallToolResult::success(vec![Content::text(output)]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
