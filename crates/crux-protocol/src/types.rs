//! Core domain types: identifiers, enums, pane info, events, and policies.

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
// Event types
// ---------------------------------------------------------------------------

/// Event types available for subscription.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PaneEventType {
    PaneCreated,
    PaneClosed,
    PaneFocused,
    PaneResized,
    TitleChanged,
    ClipboardSet,
}

// ---------------------------------------------------------------------------
// OSC 52 clipboard access policy
// ---------------------------------------------------------------------------

/// OSC 52 clipboard access policy.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Osc52Policy {
    /// Default: programs can write to clipboard, not read.
    #[default]
    WriteOnly,
    /// User opted in: programs can read and write clipboard.
    ReadWrite,
    /// No clipboard access via OSC 52.
    Disabled,
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

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

    #[test]
    fn pane_event_type_serde() {
        let evt = PaneEventType::TitleChanged;
        let json = serde_json::to_string(&evt).unwrap();
        assert_eq!(json, r#""title_changed""#);
        let parsed: PaneEventType = serde_json::from_str(&json).unwrap();
        assert!(matches!(parsed, PaneEventType::TitleChanged));
    }

    #[test]
    fn osc52_policy_default() {
        let policy = Osc52Policy::default();
        assert!(matches!(policy, Osc52Policy::WriteOnly));
    }
}
