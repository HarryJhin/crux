mod ipc_client;
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

    let ipc = ipc_client::IpcClient::connect()?;
    let server = server::CruxMcpServer::new(ipc);

    log::info!("crux-mcp server starting via stdio");
    let service = server.serve(rmcp::transport::stdio()).await?;
    service.waiting().await?;

    Ok(())
}
