use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use crux_protocol::{decode_frame, encode_frame, JsonRpcRequest, JsonRpcResponse};

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

        let request = JsonRpcRequest::new(id, method, Some(params));
        let req_bytes = serde_json::to_vec(&request)?;
        let frame = encode_frame(&req_bytes);

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

            if let Some((_consumed, payload)) = decode_frame(&pending) {
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
