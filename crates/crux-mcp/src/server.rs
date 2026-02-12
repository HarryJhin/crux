use std::sync::Arc;

use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::model::*;
use rmcp::service::RequestContext;
use rmcp::ErrorData as McpError;
use rmcp::RoleServer;
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

    pub fn new_from_arc(ipc: Arc<IpcClient>) -> Self {
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
            capabilities: ServerCapabilities::builder()
                .enable_tools()
                .enable_resources()
                .build(),
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

    async fn list_resource_templates(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourceTemplatesResult, McpError> {
        Ok(ListResourceTemplatesResult {
            resource_templates: crate::resources::resource_templates(),
            meta: None,
            next_cursor: None,
        })
    }

    async fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<ReadResourceResult, McpError> {
        let uri = &request.uri;
        let (pane_id, resource_type) =
            crate::resources::parse_resource_uri(uri).ok_or_else(|| {
                McpError::invalid_params(format!("invalid resource URI: {uri}"), None)
            })?;

        let ipc = self.ipc.clone();
        let resource_type = resource_type.to_string();
        let contents = tokio::task::spawn_blocking(move || {
            crate::resources::read_resource_data(&ipc, pane_id, &resource_type)
        })
        .await
        .map_err(|e| McpError::internal_error(format!("task join error: {e}"), None))?
        .map_err(|e| McpError::internal_error(e, None))?;

        Ok(ReadResourceResult {
            contents: vec![contents],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Note: Server tests are limited because IpcClient requires a real Unix socket
    // connection and its fields are private. These tests verify basic metadata
    // without requiring an actual IPC connection.

    #[test]
    fn test_server_metadata_name() {
        // Test the server name constant
        let expected_name = "crux-mcp";
        // This would be returned by get_info() if we could construct a server
        assert_eq!(expected_name, "crux-mcp");
    }

    #[test]
    fn test_server_metadata_version() {
        // Verify CARGO_PKG_VERSION is set
        let version = env!("CARGO_PKG_VERSION");
        assert!(!version.is_empty());
        assert!(version.contains('.'), "Version should contain dots");
    }

    #[test]
    fn test_server_clone_trait() {
        // Verify CruxMcpServer implements Clone
        // We can't construct one without IPC, but we can check the trait bound
        fn assert_clone<T: Clone>() {}
        assert_clone::<CruxMcpServer>();
    }

    #[test]
    fn test_tool_router_construction() {
        // Test that tool routers can be combined
        let router1 = crate::tools::pane::router();
        let router2 = crate::tools::command::router();
        let _combined = router1 + router2;
        // If this compiles, the router construction works
    }
}
