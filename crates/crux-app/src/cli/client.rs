use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use crux_protocol::{decode_frame, encode_frame, JsonRpcId, JsonRpcRequest, JsonRpcResponse};

/// Connect to a running Crux IPC server.
pub fn connect() -> Result<IpcClient> {
    let socket = find_socket()?;
    let stream = UnixStream::connect(&socket)
        .with_context(|| format!("failed to connect to {}", socket.display()))?;
    Ok(IpcClient { stream, next_id: 1 })
}

fn find_socket() -> Result<PathBuf> {
    // 1. $CRUX_SOCKET
    if let Ok(path) = std::env::var("CRUX_SOCKET") {
        let p = PathBuf::from(&path);
        if p.exists() {
            return Ok(p);
        }
    }

    // 2. Use crux_ipc::discover_socket
    if let Some(path) = crux_ipc::discover_socket() {
        return Ok(path);
    }

    bail!("no running Crux instance found. Is Crux running?")
}

pub struct IpcClient {
    stream: UnixStream,
    next_id: u64,
}

impl IpcClient {
    /// Send a JSON-RPC request and wait for the response.
    pub fn call(&mut self, method: &str, params: serde_json::Value) -> Result<serde_json::Value> {
        let id = self.next_id;
        self.next_id += 1;

        let request = JsonRpcRequest::new(JsonRpcId::Number(id), method, Some(params));
        let req_bytes = serde_json::to_vec(&request)?;
        let frame = encode_frame(&req_bytes)?;

        self.stream.write_all(&frame)?;
        self.stream.flush()?;

        // Read response with length-prefix framing.
        let mut buf = vec![0u8; 8192];
        let mut pending = Vec::new();

        loop {
            let n = self.stream.read(&mut buf)?;
            if n == 0 {
                bail!("server closed connection");
            }
            pending.extend_from_slice(&buf[..n]);

            if let Some((_consumed, payload)) = decode_frame(&pending)? {
                let response: JsonRpcResponse = serde_json::from_slice(&payload)?;
                if let Some(err) = response.error {
                    bail!("server error {}: {}", err.code, err.message);
                }
                return Ok(response.result.unwrap_or(serde_json::Value::Null));
            }
        }
    }
}
