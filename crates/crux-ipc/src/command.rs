//! IPC command types for the GPUI bridge.
//!
//! Each variant carries a [`tokio::sync::oneshot::Sender`] so the IPC handler
//! can await the GPUI main thread's response.

use tokio::sync::oneshot;

use crux_protocol::{
    ActivatePaneParams, ClosePaneParams, GetTextParams, GetTextResult, HandshakeParams,
    HandshakeResult, ListPanesResult, SendTextParams, SendTextResult, SplitPaneParams,
    SplitPaneResult,
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
    ListPanes {
        reply: oneshot::Sender<anyhow::Result<ListPanesResult>>,
    },
    ActivatePane {
        params: ActivatePaneParams,
        reply: oneshot::Sender<anyhow::Result<()>>,
    },
    ClosePane {
        params: ClosePaneParams,
        reply: oneshot::Sender<anyhow::Result<()>>,
    },
}
