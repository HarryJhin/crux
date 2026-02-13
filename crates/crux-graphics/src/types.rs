//! Core types for the graphics protocol subsystem.
//!
//! All image data is stored in BGRA format to match Metal's preferred
//! pixel layout, avoiding costly format conversions at render time.

use serde::{Deserialize, Serialize};

/// Unique identifier for a stored image.
///
/// Image IDs are assigned by the terminal application (client) via the
/// graphics protocol. ID 0 is reserved and means "no explicit ID".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ImageId(pub u32);

/// Pixel format for image data.
///
/// BGRA is the native format for Metal textures on macOS.
/// RGB and RGBA data received from protocols is converted to BGRA
/// before storage.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PixelFormat {
    /// 3 bytes per pixel: Red, Green, Blue.
    Rgb,
    /// 4 bytes per pixel: Red, Green, Blue, Alpha.
    Rgba,
    /// 4 bytes per pixel: Blue, Green, Red, Alpha (Metal native).
    Bgra,
    /// Compressed PNG data (needs decoding before use).
    Png,
}

/// How image data is transmitted from the application to the terminal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TransmissionMode {
    /// Data is sent directly inline, base64-encoded in the escape sequence.
    Direct,
    /// Data is read from a regular file path.
    File,
    /// Data is read from a temporary file (deleted after reading).
    TempFile,
    /// Data is transferred via shared memory (POSIX shm_open).
    SharedMemory,
}

/// Raw image data with dimensions and format metadata.
#[derive(Debug, Clone)]
pub struct ImageData {
    /// Pixel data in the specified format.
    pub data: Vec<u8>,
    /// Width in pixels.
    pub width: u32,
    /// Height in pixels.
    pub height: u32,
    /// Pixel format of the data.
    pub format: PixelFormat,
}

impl ImageData {
    /// Create new image data.
    pub fn new(data: Vec<u8>, width: u32, height: u32, format: PixelFormat) -> Self {
        Self {
            data,
            width,
            height,
            format,
        }
    }

    /// Convert the image data to BGRA format in-place.
    ///
    /// This is a no-op if the data is already in BGRA format.
    /// PNG data must be decoded before calling this method.
    pub fn to_bgra(&mut self) {
        match self.format {
            PixelFormat::Bgra => {}
            PixelFormat::Rgba => {
                // Swap R and B channels: RGBA -> BGRA
                for chunk in self.data.chunks_exact_mut(4) {
                    chunk.swap(0, 2);
                }
                self.format = PixelFormat::Bgra;
            }
            PixelFormat::Rgb => {
                // Expand RGB to BGRA with alpha=255
                let mut bgra = Vec::with_capacity(self.data.len() / 3 * 4);
                for chunk in self.data.chunks_exact(3) {
                    bgra.push(chunk[2]); // B
                    bgra.push(chunk[1]); // G
                    bgra.push(chunk[0]); // R
                    bgra.push(255); // A
                }
                self.data = bgra;
                self.format = PixelFormat::Bgra;
            }
            PixelFormat::Png => {
                log::warn!("to_bgra() called on PNG data; decode first");
            }
        }
    }

    /// Returns the size of the pixel data in bytes.
    pub fn byte_size(&self) -> usize {
        self.data.len()
    }
}

/// Describes where and how an image is placed in the terminal grid.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImagePlacement {
    /// The image this placement refers to.
    pub image_id: ImageId,
    /// Unique placement ID (0 = default placement).
    pub placement_id: u32,
    /// Column position in the terminal grid (0-indexed).
    pub column: u32,
    /// Row position in the terminal grid (0-indexed, relative to scrollback).
    pub row: i32,
    /// Display width in columns (0 = auto from image dimensions).
    pub columns: u32,
    /// Display height in rows (0 = auto from image dimensions).
    pub rows: u32,
    /// X offset within the source image in pixels.
    pub source_x: u32,
    /// Y offset within the source image in pixels.
    pub source_y: u32,
    /// Width of the source region in pixels (0 = full width).
    pub source_width: u32,
    /// Height of the source region in pixels (0 = full height).
    pub source_height: u32,
    /// Z-index for layering: negative = under text, positive = over text.
    pub z_index: i32,
}

impl ImagePlacement {
    /// Create a new placement with default values.
    pub fn new(image_id: ImageId) -> Self {
        Self {
            image_id,
            placement_id: 0,
            column: 0,
            row: 0,
            columns: 0,
            rows: 0,
            source_x: 0,
            source_y: 0,
            source_width: 0,
            source_height: 0,
            z_index: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rgb_to_bgra_conversion() {
        let mut img = ImageData::new(
            vec![255, 0, 0, 0, 255, 0, 0, 0, 255],
            3,
            1,
            PixelFormat::Rgb,
        );
        img.to_bgra();
        assert_eq!(img.format, PixelFormat::Bgra);
        // Red pixel -> BGRA(0, 0, 255, 255)
        assert_eq!(&img.data[0..4], &[0, 0, 255, 255]);
        // Green pixel -> BGRA(0, 255, 0, 255)
        assert_eq!(&img.data[4..8], &[0, 255, 0, 255]);
        // Blue pixel -> BGRA(255, 0, 0, 255)
        assert_eq!(&img.data[8..12], &[255, 0, 0, 255]);
    }

    #[test]
    fn test_rgba_to_bgra_conversion() {
        let mut img = ImageData::new(
            vec![255, 0, 0, 128, 0, 255, 0, 64],
            2,
            1,
            PixelFormat::Rgba,
        );
        img.to_bgra();
        assert_eq!(img.format, PixelFormat::Bgra);
        // Red pixel with alpha 128 -> BGRA(0, 0, 255, 128)
        assert_eq!(&img.data[0..4], &[0, 0, 255, 128]);
        // Green pixel with alpha 64 -> BGRA(0, 255, 0, 64)
        assert_eq!(&img.data[4..8], &[0, 255, 0, 64]);
    }

    #[test]
    fn test_bgra_to_bgra_is_noop() {
        let original = vec![10, 20, 30, 40];
        let mut img = ImageData::new(original.clone(), 1, 1, PixelFormat::Bgra);
        img.to_bgra();
        assert_eq!(img.data, original);
    }

    #[test]
    fn test_image_placement_defaults() {
        let p = ImagePlacement::new(ImageId(42));
        assert_eq!(p.image_id, ImageId(42));
        assert_eq!(p.placement_id, 0);
        assert_eq!(p.z_index, 0);
        assert_eq!(p.columns, 0);
        assert_eq!(p.rows, 0);
    }

    #[test]
    fn test_image_data_byte_size() {
        let img = ImageData::new(vec![0; 100], 5, 5, PixelFormat::Rgba);
        assert_eq!(img.byte_size(), 100);
    }
}
