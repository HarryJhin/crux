//! CLI command definitions using clap derive macros.

use clap::{Parser, Subcommand};

/// Crux terminal emulator
#[derive(Parser)]
#[command(name = "crux-app", version, about)]
pub struct CliArgs {
    #[command(subcommand)]
    pub command: Option<CliCommand>,
}

#[derive(Subcommand)]
pub enum CliCommand {
    /// Control a running Crux instance
    Cli {
        #[command(subcommand)]
        action: CliAction,
    },
}

#[derive(Subcommand)]
pub enum CliAction {
    /// Split the current pane
    SplitPane {
        /// Split direction
        #[arg(long, default_value = "right")]
        direction: String,

        /// Size as percentage
        #[arg(long)]
        percent: Option<u8>,

        /// Target pane ID (default: active pane)
        #[arg(long)]
        pane_id: Option<u64>,

        /// Working directory for new pane
        #[arg(long)]
        cwd: Option<String>,

        /// Command to run in new pane
        #[arg(last = true)]
        command: Vec<String>,
    },

    /// Send text to a pane
    SendText {
        /// Target pane ID (default: $CRUX_PANE or active pane)
        #[arg(long)]
        pane_id: Option<u64>,

        /// Send without bracketed paste wrapping
        #[arg(long)]
        no_paste: bool,

        /// Text to send (reads from stdin if not provided)
        text: Option<String>,
    },

    /// Get text content from a pane
    GetText {
        /// Target pane ID
        #[arg(long)]
        pane_id: Option<u64>,

        /// Start line (0=screen top, negative=scrollback)
        #[arg(long)]
        start_line: Option<i32>,

        /// End line
        #[arg(long)]
        end_line: Option<i32>,

        /// Include ANSI escape sequences
        #[arg(long)]
        escapes: bool,
    },

    /// List all panes
    List {
        /// Output format
        #[arg(long, default_value = "table")]
        format: String,
    },

    /// Activate (focus) a pane
    ActivatePane {
        /// Pane ID to activate
        #[arg(long)]
        pane_id: u64,
    },

    /// Close a pane
    ClosePane {
        /// Pane ID to close
        #[arg(long)]
        pane_id: u64,

        /// Force close without confirmation
        #[arg(long)]
        force: bool,
    },

    /// Create a new window (single-window mode: returns existing window)
    WindowCreate {
        /// Window title
        #[arg(long)]
        title: Option<String>,

        /// Window width in pixels
        #[arg(long)]
        width: Option<u32>,

        /// Window height in pixels
        #[arg(long)]
        height: Option<u32>,
    },

    /// List all windows
    WindowList {
        /// Output format: "table" (default) or "json"
        #[arg(long, default_value = "table")]
        format: String,
    },
}
