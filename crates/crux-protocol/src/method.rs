//! Protocol method name constants.

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
pub const CLIPBOARD_READ: &str = "crux:clipboard/read";
pub const CLIPBOARD_WRITE: &str = "crux:clipboard/write";
pub const IME_GET_STATE: &str = "crux:ime/get-state";
pub const IME_SET_INPUT_SOURCE: &str = "crux:ime/set-input-source";
pub const EVENTS_SUBSCRIBE: &str = "crux:events/subscribe";
pub const EVENTS_POLL: &str = "crux:events/poll";
