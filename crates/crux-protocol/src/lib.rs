//! Shared protocol types for IPC and terminal communication.
//!
//! This crate defines the JSON-RPC 2.0 message types, protocol method
//! parameters/results, and length-prefix framing used by Crux's IPC layer.

pub mod error_code;
pub mod framing;
pub mod method;
mod rpc;
mod types;

// Re-export everything at crate root to preserve the existing public API.

// types
pub use types::{
    JsonRpcId, Osc52Policy, PaneEvent, PaneEventType, PaneId, PaneInfo, PaneSize, SplitDirection,
    SplitSize, TabId, WindowId,
};

// rpc
pub use rpc::{
    ActivatePaneParams, ClipboardReadParams, ClipboardReadResult, ClipboardWriteParams,
    ClosePaneParams, EventsPollResult, EventsSubscribeParams, GetSelectionParams,
    GetSelectionResult, GetSnapshotParams, GetSnapshotResult, GetTextParams, GetTextResult,
    HandshakeParams, HandshakeResult, ImeSetInputSourceParams, ImeStateResult, JsonRpcError,
    JsonRpcRequest, JsonRpcResponse, ListPanesResult, ResizePaneParams, SendTextParams,
    SendTextResult, SessionLoadParams, SessionLoadResult, SessionSaveParams, SessionSaveResult,
    SplitPaneParams, SplitPaneResult, WindowCreateParams, WindowCreateResult, WindowInfo,
    WindowListResult,
};

// framing
pub use framing::{decode_frame, encode_frame, FrameError, MAX_FRAME_SIZE};
