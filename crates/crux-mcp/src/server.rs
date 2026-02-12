use std::sync::Arc;

use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::model::*;
use rmcp::{tool_handler, ServerHandler};

use crate::ipc_client::IpcClient;

#[derive(Clone)]
pub struct CruxMcpServer {
    pub ipc: Arc<IpcClient>,
    pub tool_router: ToolRouter<Self>,
}

impl CruxMcpServer {
    pub fn new(ipc: IpcClient) -> Self {
        let ipc = Arc::new(ipc);
        let tool_router = crate::tools::pane::router()
            + crate::tools::command::router()
            + crate::tools::state::router()
            + crate::tools::content::router();
        Self { ipc, tool_router }
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for CruxMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            instructions: Some(
                "Crux terminal emulator MCP server. \
                 Control terminal panes, execute commands, and inspect state."
                    .into(),
            ),
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            server_info: Implementation {
                name: "crux-mcp".into(),
                title: None,
                version: env!("CARGO_PKG_VERSION").into(),
                description: Some("MCP server for Crux terminal emulator".into()),
                icons: None,
                website_url: None,
            },
            ..Default::default()
        }
    }
}
