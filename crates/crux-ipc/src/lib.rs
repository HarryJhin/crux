//! Unix domain socket server and JSON-RPC 2.0 protocol handler.
//!
//! This crate provides the IPC layer for Crux. The server listens on a Unix
//! domain socket, speaks length-prefixed JSON-RPC 2.0, and bridges incoming
//! requests to the GPUI main thread via an [`mpsc`](tokio::sync::mpsc) channel
//! of [`IpcCommand`] values.
//!
//! # Usage
//!
//! ```rust,ignore
//! let (socket_path, mut cmd_rx) = crux_ipc::start_ipc()?;
//! // Poll cmd_rx on the GPUI main thread to handle IPC commands.
//! ```

pub mod command;
pub mod handler;
pub mod server;
pub mod socket;

pub use command::IpcCommand;
pub use socket::{discover_socket, socket_path};

use std::path::PathBuf;
use tokio::sync::mpsc;

/// Start the IPC server on a dedicated thread with its own tokio runtime.
///
/// Returns `(socket_path, command_receiver)`. The caller (GPUI main thread)
/// should poll the receiver to handle incoming [`IpcCommand`]s.
pub fn start_ipc() -> anyhow::Result<(PathBuf, mpsc::Receiver<IpcCommand>)> {
    let path = socket::socket_path();
    let path_for_thread = path.clone();
    let (cmd_tx, cmd_rx) = mpsc::channel(64);

    std::thread::spawn(move || {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("failed to create tokio runtime for IPC server");

        rt.block_on(async move {
            match server::start_server(path_for_thread, cmd_tx).await {
                Ok(handle) => {
                    let _ = handle.await;
                }
                Err(e) => {
                    log::error!("failed to start IPC server: {e}");
                }
            }
        });
    });

    Ok((path, cmd_rx))
}
