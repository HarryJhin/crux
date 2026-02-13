//! JSON-RPC 2.0 message types and protocol method params/results.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use crate::types::{
    JsonRpcId, PaneEvent, PaneEventType, PaneId, PaneInfo, PaneSize, SplitDirection, SplitSize,
    TabId, WindowId,
};

// ---------------------------------------------------------------------------
// JSON-RPC 2.0 types
// ---------------------------------------------------------------------------

/// Content type for clipboard operations.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ClipboardContentType {
    Text,
    Image,
    #[default]
    Auto,
}

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
    pub fn new(
        id: JsonRpcId,
        method: impl Into<String>,
        params: Option<serde_json::Value>,
    ) -> Self {
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

/// Parameters for `crux:clipboard/read`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipboardReadParams {
    /// Preferred content type: "text", "image", "auto" (default: "auto").
    #[serde(default)]
    pub content_type: ClipboardContentType,
}

/// Result of `crux:clipboard/read` — tagged union for type-safe responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "content_type")]
pub enum ClipboardReadResult {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "image")]
    Image { image_path: String },
    #[serde(rename = "html")]
    Html { html: String },
    #[serde(rename = "file_paths")]
    FilePaths { paths: Vec<String> },
}

/// Parameters for `crux:clipboard/write`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipboardWriteParams {
    /// Content type: "text" or "image".
    pub content_type: ClipboardContentType,
    /// Text content (when content_type is "text").
    pub text: Option<String>,
    /// Path to PNG file (when content_type is "image").
    pub image_path: Option<String>,
}

/// Result of `crux:ime/get-state`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImeStateResult {
    /// Whether IME is currently composing (has preedit text).
    pub composing: bool,
    /// Current preedit text, if any.
    pub preedit_text: Option<String>,
    /// Current input source identifier (e.g. "com.apple.inputmethod.Korean.2SetKorean").
    pub input_source: Option<String>,
}

/// Parameters for `crux:ime/set-input-source`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImeSetInputSourceParams {
    /// Input source identifier (e.g. "com.apple.keylayout.ABC").
    pub input_source: String,
}

/// Parameters for `crux:events/subscribe`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventsSubscribeParams {
    /// Event types to subscribe to.
    pub events: Vec<PaneEventType>,
}

/// Result of `crux:events/poll` — returns buffered events since last poll.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventsPollResult {
    pub events: Vec<PaneEvent>,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error_code;
    use crate::method;

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
        let req =
            JsonRpcRequest::new(JsonRpcId::String("abc-123".into()), method::PANE_LIST, None);
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
        let resp =
            JsonRpcResponse::success(JsonRpcId::Number(42), serde_json::json!({"ok": true}));
        let json = serde_json::to_string(&resp).unwrap();
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
        assert!(!json.contains("\"result\""));
        let parsed: JsonRpcResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.error.as_ref().unwrap().code, -1001);
    }

    #[test]
    fn jsonrpc_response_null_id() {
        let resp =
            JsonRpcResponse::error(JsonRpcId::Null, error_code::PARSE_ERROR, "parse error");
        let json = serde_json::to_string(&resp).unwrap();
        let parsed: JsonRpcResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.id, JsonRpcId::Null);
    }

    #[test]
    fn clipboard_read_result_tagged_union_serde() {
        let text = ClipboardReadResult::Text {
            text: "hello".into(),
        };
        let json = serde_json::to_string(&text).unwrap();
        assert!(json.contains(r#""content_type":"text""#));
        assert!(json.contains(r#""text":"hello""#));

        let image = ClipboardReadResult::Image {
            image_path: "/tmp/img.png".into(),
        };
        let json = serde_json::to_string(&image).unwrap();
        assert!(json.contains(r#""content_type":"image""#));

        let files = ClipboardReadResult::FilePaths {
            paths: vec!["a.txt".into()],
        };
        let json = serde_json::to_string(&files).unwrap();
        assert!(json.contains(r#""content_type":"file_paths""#));
    }

    #[test]
    fn clipboard_read_params_default() {
        let params: ClipboardReadParams = serde_json::from_str("{}").unwrap();
        assert_eq!(params.content_type, ClipboardContentType::Auto);
    }
}
