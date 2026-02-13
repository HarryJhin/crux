//! Graphics protocol parsers.
//!
//! Each sub-module implements parsing for a specific terminal graphics protocol:
//!
//! - [`kitty`] — Kitty graphics protocol (APC-based, most capable)
//!
//! Future additions:
//! - iTerm2 (OSC 1337)
//! - Sixel (DCS-based, legacy)

pub mod kitty;
