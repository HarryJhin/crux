//! JSON-RPC and Crux-specific error code constants.

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
