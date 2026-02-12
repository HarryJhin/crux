//! Per-client connection handler.
//!
//! Reads length-prefixed JSON-RPC frames from a [`tokio::net::UnixStream`],
//! dispatches commands via an [`mpsc`] channel, and writes back responses.

use serde::Serialize;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixStream;
use tokio::sync::{mpsc, oneshot};

use crux_protocol::{
    decode_frame, encode_frame, error_code, method, JsonRpcId, JsonRpcRequest, JsonRpcResponse,
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
        loop {
            let (consumed, payload) = match decode_frame(&pending) {
                Ok(Some(frame)) => frame,
                Ok(None) => break, // incomplete frame
                Err(e) => {
                    // Frame-level error (e.g. oversized). Send parse error and
                    // drop the connection since we can't reliably re-sync.
                    let resp = JsonRpcResponse::error(
                        JsonRpcId::Null,
                        error_code::PARSE_ERROR,
                        format!("frame error: {e}"),
                    );
                    let resp_bytes = serde_json::to_vec(&resp)?;
                    if let Ok(frame) = encode_frame(&resp_bytes) {
                        let _ = stream.write_all(&frame).await;
                    }
                    return Ok(());
                }
            };

            // Try to parse the payload as a JSON value first to support batch requests.
            let value: serde_json::Value = match serde_json::from_slice(&payload) {
                Ok(v) => v,
                Err(e) => {
                    // Fix 7: Parse error uses Null id.
                    let resp = JsonRpcResponse::error(
                        JsonRpcId::Null,
                        error_code::PARSE_ERROR,
                        format!("invalid JSON: {e}"),
                    );
                    let resp_bytes = serde_json::to_vec(&resp)?;
                    if let Ok(frame) = encode_frame(&resp_bytes) {
                        stream.write_all(&frame).await?;
                    }
                    pending.drain(..consumed);
                    continue;
                }
            };

            // Fix 6: Batch request support.
            match value {
                serde_json::Value::Array(arr) => {
                    if arr.is_empty() {
                        let resp = JsonRpcResponse::error(
                            JsonRpcId::Null,
                            error_code::INVALID_REQUEST,
                            "empty batch request".to_string(),
                        );
                        let resp_bytes = serde_json::to_vec(&resp)?;
                        if let Ok(frame) = encode_frame(&resp_bytes) {
                            stream.write_all(&frame).await?;
                        }
                    } else {
                        let mut responses = Vec::new();
                        for item in arr {
                            match serde_json::from_value::<JsonRpcRequest>(item) {
                                Ok(request) => {
                                    if let Some(resp) =
                                        dispatch_request(request, &cmd_tx).await
                                    {
                                        responses.push(resp);
                                    }
                                    // Notifications (None returned) are not added.
                                }
                                Err(e) => {
                                    responses.push(JsonRpcResponse::error(
                                        JsonRpcId::Null,
                                        error_code::INVALID_REQUEST,
                                        format!("invalid request in batch: {e}"),
                                    ));
                                }
                            }
                        }
                        // Only send a response if there are any (all-notification batch
                        // produces no response).
                        if !responses.is_empty() {
                            let resp_bytes = serde_json::to_vec(&responses)?;
                            if let Ok(frame) = encode_frame(&resp_bytes) {
                                stream.write_all(&frame).await?;
                            }
                        }
                    }
                }
                _ => {
                    // Single request.
                    let request: JsonRpcRequest = match serde_json::from_value(value) {
                        Ok(r) => r,
                        Err(e) => {
                            let resp = JsonRpcResponse::error(
                                JsonRpcId::Null,
                                error_code::INVALID_REQUEST,
                                format!("invalid JSON-RPC request: {e}"),
                            );
                            let resp_bytes = serde_json::to_vec(&resp)?;
                            if let Ok(frame) = encode_frame(&resp_bytes) {
                                stream.write_all(&frame).await?;
                            }
                            pending.drain(..consumed);
                            continue;
                        }
                    };

                    // Fix 5: Only send response for non-notification requests.
                    if let Some(response) = dispatch_request(request, &cmd_tx).await {
                        let resp_bytes = serde_json::to_vec(&response)?;
                        if let Ok(frame) = encode_frame(&resp_bytes) {
                            stream.write_all(&frame).await?;
                        }
                    }
                }
            }

            pending.drain(..consumed);
        }
    }

    Ok(())
}

/// Route a JSON-RPC request to the appropriate handler.
///
/// Returns `None` for notifications (requests without an id) per JSON-RPC 2.0 spec.
async fn dispatch_request(
    req: JsonRpcRequest,
    cmd_tx: &mpsc::Sender<IpcCommand>,
) -> Option<JsonRpcResponse> {
    // Fix 4: Validate jsonrpc version.
    if req.jsonrpc != "2.0" {
        return Some(JsonRpcResponse::error(
            req.id.clone().unwrap_or(JsonRpcId::Null),
            error_code::INVALID_REQUEST,
            "Invalid JSON-RPC version, must be \"2.0\"".to_string(),
        ));
    }

    // Fix 5: If id is None, this is a notification — process but don't respond.
    let is_notification = req.id.is_none();
    let id = req.id.clone().unwrap_or(JsonRpcId::Null);

    let response = match req.method.as_str() {
        method::HANDSHAKE => {
            dispatch_with_params(id.clone(), req.params, cmd_tx, |params, reply| {
                IpcCommand::Handshake { params, reply }
            })
            .await
        }
        method::PANE_SPLIT => {
            dispatch_with_params(id.clone(), req.params, cmd_tx, |params, reply| {
                IpcCommand::SplitPane { params, reply }
            })
            .await
        }
        method::PANE_SEND_TEXT => {
            dispatch_with_params(id.clone(), req.params, cmd_tx, |params, reply| {
                IpcCommand::SendText { params, reply }
            })
            .await
        }
        method::PANE_GET_TEXT => {
            dispatch_with_params(id.clone(), req.params, cmd_tx, |params, reply| {
                IpcCommand::GetText { params, reply }
            })
            .await
        }
        method::PANE_GET_SELECTION => {
            dispatch_with_params(id.clone(), req.params, cmd_tx, |params, reply| {
                IpcCommand::GetSelection { params, reply }
            })
            .await
        }
        method::PANE_GET_SNAPSHOT => {
            dispatch_with_params(id.clone(), req.params, cmd_tx, |params, reply| {
                IpcCommand::GetSnapshot { params, reply }
            })
            .await
        }
        method::PANE_LIST => {
            send_command(id.clone(), cmd_tx, |reply| IpcCommand::ListPanes { reply }).await
        }
        method::PANE_RESIZE => {
            dispatch_with_params_unit(id.clone(), req.params, cmd_tx, |params, reply| {
                IpcCommand::ResizePane { params, reply }
            })
            .await
        }
        method::PANE_ACTIVATE => {
            dispatch_with_params_unit(id.clone(), req.params, cmd_tx, |params, reply| {
                IpcCommand::ActivatePane { params, reply }
            })
            .await
        }
        method::PANE_CLOSE => {
            dispatch_with_params_unit(id.clone(), req.params, cmd_tx, |params, reply| {
                IpcCommand::ClosePane { params, reply }
            })
            .await
        }
        method::WINDOW_CREATE => {
            dispatch_with_params(id.clone(), req.params, cmd_tx, |params, reply| {
                IpcCommand::WindowCreate { params, reply }
            })
            .await
        }
        method::WINDOW_LIST => {
            send_command(id.clone(), cmd_tx, |reply| IpcCommand::WindowList { reply }).await
        }
        method::SESSION_SAVE => {
            dispatch_with_params(id.clone(), req.params, cmd_tx, |params, reply| {
                IpcCommand::SessionSave { params, reply }
            })
            .await
        }
        method::SESSION_LOAD => {
            dispatch_with_params(id.clone(), req.params, cmd_tx, |params, reply| {
                IpcCommand::SessionLoad { params, reply }
            })
            .await
        }
        _ => JsonRpcResponse::error(
            id,
            error_code::METHOD_NOT_FOUND,
            format!("unknown method: {}", req.method),
        ),
    };

    if is_notification {
        None
    } else {
        Some(response)
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Parse params, send a command that returns a serialisable result.
async fn dispatch_with_params<P, R>(
    id: JsonRpcId,
    params: Option<serde_json::Value>,
    cmd_tx: &mpsc::Sender<IpcCommand>,
    make_cmd: impl FnOnce(P, oneshot::Sender<anyhow::Result<R>>) -> IpcCommand,
) -> JsonRpcResponse
where
    P: serde::de::DeserializeOwned,
    R: Serialize,
{
    let params: P = match parse_params(id.clone(), params) {
        Ok(p) => p,
        Err(resp) => return *resp,
    };
    send_command(id, cmd_tx, |reply| make_cmd(params, reply)).await
}

/// Parse params, send a command that returns `()` (mapped to `{"success": true}`).
async fn dispatch_with_params_unit<P>(
    id: JsonRpcId,
    params: Option<serde_json::Value>,
    cmd_tx: &mpsc::Sender<IpcCommand>,
    make_cmd: impl FnOnce(P, oneshot::Sender<anyhow::Result<()>>) -> IpcCommand,
) -> JsonRpcResponse
where
    P: serde::de::DeserializeOwned,
{
    let params: P = match parse_params(id.clone(), params) {
        Ok(p) => p,
        Err(resp) => return *resp,
    };
    send_command_unit(id, cmd_tx, |reply| make_cmd(params, reply)).await
}

/// Extract and deserialise `params` from a JSON-RPC request value.
fn parse_params<P: serde::de::DeserializeOwned>(
    id: JsonRpcId,
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
    id: JsonRpcId,
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
    id: JsonRpcId,
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
