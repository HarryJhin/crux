use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::*;
use rmcp::{schemars, tool, tool_router, ErrorData as McpError};

use crate::server::CruxMcpServer;

#[derive(Debug, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct SendTextParams {
    /// Pane ID (uses active pane if omitted)
    pub pane_id: Option<u64>,
    /// Text to send
    pub text: String,
    /// Wrap text in bracketed paste escape sequences
    pub bracketed_paste: Option<bool>,
}

pub(crate) fn router() -> rmcp::handler::server::router::tool::ToolRouter<CruxMcpServer> {
    CruxMcpServer::send_text_tools()
}

#[tool_router(router = send_text_tools)]
impl CruxMcpServer {
    /// Send raw text to a terminal pane.
    #[tool(description = "Send raw text directly to a terminal pane's PTY input")]
    async fn crux_send_text(
        &self,
        Parameters(params): Parameters<SendTextParams>,
    ) -> Result<CallToolResult, McpError> {
        let p = serde_json::json!({
            "pane_id": params.pane_id,
            "text": params.text,
            "bracketed_paste": params.bracketed_paste.unwrap_or(false),
        });
        let result = self
            .ipc_call(crux_protocol::method::PANE_SEND_TEXT, p)
            .await?;

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&result).unwrap_or_else(|_| "text sent".into()),
        )]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
