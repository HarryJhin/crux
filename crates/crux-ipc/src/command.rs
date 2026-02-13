//! IPC command types for the GPUI bridge.
//!
//! Each variant carries a [`tokio::sync::oneshot::Sender`] so the IPC handler
//! can await the GPUI main thread's response.

use tokio::sync::oneshot;

use crux_protocol::{
    ActivatePaneParams, ClipboardReadParams, ClipboardReadResult, ClipboardWriteParams,
    ClosePaneParams, GetSelectionParams, GetSelectionResult, GetSnapshotParams, GetSnapshotResult,
    GetTextParams, GetTextResult, HandshakeParams, HandshakeResult, ImeSetInputSourceParams,
    ImeStateResult, ListPanesResult, ResizePaneParams, SendTextParams, SendTextResult,
    SessionLoadParams, SessionLoadResult, SessionSaveParams, SessionSaveResult, SplitPaneParams,
    SplitPaneResult, WindowCreateParams, WindowCreateResult, WindowListResult,
};

/// Commands sent from the IPC server to the GPUI main thread.
pub enum IpcCommand {
    Handshake {
        params: HandshakeParams,
        reply: oneshot::Sender<anyhow::Result<HandshakeResult>>,
    },
    SplitPane {
        params: SplitPaneParams,
        reply: oneshot::Sender<anyhow::Result<SplitPaneResult>>,
    },
    SendText {
        params: SendTextParams,
        reply: oneshot::Sender<anyhow::Result<SendTextResult>>,
    },
    GetText {
        params: GetTextParams,
        reply: oneshot::Sender<anyhow::Result<GetTextResult>>,
    },
    GetSelection {
        params: GetSelectionParams,
        reply: oneshot::Sender<anyhow::Result<GetSelectionResult>>,
    },
    GetSnapshot {
        params: GetSnapshotParams,
        reply: oneshot::Sender<anyhow::Result<GetSnapshotResult>>,
    },
    ListPanes {
        reply: oneshot::Sender<anyhow::Result<ListPanesResult>>,
    },
    ResizePane {
        params: ResizePaneParams,
        reply: oneshot::Sender<anyhow::Result<()>>,
    },
    ActivatePane {
        params: ActivatePaneParams,
        reply: oneshot::Sender<anyhow::Result<()>>,
    },
    ClosePane {
        params: ClosePaneParams,
        reply: oneshot::Sender<anyhow::Result<()>>,
    },
    WindowCreate {
        params: WindowCreateParams,
        reply: oneshot::Sender<anyhow::Result<WindowCreateResult>>,
    },
    WindowList {
        reply: oneshot::Sender<anyhow::Result<WindowListResult>>,
    },
    SessionSave {
        params: SessionSaveParams,
        reply: oneshot::Sender<anyhow::Result<SessionSaveResult>>,
    },
    SessionLoad {
        params: SessionLoadParams,
        reply: oneshot::Sender<anyhow::Result<SessionLoadResult>>,
    },
    ClipboardRead {
        params: ClipboardReadParams,
        reply: oneshot::Sender<anyhow::Result<ClipboardReadResult>>,
    },
    ClipboardWrite {
        params: ClipboardWriteParams,
        reply: oneshot::Sender<anyhow::Result<()>>,
    },
    ImeGetState {
        reply: oneshot::Sender<anyhow::Result<ImeStateResult>>,
    },
    ImeSetInputSource {
        params: ImeSetInputSourceParams,
        reply: oneshot::Sender<anyhow::Result<()>>,
    },
}
