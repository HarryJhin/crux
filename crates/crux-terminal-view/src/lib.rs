//! GPUI rendering for the terminal: View, Element (canvas-based), cursor, color mapping.

mod colors;
mod element;
pub mod ime_switch;
mod input;
mod mouse;
mod view;

pub use crux_terminal::ensure_terminfo_installed;
pub use view::CruxTerminalView;
