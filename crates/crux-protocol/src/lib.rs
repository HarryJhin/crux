//! Shared protocol types for IPC and terminal communication.
//!
//! This crate defines the JSON-RPC 2.0 message types, protocol method
//! parameters/results, and length-prefix framing used by Crux's IPC layer.

use std::collections::HashMap;
use std::fmt;

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// JSON-RPC 2.0 identifier
// ---------------------------------------------------------------------------

/// JSON-RPC 2.0 request/response identifier.
/// Can be a number, string, or null per the specification.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum JsonRpcId {
    Number(u64),
    String(String),
    Null,
}

impl fmt::Display for JsonRpcId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            JsonRpcId::Number(n) => write!(f, "{n}"),
            JsonRpcId::String(s) => write!(f, "{s}"),
            JsonRpcId::Null => write!(f, "null"),
        }
    }
}

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
// Pane events (for broadcasting lifecycle changes)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PaneEvent {
    Created { pane_id: PaneId },
    Closed { pane_id: PaneId },
    Focused { pane_id: PaneId },
    Resized { pane_id: PaneId, size: PaneSize },
    TitleChanged { pane_id: PaneId, title: String },
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
    /// `None` for JSON-RPC notifications (no response expected).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<JsonRpcId>,
    pub method: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub params: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: JsonRpcId,
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
    pub fn new(id: JsonRpcId, method: impl Into<String>, params: Option<serde_json::Value>) -> Self {
        Self {
            jsonrpc: "2.0".into(),
            id: Some(id),
            method: method.into(),
            params,
        }
    }

    /// Create a JSON-RPC notification (no id, no response expected).
    pub fn notification(method: impl Into<String>, params: Option<serde_json::Value>) -> Self {
        Self {
            jsonrpc: "2.0".into(),
            id: None,
            method: method.into(),
            params,
        }
    }
}

impl JsonRpcResponse {
    pub fn success(id: JsonRpcId, result: serde_json::Value) -> Self {
        Self {
            jsonrpc: "2.0".into(),
            id,
            result: Some(result),
            error: None,
        }
    }

    pub fn error(id: JsonRpcId, code: i32, message: impl Into<String>) -> Self {
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

/// Parameters for `crux:pane/get-selection`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetSelectionParams {
    pub pane_id: Option<PaneId>,
}

/// Result of `crux:pane/get-selection`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetSelectionResult {
    pub text: Option<String>,
    pub has_selection: bool,
}

/// Parameters for `crux:pane/get-snapshot`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetSnapshotParams {
    pub pane_id: Option<PaneId>,
}

/// Result of `crux:pane/get-snapshot`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetSnapshotResult {
    pub lines: Vec<String>,
    pub rows: u32,
    pub cols: u32,
    pub cursor_row: i32,
    pub cursor_col: u32,
    pub cursor_shape: String,
    pub display_offset: u32,
    pub has_selection: bool,
    pub title: Option<String>,
    pub cwd: Option<String>,
}

/// Result of `crux:pane/list`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListPanesResult {
    pub panes: Vec<PaneInfo>,
}

/// Parameters for `crux:pane/resize`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResizePaneParams {
    pub pane_id: PaneId,
    /// Desired width in pixels (applies when pane is in a horizontal split).
    pub width: Option<f32>,
    /// Desired height in pixels (applies when pane is in a vertical split).
    pub height: Option<f32>,
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

/// Parameters for `crux:window/create`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowCreateParams {
    pub title: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
}

/// Result of `crux:window/create`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowCreateResult {
    pub window_id: WindowId,
}

/// Info about a window, returned from `crux:window/list`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowInfo {
    pub window_id: WindowId,
    pub title: String,
    pub pane_count: u32,
    pub is_focused: bool,
}

/// Result of `crux:window/list`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowListResult {
    pub windows: Vec<WindowInfo>,
}

/// Parameters for `crux:session/save`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSaveParams {
    /// File path to save the session to. Uses default if omitted.
    pub path: Option<String>,
}

/// Result of `crux:session/save`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionSaveResult {
    pub path: String,
}

/// Parameters for `crux:session/load`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionLoadParams {
    /// File path to load the session from. Uses default if omitted.
    pub path: Option<String>,
}

/// Result of `crux:session/load`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionLoadResult {
    pub pane_count: u32,
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
    pub const PANE_RESIZE: &str = "crux:pane/resize";
    pub const PANE_ACTIVATE: &str = "crux:pane/activate";
    pub const PANE_CLOSE: &str = "crux:pane/close";
    pub const PANE_GET_SNAPSHOT: &str = "crux:pane/get-snapshot";
    pub const PANE_GET_SELECTION: &str = "crux:pane/get-selection";
    pub const WINDOW_CREATE: &str = "crux:window/create";
    pub const WINDOW_LIST: &str = "crux:window/list";
    pub const SESSION_SAVE: &str = "crux:session/save";
    pub const SESSION_LOAD: &str = "crux:session/load";
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

/// Maximum frame payload size (16 MB).
pub const MAX_FRAME_SIZE: usize = 16 * 1024 * 1024;

/// Errors that can occur during frame encoding/decoding.
#[derive(Debug)]
pub enum FrameError {
    /// The message exceeds [`MAX_FRAME_SIZE`].
    MessageTooLarge(usize),
}

impl fmt::Display for FrameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FrameError::MessageTooLarge(size) => {
                write!(f, "message too large: {size} bytes (max {MAX_FRAME_SIZE})")
            }
        }
    }
}

impl std::error::Error for FrameError {}

/// Encode a message with a 4-byte big-endian length prefix.
pub fn encode_frame(msg: &[u8]) -> Result<Vec<u8>, FrameError> {
    let len: u32 = msg
        .len()
        .try_into()
        .map_err(|_| FrameError::MessageTooLarge(msg.len()))?;
    if msg.len() > MAX_FRAME_SIZE {
        return Err(FrameError::MessageTooLarge(msg.len()));
    }
    let mut frame = Vec::with_capacity(4 + msg.len());
    frame.extend_from_slice(&len.to_be_bytes());
    frame.extend_from_slice(msg);
    Ok(frame)
}

/// Decode a frame from a buffer.
///
/// Returns `Ok(Some((total_consumed_bytes, payload)))` if a complete frame is
/// available, `Ok(None)` if the buffer is incomplete, or `Err` if the frame
/// exceeds the size limit.
pub fn decode_frame(buf: &[u8]) -> Result<Option<(usize, Vec<u8>)>, FrameError> {
    if buf.len() < 4 {
        return Ok(None);
    }
    let len = u32::from_be_bytes([buf[0], buf[1], buf[2], buf[3]]) as usize;
    if len > MAX_FRAME_SIZE {
        return Err(FrameError::MessageTooLarge(len));
    }
    if buf.len() < 4 + len {
        return Ok(None);
    }
    Ok(Some((4 + len, buf[4..4 + len].to_vec())))
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
        let frame = encode_frame(payload).expect("encode");
        let (consumed, decoded) = decode_frame(&frame).expect("no error").expect("should decode");
        assert_eq!(consumed, frame.len());
        assert_eq!(decoded, payload);
    }

    #[test]
    fn frame_decode_incomplete_header() {
        assert!(decode_frame(&[0x00, 0x00]).unwrap().is_none());
    }

    #[test]
    fn frame_decode_incomplete_payload() {
        let frame = encode_frame(b"hello").expect("encode");
        // Chop off the last byte so payload is incomplete.
        assert!(decode_frame(&frame[..frame.len() - 1]).unwrap().is_none());
    }

    #[test]
    fn frame_rejects_oversized() {
        // Craft a header claiming a payload larger than MAX_FRAME_SIZE.
        let huge_len = (MAX_FRAME_SIZE + 1) as u32;
        let mut buf = huge_len.to_be_bytes().to_vec();
        buf.push(0); // at least one byte so header is complete
        assert!(decode_frame(&buf).is_err());
    }

    #[test]
    fn jsonrpc_request_serde() {
        let req = JsonRpcRequest::new(JsonRpcId::Number(1), method::PANE_LIST, None);
        let json = serde_json::to_string(&req).unwrap();
        let parsed: JsonRpcRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.jsonrpc, "2.0");
        assert_eq!(parsed.id, Some(JsonRpcId::Number(1)));
        assert_eq!(parsed.method, "crux:pane/list");
        assert!(parsed.params.is_none());
    }

    #[test]
    fn jsonrpc_request_string_id() {
        let req = JsonRpcRequest::new(
            JsonRpcId::String("abc-123".into()),
            method::PANE_LIST,
            None,
        );
        let json = serde_json::to_string(&req).unwrap();
        let parsed: JsonRpcRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.id, Some(JsonRpcId::String("abc-123".into())));
    }

    #[test]
    fn jsonrpc_notification_has_no_id() {
        let req = JsonRpcRequest::notification(method::PANE_LIST, None);
        let json = serde_json::to_string(&req).unwrap();
        assert!(!json.contains("\"id\""));
        let parsed: JsonRpcRequest = serde_json::from_str(&json).unwrap();
        assert!(parsed.id.is_none());
    }

    #[test]
    fn jsonrpc_response_success_serde() {
        let resp = JsonRpcResponse::success(JsonRpcId::Number(42), serde_json::json!({"ok": true}));
        let json = serde_json::to_string(&resp).unwrap();
        // "error" field should be omitted
        assert!(!json.contains("\"error\""));
        let parsed: JsonRpcResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.id, JsonRpcId::Number(42));
        assert!(parsed.result.is_some());
        assert!(parsed.error.is_none());
    }

    #[test]
    fn jsonrpc_response_error_serde() {
        let resp = JsonRpcResponse::error(
            JsonRpcId::Number(7),
            error_code::PANE_NOT_FOUND,
            "pane 99 not found",
        );
        let json = serde_json::to_string(&resp).unwrap();
        // "result" field should be omitted
        assert!(!json.contains("\"result\""));
        let parsed: JsonRpcResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.error.as_ref().unwrap().code, -1001);
    }

    #[test]
    fn jsonrpc_response_null_id() {
        let resp = JsonRpcResponse::error(
            JsonRpcId::Null,
            error_code::PARSE_ERROR,
            "parse error",
        );
        let json = serde_json::to_string(&resp).unwrap();
        let parsed: JsonRpcResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.id, JsonRpcId::Null);
    }

    #[test]
    fn jsonrpc_id_display() {
        assert_eq!(JsonRpcId::Number(42).to_string(), "42");
        assert_eq!(JsonRpcId::String("abc".into()).to_string(), "abc");
        assert_eq!(JsonRpcId::Null.to_string(), "null");
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
