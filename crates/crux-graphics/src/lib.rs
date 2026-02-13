//! Graphics protocol support for the Crux terminal emulator.
//!
//! This crate provides protocol-agnostic image management for terminal
//! graphics protocols (Kitty, iTerm2, Sixel). It handles:
//!
//! - **Protocol parsing**: Decoding escape sequences into structured commands
//! - **Image storage**: Memory-managed store with LRU eviction and quota enforcement
//! - **Placement tracking**: Mapping images to terminal grid positions with z-index layering
//!
//! # Architecture
//!
//! ```text
//! PTY byte stream
//!     │
//!     ▼
//! protocol::kitty::parse_kitty_command()  ← parse escape sequences
//!     │
//!     ▼
//! ImageManager::store_image()             ← store with quota enforcement
//! ImageManager::place_image()             ← track grid position
//!     │
//!     ▼
//! ImageManager::get_placements_in_range() ← query for rendering
//! ```
//!
//! # Pixel Format
//!
//! All image data is stored in **BGRA** format to match Metal's native
//! texture layout on macOS. RGB and RGBA data from protocols is converted
//! to BGRA before storage.
//!
//! # Memory Management
//!
//! The [`ImageManager`] enforces a configurable memory quota (default 320 MiB).
//! When the quota is exceeded, the least-recently-used images are evicted.
//! Individual images are capped at 64 MiB.

pub mod error;
pub mod manager;
pub mod protocol;
pub mod types;

// Re-export primary types for convenience.
pub use error::GraphicsError;
pub use manager::ImageManager;
pub use types::{ImageData, ImageId, ImagePlacement, PixelFormat, TransmissionMode};
