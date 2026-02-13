use rmcp::handler::server::wrapper::Parameters;
use rmcp::model::*;
use rmcp::{schemars, tool, tool_router, ErrorData as McpError};

use crate::server::CruxMcpServer;

use super::common::translate_keys;

#[derive(Debug, serde::Serialize, serde::Deserialize, schemars::JsonSchema)]
pub struct SendKeysParams {
    /// Pane ID (uses active pane if omitted)
    pub pane_id: Option<u64>,
    /// Key name or sequence (e.g. 'enter', 'ctrl-c', 'up')
    pub keys: String,
}

pub(crate) fn router() -> rmcp::handler::server::router::tool::ToolRouter<CruxMcpServer> {
    CruxMcpServer::send_keys_tools()
}

#[tool_router(router = send_keys_tools)]
impl CruxMcpServer {
    /// Send key sequences to a terminal pane.
    #[tool(
        description = "Send special key sequences to a terminal pane. Supports: enter, tab, escape, ctrl-c, ctrl-d, ctrl-z, up, down, left, right, backspace."
    )]
    async fn crux_send_keys(
        &self,
        Parameters(params): Parameters<SendKeysParams>,
    ) -> Result<CallToolResult, McpError> {
        let text = translate_keys(&params.keys);
        let p = serde_json::json!({
            "pane_id": params.pane_id,
            "text": text,
            "bracketed_paste": false,
        });
        let result = self
            .ipc_call(crux_protocol::method::PANE_SEND_TEXT, p)
            .await?;

        Ok(CallToolResult::success(vec![Content::text(
            serde_json::to_string_pretty(&result).unwrap_or_else(|_| "keys sent".into()),
        )]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
