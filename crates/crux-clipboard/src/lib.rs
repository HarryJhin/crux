//! NSPasteboard clipboard and drag-and-drop support.

#![cfg(target_os = "macos")]

use std::path::PathBuf;

use objc2_app_kit::{
    NSPasteboard, NSPasteboardTypeFileURL, NSPasteboardTypePNG, NSPasteboardTypeString,
    NSPasteboardTypeTIFF,
};
use objc2_foundation::{MainThreadMarker, NSArray, NSData, NSString};

/// Errors that can occur during clipboard operations.
#[derive(Debug, thiserror::Error)]
pub enum ClipboardError {
    #[error("no pasteboard types available")]
    NoPasteboardTypes,
    #[error("no supported clipboard content found")]
    NoSupportedContent,
    #[error("no text in pasteboard")]
    NoText,
    #[error("no image data in pasteboard")]
    NoImage,
    #[error("failed to write to pasteboard")]
    WriteFailed,
    #[error("file path reading not yet implemented")]
    NotImplemented,
}

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
    pub fn read(_mtm: MainThreadMarker) -> Result<ClipboardContent, ClipboardError> {
        // SAFETY: generalPasteboard() returns a process-lifetime singleton; called from main thread.
        let pasteboard = unsafe { NSPasteboard::generalPasteboard() };

        // SAFETY: types() returns an autoreleased optional array of pasteboard type strings.
        let types = unsafe { pasteboard.types() }.ok_or(ClipboardError::NoPasteboardTypes)?;

        // Check for file URLs
        // SAFETY: NSPasteboardTypeFileURL is a valid static pasteboard type constant.
        if Self::contains_type(&types, unsafe { NSPasteboardTypeFileURL }) {
            if let Ok(paths) = Self::read_file_paths_internal(&pasteboard) {
                return Ok(ClipboardContent::FilePaths(paths));
            }
        }

        // Check for images (PNG or TIFF)
        // SAFETY: NSPasteboardTypePNG is a valid static pasteboard type constant.
        if Self::contains_type(&types, unsafe { NSPasteboardTypePNG }) {
            if let Ok(data) = Self::read_image_internal(&pasteboard) {
                return Ok(ClipboardContent::Image { png_data: data });
            }
        }

        // SAFETY: NSPasteboardTypeTIFF is a valid static pasteboard type constant.
        if Self::contains_type(&types, unsafe { NSPasteboardTypeTIFF }) {
            if let Ok(data) = Self::read_image_internal(&pasteboard) {
                return Ok(ClipboardContent::Image { png_data: data });
            }
        }

        // Check for HTML (public.html UTI)
        let html_type = NSString::from_str("public.html");
        if Self::contains_type(&types, &html_type) {
            // SAFETY: pasteboard is valid, html_type is a valid NSString key.
            let data = unsafe { pasteboard.dataForType(&html_type) };
            if let Some(data) = data {
                if let Ok(html) = String::from_utf8(data.bytes().to_vec()) {
                    return Ok(ClipboardContent::Html(html));
                }
            }
        }

        // Default to plain text
        // SAFETY: NSPasteboardTypeString is a valid static pasteboard type constant.
        if Self::contains_type(&types, unsafe { NSPasteboardTypeString }) {
            if let Ok(text) = Self::read_text_internal(&pasteboard) {
                return Ok(ClipboardContent::Text(text));
            }
        }

        Err(ClipboardError::NoSupportedContent)
    }

    /// Reads plain text from the clipboard.
    pub fn read_text(_mtm: MainThreadMarker) -> Result<String, ClipboardError> {
        // SAFETY: generalPasteboard() returns a process-lifetime singleton; called from main thread.
        let pasteboard = unsafe { NSPasteboard::generalPasteboard() };
        Self::read_text_internal(&pasteboard)
    }

    /// Reads image data from the clipboard as PNG.
    ///
    /// If the clipboard contains TIFF data, it is returned as-is.
    /// The caller is responsible for converting TIFF to PNG if needed.
    pub fn read_image(_mtm: MainThreadMarker) -> Result<Vec<u8>, ClipboardError> {
        // SAFETY: generalPasteboard() returns a process-lifetime singleton; called from main thread.
        let pasteboard = unsafe { NSPasteboard::generalPasteboard() };
        Self::read_image_internal(&pasteboard)
    }

    /// Writes plain text to the clipboard.
    pub fn write_text(text: &str, _mtm: MainThreadMarker) -> Result<(), ClipboardError> {
        // SAFETY: generalPasteboard() returns a process-lifetime singleton; called from main thread.
        let pasteboard = unsafe { NSPasteboard::generalPasteboard() };

        // SAFETY: clearContents() resets the pasteboard; valid on a live pasteboard instance.
        unsafe { pasteboard.clearContents() };

        let ns_string = NSString::from_str(text);

        // SAFETY: setString_forType writes a valid NSString with a valid type key.
        let success =
            unsafe { pasteboard.setString_forType(&ns_string, NSPasteboardTypeString) };

        if success {
            Ok(())
        } else {
            Err(ClipboardError::WriteFailed)
        }
    }

    /// Writes PNG image data to the clipboard.
    pub fn write_image(png_data: &[u8], _mtm: MainThreadMarker) -> Result<(), ClipboardError> {
        // SAFETY: generalPasteboard() returns a process-lifetime singleton; called from main thread.
        let pasteboard = unsafe { NSPasteboard::generalPasteboard() };

        // SAFETY: clearContents() resets the pasteboard; valid on a live pasteboard instance.
        unsafe { pasteboard.clearContents() };

        let ns_data = NSData::with_bytes(png_data);

        // SAFETY: setData_forType writes valid NSData with a valid type key.
        let success =
            unsafe { pasteboard.setData_forType(Some(&ns_data), NSPasteboardTypePNG) };

        if success {
            Ok(())
        } else {
            Err(ClipboardError::WriteFailed)
        }
    }

    /// Returns a list of available type identifiers in the clipboard.
    pub fn available_types(_mtm: MainThreadMarker) -> Vec<String> {
        // SAFETY: generalPasteboard() returns a process-lifetime singleton; called from main thread.
        let pasteboard = unsafe { NSPasteboard::generalPasteboard() };

        // SAFETY: types() returns an autoreleased optional array of pasteboard type strings.
        let types = unsafe { pasteboard.types() };

        match types {
            Some(types) => (0..types.count())
                .map(|i| {
                    // SAFETY: index is in bounds (0..count), returns autoreleased NSString.
                    unsafe { types.objectAtIndex(i) }.to_string()
                })
                .collect(),
            None => Vec::new(),
        }
    }

    // -- Internal helper methods --

    fn read_text_internal(pasteboard: &NSPasteboard) -> Result<String, ClipboardError> {
        // SAFETY: stringForType with a valid type key returns an autoreleased optional NSString.
        unsafe { pasteboard.stringForType(NSPasteboardTypeString) }
            .map(|s| s.to_string())
            .ok_or(ClipboardError::NoText)
    }

    fn read_image_internal(pasteboard: &NSPasteboard) -> Result<Vec<u8>, ClipboardError> {
        // Try PNG first
        // SAFETY: dataForType with a valid type key returns an autoreleased optional NSData.
        if let Some(data) = unsafe { pasteboard.dataForType(NSPasteboardTypePNG) } {
            return Ok(data.bytes().to_vec());
        }

        // Fall back to TIFF
        // SAFETY: dataForType with a valid type key returns an autoreleased optional NSData.
        if let Some(data) = unsafe { pasteboard.dataForType(NSPasteboardTypeTIFF) } {
            return Ok(data.bytes().to_vec());
        }

        Err(ClipboardError::NoImage)
    }

    fn read_file_paths_internal(
        _pasteboard: &NSPasteboard,
    ) -> Result<Vec<PathBuf>, ClipboardError> {
        // Note: Reading file URLs from NSPasteboard requires using readObjectsForClasses_options
        // with proper type casting, which is complex with objc2 0.2.x API.
        // For Phase 3, we'll focus on text and image support first.
        // This can be implemented later with a more robust approach using NSFilePromiseReceiver
        // or by upgrading to objc2 0.5+ which has better typed array support.
        Err(ClipboardError::NotImplemented)
    }

    fn contains_type(types: &NSArray<NSString>, type_str: &NSString) -> bool {
        (0..types.count()).any(|i| {
            // SAFETY: objectAtIndex with valid index returns autoreleased NSString;
            // isEqualToString compares two valid NSString instances.
            unsafe { types.objectAtIndex(i).isEqualToString(type_str) }
        })
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

    #[test]
    fn test_clipboard_error_display() {
        assert_eq!(
            ClipboardError::NoPasteboardTypes.to_string(),
            "no pasteboard types available"
        );
        assert_eq!(
            ClipboardError::NoSupportedContent.to_string(),
            "no supported clipboard content found"
        );
        assert_eq!(ClipboardError::NoText.to_string(), "no text in pasteboard");
        assert_eq!(
            ClipboardError::NoImage.to_string(),
            "no image data in pasteboard"
        );
        assert_eq!(
            ClipboardError::WriteFailed.to_string(),
            "failed to write to pasteboard"
        );
        assert_eq!(
            ClipboardError::NotImplemented.to_string(),
            "file path reading not yet implemented"
        );
    }

    // Note: Testing actual NSPasteboard operations requires a macOS runtime environment
    // and may interfere with the user's actual clipboard, so we only test the type system here.
}
