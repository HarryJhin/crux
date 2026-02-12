//! Main IPC server — accepts connections on a Unix domain socket.

use std::path::PathBuf;

use tokio::net::UnixListener;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use crate::command::IpcCommand;
use crate::handler::handle_client;

/// Start the IPC server on a background tokio task.
///
/// Binds to `socket_path`, verifies peer credentials on each connection, and
/// spawns a per-client handler that communicates with the GPUI thread through
/// `cmd_tx`.
///
/// Returns the server task handle.
pub async fn start_server(
    socket_path: PathBuf,
    cmd_tx: mpsc::Sender<IpcCommand>,
) -> anyhow::Result<JoinHandle<()>> {
    // Clean up stale socket from a previous run.
    let _ = std::fs::remove_file(&socket_path);

    let listener = UnixListener::bind(&socket_path)?;

    // Restrict socket permissions to owner-only.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&socket_path, std::fs::Permissions::from_mode(0o600))?;
    }

    log::info!("IPC server listening on {}", socket_path.display());

    let handle = tokio::spawn(async move {
        loop {
            match listener.accept().await {
                Ok((stream, _addr)) => {
                    // Verify that the connecting process has the same UID.
                    #[cfg(unix)]
                    {
                        let my_uid = unsafe { libc::getuid() };
                        match stream.peer_cred() {
                            Ok(cred) => {
                                if cred.uid() != my_uid {
                                    log::warn!("rejected connection from UID {}", cred.uid());
                                    continue;
                                }
                            }
                            Err(e) => {
                                log::warn!("failed to get peer credentials: {e}");
                                continue;
                            }
                        }
                    }

                    let tx = cmd_tx.clone();
                    tokio::spawn(async move {
                        if let Err(e) = handle_client(stream, tx).await {
                            log::debug!("client disconnected: {e}");
                        }
                    });
                }
                Err(e) => {
                    log::error!("accept error: {e}");
                }
            }
        }
    });

    Ok(handle)
}
