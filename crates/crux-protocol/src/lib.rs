//! Shared protocol types for IPC and terminal communication.
//!
//! This crate defines the JSON-RPC 2.0 message types, protocol method
//! parameters/results, and length-prefix framing used by Crux's IPC layer.

use std::collections::HashMap;
use std::fmt;

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Core ID types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PaneId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct WindowId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TabId(pub u64);

impl fmt::Display for PaneId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl fmt::Display for WindowId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl fmt::Display for TabId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

// ---------------------------------------------------------------------------
// Enums
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SplitDirection {
    Right,
    Left,
    Top,
    Bottom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum SplitSize {
    Percent(u8),
    Cells(u32),
}

// ---------------------------------------------------------------------------
// PaneInfo / PaneSize
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PaneInfo {
    pub pane_id: PaneId,
    pub window_id: WindowId,
    pub tab_id: TabId,
    pub size: PaneSize,
    pub title: String,
    pub cwd: Option<String>,
    pub is_active: bool,
    pub is_zoomed: bool,
    pub cursor_x: u32,
    pub cursor_y: u32,
    pub tty: Option<String>,
    pub pid: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PaneSize {
    pub rows: u32,
    pub cols: u32,
}

// ---------------------------------------------------------------------------
// JSON-RPC 2.0 types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: u64,
    pub method: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

impl JsonRpcRequest {
    pub fn new(id: u64, method: impl Into<String>, params: Option<serde_json::Value>) -> Self {
        Self {
            jsonrpc: "2.0".into(),
            id,
            method: method.into(),
            params,
        }
    }
}

impl JsonRpcResponse {
    pub fn success(id: u64, result: serde_json::Value) -> Self {
        Self {
            jsonrpc: "2.0".into(),
            id,
            result: Some(result),
            error: None,
        }
    }

    pub fn error(id: u64, code: i32, message: impl Into<String>) -> Self {
        Self {
            jsonrpc: "2.0".into(),
            id,
            result: None,
            error: Some(JsonRpcError {
                code,
                message: message.into(),
                data: None,
            }),
        }
    }
}

// ---------------------------------------------------------------------------
// Protocol method params & results
// ---------------------------------------------------------------------------

/// Parameters for `crux:pane/split`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SplitPaneParams {
    pub target_pane_id: Option<PaneId>,
    pub direction: SplitDirection,
    pub size: Option<SplitSize>,
    pub cwd: Option<String>,
    pub command: Option<Vec<String>>,
    pub env: Option<HashMap<String, String>>,
}

/// Result of `crux:pane/split`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SplitPaneResult {
    pub pane_id: PaneId,
    pub window_id: WindowId,
    pub tab_id: TabId,
    pub size: PaneSize,
    pub tty: Option<String>,
}

/// Parameters for `crux:pane/send-text`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendTextParams {
    pub pane_id: Option<PaneId>,
    pub text: String,
    #[serde(default)]
    pub bracketed_paste: bool,
}

/// Result of `crux:pane/send-text`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SendTextResult {
    pub bytes_written: usize,
}

/// Parameters for `crux:pane/get-text`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetTextParams {
    pub pane_id: Option<PaneId>,
    pub start_line: Option<i32>,
    pub end_line: Option<i32>,
    #[serde(default)]
    pub include_escapes: bool,
}

/// Result of `crux:pane/get-text`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetTextResult {
    pub lines: Vec<String>,
    pub first_line: i32,
    pub cursor_row: u32,
    pub cursor_col: u32,
}

/// Result of `crux:pane/list`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListPanesResult {
    pub panes: Vec<PaneInfo>,
}

/// Parameters for `crux:pane/activate`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ActivatePaneParams {
    pub pane_id: PaneId,
}

/// Parameters for `crux:pane/close`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClosePaneParams {
    pub pane_id: PaneId,
    #[serde(default)]
    pub force: bool,
}

/// Parameters for `crux:handshake`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HandshakeParams {
    pub client_name: String,
    pub client_version: String,
    pub protocol_version: String,
    pub capabilities: Vec<String>,
}

/// Result of `crux:handshake`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HandshakeResult {
    pub server_name: String,
    pub server_version: String,
    pub protocol_version: String,
    pub supported_capabilities: Vec<String>,
}

// ---------------------------------------------------------------------------
// Method name constants
// ---------------------------------------------------------------------------

pub mod method {
    pub const HANDSHAKE: &str = "crux:handshake";
    pub const PANE_SPLIT: &str = "crux:pane/split";
    pub const PANE_SEND_TEXT: &str = "crux:pane/send-text";
    pub const PANE_GET_TEXT: &str = "crux:pane/get-text";
    pub const PANE_LIST: &str = "crux:pane/list";
    pub const PANE_ACTIVATE: &str = "crux:pane/activate";
    pub const PANE_CLOSE: &str = "crux:pane/close";
}

// ---------------------------------------------------------------------------
// Error code constants
// ---------------------------------------------------------------------------

pub mod error_code {
    // Standard JSON-RPC
    pub const PARSE_ERROR: i32 = -32700;
    pub const INVALID_REQUEST: i32 = -32600;
    pub const METHOD_NOT_FOUND: i32 = -32601;
    pub const INVALID_PARAMS: i32 = -32602;
    pub const INTERNAL_ERROR: i32 = -32603;

    // Crux-specific
    pub const PANE_NOT_FOUND: i32 = -1001;
    pub const WINDOW_NOT_FOUND: i32 = -1002;
    pub const HANDSHAKE_REQUIRED: i32 = -1003;
}

// ---------------------------------------------------------------------------
// Length-prefix framing
// ---------------------------------------------------------------------------

/// Encode a message with a 4-byte big-endian length prefix.
pub fn encode_frame(msg: &[u8]) -> Vec<u8> {
    let len = msg.len() as u32;
    let mut frame = Vec::with_capacity(4 + msg.len());
    frame.extend_from_slice(&len.to_be_bytes());
    frame.extend_from_slice(msg);
    frame
}

/// Decode a frame from a buffer.
///
/// Returns `Some((total_consumed_bytes, payload))` if a complete frame is
/// available, or `None` if the buffer is incomplete.
pub fn decode_frame(buf: &[u8]) -> Option<(usize, Vec<u8>)> {
    if buf.len() < 4 {
        return None;
    }
    let len = u32::from_be_bytes([buf[0], buf[1], buf[2], buf[3]]) as usize;
    if buf.len() < 4 + len {
        return None;
    }
    Some((4 + len, buf[4..4 + len].to_vec()))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_round_trip() {
        let payload = b"hello world";
        let frame = encode_frame(payload);
        let (consumed, decoded) = decode_frame(&frame).expect("should decode");
        assert_eq!(consumed, frame.len());
        assert_eq!(decoded, payload);
    }

    #[test]
    fn frame_decode_incomplete_header() {
        assert!(decode_frame(&[0x00, 0x00]).is_none());
    }

    #[test]
    fn frame_decode_incomplete_payload() {
        let frame = encode_frame(b"hello");
        // Chop off the last byte so payload is incomplete.
        assert!(decode_frame(&frame[..frame.len() - 1]).is_none());
    }

    #[test]
    fn jsonrpc_request_serde() {
        let req = JsonRpcRequest::new(1, method::PANE_LIST, None);
        let json = serde_json::to_string(&req).unwrap();
        let parsed: JsonRpcRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.jsonrpc, "2.0");
        assert_eq!(parsed.id, 1);
        assert_eq!(parsed.method, "crux:pane/list");
        assert!(parsed.params.is_none());
    }

    #[test]
    fn jsonrpc_response_success_serde() {
        let resp = JsonRpcResponse::success(42, serde_json::json!({"ok": true}));
        let json = serde_json::to_string(&resp).unwrap();
        // "error" field should be omitted
        assert!(!json.contains("\"error\""));
        let parsed: JsonRpcResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.id, 42);
        assert!(parsed.result.is_some());
        assert!(parsed.error.is_none());
    }

    #[test]
    fn jsonrpc_response_error_serde() {
        let resp = JsonRpcResponse::error(7, error_code::PANE_NOT_FOUND, "pane 99 not found");
        let json = serde_json::to_string(&resp).unwrap();
        // "result" field should be omitted
        assert!(!json.contains("\"result\""));
        let parsed: JsonRpcResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.error.as_ref().unwrap().code, -1001);
    }

    #[test]
    fn pane_id_display() {
        assert_eq!(PaneId(42).to_string(), "42");
        assert_eq!(WindowId(0).to_string(), "0");
        assert_eq!(TabId(100).to_string(), "100");
    }

    #[test]
    fn split_direction_serde() {
        let dir = SplitDirection::Right;
        let json = serde_json::to_string(&dir).unwrap();
        assert_eq!(json, "\"right\"");
        let parsed: SplitDirection = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, SplitDirection::Right);
    }

    #[test]
    fn split_size_serde() {
        let percent = SplitSize::Percent(50);
        let json = serde_json::to_string(&percent).unwrap();
        let parsed: SplitSize = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, percent);

        let cells = SplitSize::Cells(80);
        let json = serde_json::to_string(&cells).unwrap();
        let parsed: SplitSize = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, cells);
    }
}
