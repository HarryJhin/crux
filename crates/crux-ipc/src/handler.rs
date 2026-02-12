//! Per-client connection handler.
//!
//! Reads length-prefixed JSON-RPC frames from a [`tokio::net::UnixStream`],
//! dispatches commands via an [`mpsc`] channel, and writes back responses.

use serde::Serialize;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;
use tokio::sync::{mpsc, oneshot};

use crux_protocol::{
    decode_frame, encode_frame, error_code, method, JsonRpcRequest, JsonRpcResponse,
};

use crate::command::IpcCommand;

/// Handle a single client connection.
pub async fn handle_client(
    mut stream: UnixStream,
    cmd_tx: mpsc::Sender<IpcCommand>,
) -> anyhow::Result<()> {
    let mut buf = vec![0u8; 8192];
    let mut pending = Vec::new();

    loop {
        let n = stream.read(&mut buf).await?;
        if n == 0 {
            break; // client disconnected
        }

        pending.extend_from_slice(&buf[..n]);

        // Process all complete frames in the buffer.
        while let Some((consumed, payload)) = decode_frame(&pending) {
            let request: JsonRpcRequest = match serde_json::from_slice(&payload) {
                Ok(r) => r,
                Err(e) => {
                    // We can't know the id if parsing failed, use 0.
                    let resp = JsonRpcResponse::error(
                        0,
                        error_code::PARSE_ERROR,
                        format!("invalid JSON-RPC request: {e}"),
                    );
                    let resp_bytes = serde_json::to_vec(&resp)?;
                    stream.write_all(&encode_frame(&resp_bytes)).await?;
                    pending.drain(..consumed);
                    continue;
                }
            };

            let response = dispatch_request(request, &cmd_tx).await;
            let resp_bytes = serde_json::to_vec(&response)?;
            stream.write_all(&encode_frame(&resp_bytes)).await?;

            pending.drain(..consumed);
        }
    }

    Ok(())
}

/// Route a JSON-RPC request to the appropriate handler.
async fn dispatch_request(
    req: JsonRpcRequest,
    cmd_tx: &mpsc::Sender<IpcCommand>,
) -> JsonRpcResponse {
    match req.method.as_str() {
        method::HANDSHAKE => {
            dispatch_with_params(req.id, req.params, cmd_tx, |params, reply| {
                IpcCommand::Handshake { params, reply }
            })
            .await
        }
        method::PANE_SPLIT => {
            dispatch_with_params(req.id, req.params, cmd_tx, |params, reply| {
                IpcCommand::SplitPane { params, reply }
            })
            .await
        }
        method::PANE_SEND_TEXT => {
            dispatch_with_params(req.id, req.params, cmd_tx, |params, reply| {
                IpcCommand::SendText { params, reply }
            })
            .await
        }
        method::PANE_GET_TEXT => {
            dispatch_with_params(req.id, req.params, cmd_tx, |params, reply| {
                IpcCommand::GetText { params, reply }
            })
            .await
        }
        method::PANE_LIST => {
            send_command(req.id, cmd_tx, |reply| IpcCommand::ListPanes { reply }).await
        }
        method::PANE_ACTIVATE => {
            dispatch_with_params_unit(req.id, req.params, cmd_tx, |params, reply| {
                IpcCommand::ActivatePane { params, reply }
            })
            .await
        }
        method::PANE_CLOSE => {
            dispatch_with_params_unit(req.id, req.params, cmd_tx, |params, reply| {
                IpcCommand::ClosePane { params, reply }
            })
            .await
        }
        _ => JsonRpcResponse::error(
            req.id,
            error_code::METHOD_NOT_FOUND,
            format!("unknown method: {}", req.method),
        ),
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Parse params, send a command that returns a serialisable result.
async fn dispatch_with_params<P, R>(
    id: u64,
    params: Option<serde_json::Value>,
    cmd_tx: &mpsc::Sender<IpcCommand>,
    make_cmd: impl FnOnce(P, oneshot::Sender<anyhow::Result<R>>) -> IpcCommand,
) -> JsonRpcResponse
where
    P: serde::de::DeserializeOwned,
    R: Serialize,
{
    let params: P = match parse_params(id, params) {
        Ok(p) => p,
        Err(resp) => return *resp,
    };
    send_command(id, cmd_tx, |reply| make_cmd(params, reply)).await
}

/// Parse params, send a command that returns `()` (mapped to `{"success": true}`).
async fn dispatch_with_params_unit<P>(
    id: u64,
    params: Option<serde_json::Value>,
    cmd_tx: &mpsc::Sender<IpcCommand>,
    make_cmd: impl FnOnce(P, oneshot::Sender<anyhow::Result<()>>) -> IpcCommand,
) -> JsonRpcResponse
where
    P: serde::de::DeserializeOwned,
{
    let params: P = match parse_params(id, params) {
        Ok(p) => p,
        Err(resp) => return *resp,
    };
    send_command_unit(id, cmd_tx, |reply| make_cmd(params, reply)).await
}

/// Extract and deserialise `params` from a JSON-RPC request value.
fn parse_params<P: serde::de::DeserializeOwned>(
    id: u64,
    params: Option<serde_json::Value>,
) -> Result<P, Box<JsonRpcResponse>> {
    let value = params.unwrap_or(serde_json::Value::Null);
    serde_json::from_value(value).map_err(|e| {
        Box::new(JsonRpcResponse::error(
            id,
            error_code::INVALID_PARAMS,
            format!("invalid params: {e}"),
        ))
    })
}

/// Send a command through the channel and await a serialisable result.
async fn send_command<T: Serialize>(
    id: u64,
    cmd_tx: &mpsc::Sender<IpcCommand>,
    make_cmd: impl FnOnce(oneshot::Sender<anyhow::Result<T>>) -> IpcCommand,
) -> JsonRpcResponse {
    let (tx, rx) = oneshot::channel();
    if cmd_tx.send(make_cmd(tx)).await.is_err() {
        return JsonRpcResponse::error(id, error_code::INTERNAL_ERROR, "server shutting down");
    }
    match rx.await {
        Ok(Ok(result)) => match serde_json::to_value(result) {
            Ok(v) => JsonRpcResponse::success(id, v),
            Err(e) => JsonRpcResponse::error(id, error_code::INTERNAL_ERROR, e.to_string()),
        },
        Ok(Err(e)) => JsonRpcResponse::error(id, error_code::INTERNAL_ERROR, e.to_string()),
        Err(_) => JsonRpcResponse::error(id, error_code::INTERNAL_ERROR, "handler dropped"),
    }
}

/// Send a command that returns `()`, mapped to `{"success": true}`.
async fn send_command_unit(
    id: u64,
    cmd_tx: &mpsc::Sender<IpcCommand>,
    make_cmd: impl FnOnce(oneshot::Sender<anyhow::Result<()>>) -> IpcCommand,
) -> JsonRpcResponse {
    let (tx, rx) = oneshot::channel();
    if cmd_tx.send(make_cmd(tx)).await.is_err() {
        return JsonRpcResponse::error(id, error_code::INTERNAL_ERROR, "server shutting down");
    }
    match rx.await {
        Ok(Ok(())) => JsonRpcResponse::success(id, serde_json::json!({"success": true})),
        Ok(Err(e)) => JsonRpcResponse::error(id, error_code::INTERNAL_ERROR, e.to_string()),
        Err(_) => JsonRpcResponse::error(id, error_code::INTERNAL_ERROR, "handler dropped"),
    }
}
