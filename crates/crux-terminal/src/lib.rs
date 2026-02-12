//! Terminal emulation core: VT parser, PTY management, terminal state.
//!
//! This crate wraps `alacritty_terminal` and `portable-pty` to provide
//! a self-contained terminal emulator entity that can be driven from
//! any UI framework (GPUI in our case).

pub mod event;
pub mod pty;
pub mod terminal;

// Re-export primary types at crate root for convenience.
pub use event::{CruxEventListener, TerminalEvent};
pub use pty::ensure_terminfo_installed;
pub use terminal::{
    CruxTerminal, CursorState, DamageState, IndexedCell, LineDamage, TerminalContent, TerminalSize,
};

// Re-export commonly needed alacritty types so downstream crates
// don't need to depend on alacritty_terminal directly.
pub use alacritty_terminal::grid::{Dimensions, Scroll};
pub use alacritty_terminal::index::{Column, Direction, Line, Point, Side};
pub use alacritty_terminal::selection::{Selection, SelectionRange, SelectionType};
pub use alacritty_terminal::term::cell::Flags as CellFlags;
pub use alacritty_terminal::term::TermMode;
pub use alacritty_terminal::vte::ansi::{Color, CursorShape, NamedColor};
