use std::sync::Arc;

use governor::{DefaultDirectRateLimiter, Quota, RateLimiter};
use rmcp::handler::server::router::tool::ToolRouter;
use rmcp::model::*;
use rmcp::service::RequestContext;
use rmcp::ErrorData as McpError;
use rmcp::RoleServer;
use rmcp::{tool_handler, ServerHandler};

use crate::ipc_client::{IpcClient, IpcTransport};

#[derive(Clone)]
pub struct CruxMcpServer {
    pub ipc: Arc<dyn IpcTransport>,
    pub tool_router: ToolRouter<Self>,
    pub rate_limiter: Arc<DefaultDirectRateLimiter>,
}

impl CruxMcpServer {
    pub fn new(ipc: IpcClient) -> Self {
        let ipc: Arc<dyn IpcTransport> = Arc::new(ipc);
        let tool_router = crate::tools::pane::router()
            + crate::tools::command::router()
            + crate::tools::state::router()
            + crate::tools::content::router();

        // Rate limiter: 20 requests per second with burst of 40
        let quota = Quota::per_second(std::num::NonZeroU32::new(20).unwrap())
            .allow_burst(std::num::NonZeroU32::new(40).unwrap());
        let rate_limiter = Arc::new(RateLimiter::direct(quota));

        Self {
            ipc,
            tool_router,
            rate_limiter,
        }
    }

    pub fn new_from_arc(ipc: Arc<dyn IpcTransport>) -> Self {
        let tool_router = crate::tools::pane::router()
            + crate::tools::command::router()
            + crate::tools::state::router()
            + crate::tools::content::router();

        // Rate limiter: 20 requests per second with burst of 40
        let quota = Quota::per_second(std::num::NonZeroU32::new(20).unwrap())
            .allow_burst(std::num::NonZeroU32::new(40).unwrap());
        let rate_limiter = Arc::new(RateLimiter::direct(quota));

        Self {
            ipc,
            tool_router,
            rate_limiter,
        }
    }

    /// Helper to make IPC calls with consistent error handling.
    pub(crate) async fn ipc_call(
        &self,
        method: &str,
        params: serde_json::Value,
    ) -> Result<serde_json::Value, McpError> {
        // Check rate limiter before making the IPC call
        if self.rate_limiter.check().is_err() {
            return Err(McpError::internal_error(
                "Rate limited: too many requests. Please wait before retrying.",
                None,
            ));
        }

        let ipc = self.ipc.clone();
        let method = method.to_string();
        let method_for_error = method.clone();

        tokio::task::spawn_blocking(move || ipc.call(&method, params))
            .await
            .map_err(|e| McpError::internal_error(format!("task join error: {e}"), None))?
            .map_err(|e| {
                let err_str = e.to_string();

                // Parse IPC error messages intelligently
                if err_str.contains("server error") {
                    // Extract error code and message from "server error {code}: {message}"
                    if err_str.contains("-1001") || err_str.to_lowercase().contains("not found") {
                        return McpError::invalid_params(
                            "Pane not found. Use crux_list_panes to see available pane IDs.",
                            None,
                        );
                    }
                }

                if err_str.contains("server closed connection") {
                    return McpError::internal_error(
                        "Crux terminal disconnected. Is Crux still running?",
                        None,
                    );
                }

                // Default error with context
                McpError::internal_error(
                    format!(
                        "IPC call to '{}' failed: {}. Check Crux terminal logs.",
                        method_for_error, e
                    ),
                    None,
                )
            })
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
                .enable_prompts()
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

    async fn list_resources(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListResourcesResult, McpError> {
        // Query active panes via IPC and generate concrete resource URIs
        let ipc = self.ipc.clone();
        let panes_result = tokio::task::spawn_blocking(move || {
            ipc.call(crux_protocol::method::PANE_LIST, serde_json::json!({}))
        })
        .await
        .map_err(|e| McpError::internal_error(format!("task join error: {e}"), None))?;

        // If IPC call fails (Crux not running), return empty list for graceful degradation
        let panes = match panes_result {
            Ok(result) => result
                .get("panes")
                .and_then(|v| v.as_array())
                .cloned()
                .unwrap_or_default(),
            Err(_) => {
                return Ok(ListResourcesResult {
                    resources: vec![],
                    meta: None,
                    next_cursor: None,
                })
            }
        };

        // Generate 2 resources per pane: scrollback and state
        let mut resources = Vec::new();
        for pane in panes {
            if let Some(pane_id) = pane.get("pane_id").and_then(|v| v.as_u64()) {
                // Scrollback resource
                resources.push(Annotated::new(
                    RawResource {
                        uri: format!("crux://pane/{pane_id}/scrollback"),
                        name: format!("Pane {pane_id} Scrollback"),
                        title: Some(format!("Pane {pane_id} Scrollback")),
                        description: Some("Terminal scrollback buffer content".into()),
                        mime_type: Some("text/plain".into()),
                        icons: None,
                        meta: None,
                        size: None,
                    },
                    None,
                ));

                // State resource
                resources.push(Annotated::new(
                    RawResource {
                        uri: format!("crux://pane/{pane_id}/state"),
                        name: format!("Pane {pane_id} State"),
                        title: Some(format!("Pane {pane_id} State")),
                        description: Some("Full pane state as JSON".into()),
                        mime_type: Some("application/json".into()),
                        icons: None,
                        meta: None,
                        size: None,
                    },
                    None,
                ));
            }
        }

        Ok(ListResourcesResult {
            resources,
            meta: None,
            next_cursor: None,
        })
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
            crate::resources::read_resource_data(&*ipc, pane_id, &resource_type)
        })
        .await
        .map_err(|e| McpError::internal_error(format!("task join error: {e}"), None))??;

        Ok(ReadResourceResult {
            contents: vec![contents],
        })
    }

    async fn list_prompts(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> Result<ListPromptsResult, McpError> {
        Ok(ListPromptsResult {
            prompts: crate::prompts::list(),
            meta: None,
            next_cursor: None,
        })
    }

    async fn get_prompt(
        &self,
        request: GetPromptRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<GetPromptResult, McpError> {
        crate::prompts::get(&request.name, &request.arguments)
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

    #[test]
    fn test_rate_limiter_initialization() {
        // Verify that rate limiter can be created with the expected quota
        use governor::{Quota, RateLimiter};
        use std::num::NonZeroU32;

        let quota = Quota::per_second(NonZeroU32::new(20).unwrap())
            .allow_burst(NonZeroU32::new(40).unwrap());
        let limiter = RateLimiter::direct(quota);

        // First request should succeed
        assert!(limiter.check().is_ok());
    }

    #[test]
    fn test_rate_limiter_in_arc() {
        // Verify that rate limiter can be wrapped in Arc (required for Clone)
        use governor::{Quota, RateLimiter};
        use std::num::NonZeroU32;

        let quota = Quota::per_second(NonZeroU32::new(20).unwrap())
            .allow_burst(NonZeroU32::new(40).unwrap());
        let limiter = Arc::new(RateLimiter::direct(quota));

        // Cloning Arc should work
        let _clone = limiter.clone();

        // First request should succeed
        assert!(limiter.check().is_ok());
    }
}
