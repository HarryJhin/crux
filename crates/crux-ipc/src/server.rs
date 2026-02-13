//! Main IPC server — accepts connections on a Unix domain socket.

use std::path::PathBuf;
use std::sync::Arc;

use tokio::net::UnixListener;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use crate::command::IpcCommand;
use crate::handler::handle_client;

/// Maximum number of concurrent client connections.
const MAX_CONNECTIONS: usize = 64;

/// Start the IPC server on a background tokio task.
///
/// Binds to `socket_path`, verifies peer credentials on each connection, and
/// spawns a per-client handler that communicates with the GPUI thread through
/// `cmd_tx`.
///
/// The `cancel` token allows graceful shutdown — when cancelled, the server
/// stops accepting new connections, cleans up the socket file, and exits.
///
/// Returns the server task handle.
pub async fn start_server(
    socket_path: PathBuf,
    cmd_tx: mpsc::Sender<IpcCommand>,
    cancel: CancellationToken,
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

    // Fix 9: Connection limit semaphore.
    let semaphore = Arc::new(tokio::sync::Semaphore::new(MAX_CONNECTIONS));

    let socket_path_cleanup = socket_path.clone();
    let handle = tokio::spawn(async move {
        loop {
            // Fix 8: Graceful shutdown via CancellationToken.
            tokio::select! {
                _ = cancel.cancelled() => {
                    log::info!("IPC server shutting down");
                    // Fix 10: Clean up socket file on shutdown.
                    let _ = std::fs::remove_file(&socket_path_cleanup);
                    break;
                }
                result = listener.accept() => {
                    match result {
                        Ok((stream, _addr)) => {
                            // Verify that the connecting process has the same UID.
                            #[cfg(unix)]
                            {
                                let my_uid = unsafe { libc::getuid() };
                                match stream.peer_cred() {
                                    Ok(cred) => {
                                        if cred.uid() != my_uid {
                                            log::warn!(
                                                "rejected connection from UID {}",
                                                cred.uid()
                                            );
                                            continue;
                                        }
                                        log::info!("client connected (UID {})", cred.uid());
                                    }
                                    Err(e) => {
                                        log::warn!("failed to get peer credentials: {e}");
                                        continue;
                                    }
                                }
                            }

                            // Fix 9: Acquire a permit before spawning.
                            let permit = match semaphore.clone().try_acquire_owned() {
                                Ok(permit) => permit,
                                Err(_) => {
                                    log::warn!("connection limit reached, rejecting client");
                                    continue;
                                }
                            };

                            let tx = cmd_tx.clone();
                            tokio::spawn(async move {
                                // Wrap client handler with timeout.
                                use tokio::time::{timeout, Duration};
                                const CLIENT_TIMEOUT: Duration = Duration::from_secs(300); // 5 minutes

                                match timeout(CLIENT_TIMEOUT, handle_client(stream, tx)).await {
                                    Ok(Ok(())) => {
                                        log::debug!("client disconnected gracefully");
                                    }
                                    Ok(Err(e)) => {
                                        log::debug!("client disconnected: {e}");
                                    }
                                    Err(_) => {
                                        log::warn!("client timed out after {} seconds", CLIENT_TIMEOUT.as_secs());
                                    }
                                }
                                drop(permit); // Release on disconnect.
                            });
                        }
                        Err(e) => {
                            log::error!("accept error: {e}");
                        }
                    }
                }
            }
        }
    });

    Ok(handle)
}
