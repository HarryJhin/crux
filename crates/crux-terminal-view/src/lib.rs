//! GPUI rendering for the terminal: View, Element (canvas-based), cursor, color mapping.

mod colors;
mod element;
mod input;
mod mouse;
mod view;

pub use crux_terminal::ensure_terminfo_installed;
pub use view::CruxTerminalView;
