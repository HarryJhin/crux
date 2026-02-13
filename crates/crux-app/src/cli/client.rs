pub use crux_ipc::IpcClient;

/// Connect to a running Crux IPC server.
pub fn connect() -> anyhow::Result<IpcClient> {
    IpcClient::connect()
}
