//! NSPasteboard clipboard and drag-and-drop support.

#![cfg(target_os = "macos")]

use anyhow::{anyhow, Result};
use std::path::PathBuf;

#[cfg(target_os = "macos")]
use objc2_app_kit::{
    NSPasteboard, NSPasteboardTypeFileURL, NSPasteboardTypePNG, NSPasteboardTypeString,
    NSPasteboardTypeTIFF,
};
#[cfg(target_os = "macos")]
use objc2_foundation::{NSArray, NSData, NSString};

/// Represents the different types of content that can be stored in the clipboard.
#[derive(Debug, Clone, PartialEq)]
pub enum ClipboardContent {
    /// Plain text content
    Text(String),
    /// HTML content
    Html(String),
    /// Image data in PNG format
    Image { png_data: Vec<u8> },
    /// List of file paths
    FilePaths(Vec<PathBuf>),
}

/// Main clipboard interface for reading and writing to NSPasteboard.
pub struct Clipboard;

impl Clipboard {
    /// Reads the current clipboard content and detects its type.
    ///
    /// Priority: FilePaths > Image > Html > Text
    pub fn read() -> Result<ClipboardContent> {
        unsafe {
            let pasteboard = NSPasteboard::generalPasteboard();
            let types = pasteboard
                .types()
                .ok_or_else(|| anyhow!("Failed to get pasteboard types"))?;

            // Check for file URLs
            if Self::contains_type(&types, NSPasteboardTypeFileURL) {
                if let Ok(paths) = Self::read_file_paths_internal(&pasteboard) {
                    return Ok(ClipboardContent::FilePaths(paths));
                }
            }

            // Check for images (PNG or TIFF)
            if Self::contains_type(&types, NSPasteboardTypePNG) {
                if let Ok(data) = Self::read_image_internal(&pasteboard) {
                    return Ok(ClipboardContent::Image { png_data: data });
                }
            }

            if Self::contains_type(&types, NSPasteboardTypeTIFF) {
                if let Ok(data) = Self::read_image_internal(&pasteboard) {
                    return Ok(ClipboardContent::Image { png_data: data });
                }
            }

            // Check for HTML (public.html UTI)
            let html_type = NSString::from_str("public.html");
            if Self::contains_type_str(&types, &html_type) {
                if let Some(data) = pasteboard.dataForType(&html_type) {
                    if let Ok(html) = String::from_utf8(data.bytes().to_vec()) {
                        return Ok(ClipboardContent::Html(html));
                    }
                }
            }

            // Default to plain text
            if Self::contains_type(&types, NSPasteboardTypeString) {
                if let Ok(text) = Self::read_text_internal(&pasteboard) {
                    return Ok(ClipboardContent::Text(text));
                }
            }

            Err(anyhow!("No supported clipboard content found"))
        }
    }

    /// Reads plain text from the clipboard.
    pub fn read_text() -> Result<String> {
        unsafe {
            let pasteboard = NSPasteboard::generalPasteboard();
            Self::read_text_internal(&pasteboard)
        }
    }

    /// Reads image data from the clipboard as PNG.
    ///
    /// If the clipboard contains TIFF data, it is returned as-is.
    /// The caller is responsible for converting TIFF to PNG if needed.
    pub fn read_image() -> Result<Vec<u8>> {
        unsafe {
            let pasteboard = NSPasteboard::generalPasteboard();
            Self::read_image_internal(&pasteboard)
        }
    }

    /// Writes plain text to the clipboard.
    pub fn write_text(text: &str) -> Result<()> {
        unsafe {
            let pasteboard = NSPasteboard::generalPasteboard();
            pasteboard.clearContents();

            let ns_string = NSString::from_str(text);
            let success = pasteboard.setString_forType(&ns_string, NSPasteboardTypeString);

            if success {
                Ok(())
            } else {
                Err(anyhow!("Failed to write text to pasteboard"))
            }
        }
    }

    /// Writes PNG image data to the clipboard.
    pub fn write_image(png_data: &[u8]) -> Result<()> {
        unsafe {
            let pasteboard = NSPasteboard::generalPasteboard();
            pasteboard.clearContents();

            let ns_data = NSData::with_bytes(png_data);
            let success = pasteboard.setData_forType(Some(&ns_data), NSPasteboardTypePNG);

            if success {
                Ok(())
            } else {
                Err(anyhow!("Failed to write image to pasteboard"))
            }
        }
    }

    /// Returns a list of available type identifiers in the clipboard.
    pub fn available_types() -> Vec<String> {
        unsafe {
            let pasteboard = NSPasteboard::generalPasteboard();
            if let Some(types) = pasteboard.types() {
                (0..types.count())
                    .map(|i| types.objectAtIndex(i).to_string())
                    .collect()
            } else {
                Vec::new()
            }
        }
    }

    // Internal helper methods

    unsafe fn read_text_internal(pasteboard: &NSPasteboard) -> Result<String> {
        pasteboard
            .stringForType(NSPasteboardTypeString)
            .map(|s| s.to_string())
            .ok_or_else(|| anyhow!("No text in pasteboard"))
    }

    unsafe fn read_image_internal(pasteboard: &NSPasteboard) -> Result<Vec<u8>> {
        // Try PNG first
        if let Some(data) = pasteboard.dataForType(NSPasteboardTypePNG) {
            return Ok(data.bytes().to_vec());
        }

        // Fall back to TIFF
        if let Some(data) = pasteboard.dataForType(NSPasteboardTypeTIFF) {
            return Ok(data.bytes().to_vec());
        }

        Err(anyhow!("No image data in pasteboard"))
    }

    unsafe fn read_file_paths_internal(_pasteboard: &NSPasteboard) -> Result<Vec<PathBuf>> {
        // Note: Reading file URLs from NSPasteboard requires using readObjectsForClasses_options
        // with proper type casting, which is complex with objc2 0.2.x API.
        // For Phase 3, we'll focus on text and image support first.
        // This can be implemented later with a more robust approach using NSFilePromiseReceiver
        // or by upgrading to objc2 0.5+ which has better typed array support.
        Err(anyhow!("File path reading not yet implemented"))
    }

    unsafe fn contains_type(types: &NSArray<NSString>, type_str: &'static NSString) -> bool {
        (0..types.count()).any(|i| types.objectAtIndex(i).isEqualToString(type_str))
    }

    unsafe fn contains_type_str(types: &NSArray<NSString>, type_str: &NSString) -> bool {
        (0..types.count()).any(|i| types.objectAtIndex(i).isEqualToString(type_str))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_clipboard_content_types() {
        // Test that ClipboardContent variants are created correctly
        let text = ClipboardContent::Text("Hello".to_string());
        assert!(matches!(text, ClipboardContent::Text(_)));

        let html = ClipboardContent::Html("<p>Test</p>".to_string());
        assert!(matches!(html, ClipboardContent::Html(_)));

        let image = ClipboardContent::Image {
            png_data: vec![0x89, 0x50, 0x4E, 0x47],
        };
        assert!(matches!(image, ClipboardContent::Image { .. }));

        let paths = ClipboardContent::FilePaths(vec![PathBuf::from("/tmp/test.txt")]);
        assert!(matches!(paths, ClipboardContent::FilePaths(_)));
    }

    #[test]
    fn test_clipboard_content_equality() {
        let text1 = ClipboardContent::Text("Hello".to_string());
        let text2 = ClipboardContent::Text("Hello".to_string());
        assert_eq!(text1, text2);

        let text3 = ClipboardContent::Text("World".to_string());
        assert_ne!(text1, text3);
    }

    // Note: Testing actual NSPasteboard operations requires a macOS runtime environment
    // and may interfere with the user's actual clipboard, so we only test the type system here.
}
