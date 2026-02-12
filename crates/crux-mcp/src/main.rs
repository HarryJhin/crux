mod ipc_client;
mod resources;
mod server;
mod tools;

use clap::Parser;
use rmcp::ServiceExt;

#[derive(Parser)]
#[command(name = "crux-mcp", about = "MCP server for Crux terminal")]
struct Args {
    /// Override socket path
    #[arg(long)]
    socket: Option<String>,

    /// Also listen on HTTP (localhost only)
    #[arg(long)]
    http: bool,

    /// HTTP port (used with --http)
    #[arg(long, default_value = "3100")]
    port: u16,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info"))
        .target(env_logger::Target::Stderr)
        .init();

    let args = Args::parse();
    if let Some(ref socket) = args.socket {
        // SAFETY: single-threaded at this point, before any IPC connection.
        unsafe { std::env::set_var("CRUX_SOCKET", socket) };
    }

    let ipc = ipc_client::IpcClient::connect_with_retry(10)?;

    if args.http {
        run_http(ipc, args.port).await
    } else {
        run_stdio(ipc).await
    }
}

async fn run_stdio(ipc: ipc_client::IpcClient) -> anyhow::Result<()> {
    let server = server::CruxMcpServer::new(ipc);
    log::info!("crux-mcp server starting via stdio");
    let service = server.serve(rmcp::transport::stdio()).await?;
    service.waiting().await?;
    Ok(())
}

async fn run_http(ipc: ipc_client::IpcClient, port: u16) -> anyhow::Result<()> {
    use rmcp::transport::streamable_http_server::{
        session::local::LocalSessionManager, StreamableHttpServerConfig, StreamableHttpService,
    };
    use std::sync::Arc;

    let ipc = Arc::new(ipc);
    let session_manager = Arc::new(LocalSessionManager::default());
    let config = StreamableHttpServerConfig::default();

    let http_service = StreamableHttpService::new(
        {
            let ipc = ipc.clone();
            move || Ok(server::CruxMcpServer::new_from_arc(ipc.clone()))
        },
        session_manager,
        config,
    );

    let app = axum::Router::new().fallback_service(http_service);
    let addr = std::net::SocketAddr::from(([127, 0, 0, 1], port));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    log::info!("crux-mcp HTTP server listening on http://{addr}/mcp");
    axum::serve(listener, app).await?;
    Ok(())
}
