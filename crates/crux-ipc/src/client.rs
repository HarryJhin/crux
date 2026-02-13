//! Synchronous IPC client for connecting to a running Crux instance.

use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use crux_protocol::{decode_frame, encode_frame, JsonRpcId, JsonRpcRequest, JsonRpcResponse};

/// Transport abstraction for IPC communication.
/// Enables testing MCP tools without a running Crux instance.
pub trait IpcTransport: Send + Sync {
    fn call(&self, method: &str, params: serde_json::Value) -> Result<serde_json::Value>;
}

/// Synchronous IPC client that connects to the running Crux terminal via Unix socket.
///
/// Thread-safe via internal mutexes so it can be shared across async tasks
/// (e.g., wrapped in `Arc<IpcClient>` for the MCP server).
pub struct IpcClient {
    stream: Mutex<UnixStream>,
    next_id: Mutex<u64>,
}

impl IpcClient {
    /// Connect to a running Crux instance.
    ///
    /// Discovers the socket via `$CRUX_SOCKET` or [`discover_socket`](crate::discover_socket).
    pub fn connect() -> Result<Self> {
        let socket = find_socket()?;
        Self::connect_to(socket)
    }

    /// Connect to a specific socket path.
    pub fn connect_to(path: PathBuf) -> Result<Self> {
        let stream = UnixStream::connect(&path)
            .with_context(|| format!("failed to connect to {}", path.display()))?;
        stream.set_read_timeout(Some(Duration::from_secs(30)))?;
        Ok(Self {
            stream: Mutex::new(stream),
            next_id: Mutex::new(1),
        })
    }

    /// Connect with exponential backoff retry.
    pub fn connect_with_retry(max_attempts: u32) -> Result<Self> {
        let mut delay = Duration::from_millis(100);
        for attempt in 1..=max_attempts {
            match Self::connect() {
                Ok(client) => return Ok(client),
                Err(e) if attempt == max_attempts => return Err(e),
                Err(e) => {
                    log::info!(
                        "IPC connect attempt {}/{}: {}, retrying in {:?}",
                        attempt,
                        max_attempts,
                        e,
                        delay
                    );
                    std::thread::sleep(delay);
                    delay = std::cmp::min(delay * 2, Duration::from_secs(5));
                }
            }
        }
        unreachable!()
    }

    /// Send a JSON-RPC request and wait for the response.
    fn call_inner(&self, method: &str, params: serde_json::Value) -> Result<serde_json::Value> {
        let mut stream = self.stream.lock().map_err(|_| anyhow::anyhow!("IPC client mutex poisoned"))?;
        let mut next_id = self.next_id.lock().map_err(|_| anyhow::anyhow!("IPC client mutex poisoned"))?;
        let id = *next_id;
        *next_id += 1;

        let request = JsonRpcRequest::new(JsonRpcId::Number(id), method, Some(params));
        let req_bytes = serde_json::to_vec(&request)?;
        let frame =
            encode_frame(&req_bytes).map_err(|e| anyhow::anyhow!("frame encode error: {e}"))?;

        stream.write_all(&frame)?;
        stream.flush()?;

        let mut buf = vec![0u8; 65536];
        let mut pending = Vec::new();

        loop {
            let n = stream.read(&mut buf)?;
            if n == 0 {
                bail!("server closed connection");
            }
            pending.extend_from_slice(&buf[..n]);

            if let Some((_consumed, payload)) =
                decode_frame(&pending).map_err(|e| anyhow::anyhow!("frame decode error: {e}"))?
            {
                let response: JsonRpcResponse = serde_json::from_slice(&payload)?;
                if let Some(err) = response.error {
                    bail!("server error {}: {}", err.code, err.message);
                }
                return Ok(response.result.unwrap_or(serde_json::Value::Null));
            }
        }
    }
}

impl IpcTransport for IpcClient {
    fn call(&self, method: &str, params: serde_json::Value) -> Result<serde_json::Value> {
        self.call_inner(method, params)
    }
}

/// Find the socket path for a running Crux instance.
fn find_socket() -> Result<PathBuf> {
    if let Ok(path) = std::env::var("CRUX_SOCKET") {
        let p = PathBuf::from(&path);
        if p.exists() {
            return Ok(p);
        }
    }

    if let Some(path) = crate::discover_socket() {
        return Ok(path);
    }

    bail!("no running Crux instance found. Is Crux running?")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn connect_fails_without_running_instance() {
        let result = IpcClient::connect();
        assert!(result.is_err());
    }

    #[test]
    fn connect_with_retry_fails_without_running_instance() {
        let result = IpcClient::connect_with_retry(1);
        assert!(
            result.is_err(),
            "connect_with_retry should fail without running Crux"
        );
    }

    #[test]
    fn find_socket_falls_through_nonexistent_env() {
        unsafe { std::env::set_var("CRUX_SOCKET", "/tmp/nonexistent-crux-socket-12345") };
        let result = find_socket();
        let _ = result;
        unsafe { std::env::remove_var("CRUX_SOCKET") };
    }

    #[test]
    fn find_socket_without_env_uses_discover() {
        std::env::remove_var("CRUX_SOCKET");
        let result = find_socket();
        if result.is_err() {
            let err_msg = result.unwrap_err().to_string();
            assert!(
                err_msg.contains("no running Crux instance found"),
                "expected 'no running Crux instance' error, got: {err_msg}",
            );
        }
    }

    #[test]
    fn jsonrpc_request_serialization() {
        let request = JsonRpcRequest::new(
            JsonRpcId::Number(1),
            "test_method",
            Some(serde_json::json!({"key": "value"})),
        );
        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("\"jsonrpc\":\"2.0\""));
        assert!(json.contains("\"method\":\"test_method\""));
        assert!(json.contains("\"id\":1"));
    }
}
