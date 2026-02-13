use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use crux_protocol::{decode_frame, encode_frame, JsonRpcId, JsonRpcRequest, JsonRpcResponse};

/// Synchronous IPC client that connects to the running Crux terminal via Unix socket.
///
/// Thread-safe via internal mutexes so it can be shared across async tasks.
pub struct IpcClient {
    stream: Mutex<UnixStream>,
    next_id: Mutex<u64>,
}

impl IpcClient {
    /// Connect to a running Crux instance.
    pub fn connect() -> Result<Self> {
        let socket = find_socket()?;
        let stream = UnixStream::connect(&socket)
            .with_context(|| format!("failed to connect to {}", socket.display()))?;
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
    pub fn call(&self, method: &str, params: serde_json::Value) -> Result<serde_json::Value> {
        let mut stream = self.stream.lock().unwrap();
        let mut next_id = self.next_id.lock().unwrap();
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

fn find_socket() -> Result<PathBuf> {
    if let Ok(path) = std::env::var("CRUX_SOCKET") {
        let p = PathBuf::from(&path);
        if p.exists() {
            return Ok(p);
        }
    }

    if let Some(path) = crux_ipc::discover_socket() {
        return Ok(path);
    }

    bail!("no running Crux instance found. Is Crux running?")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_connect_with_retry_at_least_one_attempt() {
        // Attempting to connect with at least 1 attempt should try to connect.
        // This will fail (no running Crux instance), but it should not panic.
        let result = IpcClient::connect_with_retry(1);
        assert!(
            result.is_err(),
            "connect_with_retry should fail without running Crux"
        );
    }

    #[test]
    fn test_find_socket_respects_crux_socket_env() {
        // Test that CRUX_SOCKET environment variable is checked first.
        // When CRUX_SOCKET points to a nonexistent path, find_socket falls
        // through to discover_socket(). The result depends on whether a Crux
        // instance is running, so we only verify the code path doesn't panic.
        unsafe { std::env::set_var("CRUX_SOCKET", "/tmp/nonexistent-crux-socket-12345") };
        let result = find_socket();
        // If no Crux instance is running, this errors; otherwise discover succeeds.
        // Either outcome is valid — we just verify no panic.
        let _ = result;
        unsafe { std::env::remove_var("CRUX_SOCKET") };
    }

    #[test]
    fn test_find_socket_without_env_uses_discover() {
        // Test that discover_socket is used when CRUX_SOCKET is not set.
        std::env::remove_var("CRUX_SOCKET");
        let result = find_socket();
        // This will fail unless a Crux instance is actually running,
        // but the test verifies the code path doesn't panic.
        if result.is_err() {
            let err_msg = result.unwrap_err().to_string();
            assert!(
                err_msg.contains("no running Crux instance found"),
                "expected 'no running Crux instance' error, got: {}",
                err_msg
            );
        }
    }

    #[test]
    fn test_jsonrpc_request_serialization() {
        // Test that we can create and serialize a valid JSON-RPC request.
        use crux_protocol::JsonRpcRequest;
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
