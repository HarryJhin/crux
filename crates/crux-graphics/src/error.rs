//! Error types for the graphics protocol subsystem.

use crate::types::ImageId;

/// Errors that can occur during graphics protocol operations.
#[derive(Debug, thiserror::Error)]
pub enum GraphicsError {
    /// The image ID was not found in the store.
    #[error("image not found: {0:?}")]
    ImageNotFound(ImageId),

    /// The placement ID was not found for the given image.
    #[error("placement not found: image {image_id:?}, placement {placement_id}")]
    PlacementNotFound {
        image_id: ImageId,
        placement_id: u32,
    },

    /// The image data exceeds the maximum allowed size.
    #[error("image too large: {size} bytes (max {max} bytes)")]
    ImageTooLarge { size: usize, max: usize },

    /// The total memory quota has been exceeded.
    #[error("memory quota exceeded: {used} / {quota} bytes")]
    QuotaExceeded { used: usize, quota: usize },

    /// Invalid image dimensions.
    #[error("invalid dimensions: {width}x{height}")]
    InvalidDimensions { width: u32, height: u32 },

    /// Base64 decoding failed.
    #[error("base64 decode error: {0}")]
    Base64Decode(#[from] base64::DecodeError),

    /// The pixel format is invalid or unsupported.
    #[error("unsupported pixel format: {0}")]
    UnsupportedFormat(String),

    /// An error occurred while parsing a graphics protocol command.
    #[error("parse error: {0}")]
    ParseError(String),

    /// File I/O error (for file-based transmission).
    #[error("file error: {0}")]
    FileError(#[from] std::io::Error),

    /// Chunked transfer is incomplete or corrupted.
    #[error("incomplete chunked transfer for image {0:?}")]
    IncompleteTransfer(ImageId),
}
