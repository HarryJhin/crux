//! GPUI rendering for the terminal: View, Element (canvas-based), cursor, color mapping.

mod clipboard_handler;
mod colors;
mod element;
pub mod ime_switch;
mod input;
mod keyboard;
#[allow(dead_code)] // TODO: Wire into keyboard.rs when Kitty keyboard response encoding is needed
mod kitty_encode;
mod mouse;
pub mod url_detector;
mod view;

pub use crux_terminal::ensure_terminfo_installed;
pub use view::CruxTerminalView;
