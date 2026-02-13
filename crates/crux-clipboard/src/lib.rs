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
    #[error("failed to decode image: {0}")]
    ImageDecode(String),
    #[error("failed to encode image: {0}")]
    ImageEncode(String),
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

/// Convert TIFF image data to PNG format.
///
/// macOS screenshots and pasteboard images are often in TIFF format.
/// This converts them to PNG for broader compatibility.
fn tiff_to_png(tiff_data: &[u8]) -> Result<Vec<u8>, ClipboardError> {
    let img =
        image::load_from_memory_with_format(tiff_data, image::ImageFormat::Tiff).map_err(|e| {
            log::warn!("TIFF decode failed: {e}");
            ClipboardError::ImageDecode(e.to_string())
        })?;
    let mut png_buf = Vec::new();
    img.write_to(
        &mut std::io::Cursor::new(&mut png_buf),
        image::ImageFormat::Png,
    )
    .map_err(|e| {
        log::warn!("PNG encode failed: {e}");
        ClipboardError::ImageEncode(e.to_string())
    })?;
    Ok(png_buf)
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
        let success = unsafe { pasteboard.setString_forType(&ns_string, NSPasteboardTypeString) };

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
        let success = unsafe { pasteboard.setData_forType(Some(&ns_data), NSPasteboardTypePNG) };

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
        // Try PNG first — return as-is.
        if let Some(data) = unsafe { pasteboard.dataForType(NSPasteboardTypePNG) } {
            return Ok(data.bytes().to_vec());
        }

        // Fall back to TIFF and convert to PNG.
        if let Some(data) = unsafe { pasteboard.dataForType(NSPasteboardTypeTIFF) } {
            return tiff_to_png(data.bytes());
        }

        Err(ClipboardError::NoImage)
    }

    fn read_file_paths_internal(pasteboard: &NSPasteboard) -> Result<Vec<PathBuf>, ClipboardError> {
        // Read file URL string from pasteboard.
        let file_url_type = unsafe { NSPasteboardTypeFileURL };
        let data = unsafe { pasteboard.stringForType(file_url_type) };
        if let Some(url_string) = data {
            let url_str = url_string.to_string();
            // File URLs are percent-encoded: file:///path/to/file
            if let Ok(url) = url::Url::parse(&url_str) {
                if let Ok(path) = url.to_file_path() {
                    return Ok(vec![path]);
                }
            }
            // Fallback: treat as a plain path.
            return Ok(vec![PathBuf::from(url_str)]);
        }
        Err(ClipboardError::NoSupportedContent)
    }

    fn contains_type(types: &NSArray<NSString>, type_str: &NSString) -> bool {
        (0..types.count()).any(|i| {
            // SAFETY: objectAtIndex with valid index returns autoreleased NSString;
            // isEqualToString compares two valid NSString instances.
            unsafe { types.objectAtIndex(i).isEqualToString(type_str) }
        })
    }
}

/// Save clipboard image to a temp file and return the path.
///
/// Uses `$TMPDIR/crux-clipboard/` with 0700 permissions to prevent symlink attacks.
/// File names include PID + atomic counter + timestamp for collision prevention.
/// Files are created with `create_new` (O_EXCL) to prevent TOCTOU races.
pub fn save_image_to_temp(png_data: &[u8]) -> Result<std::path::PathBuf, ClipboardError> {
    use std::io::Write;
    use std::sync::atomic::{AtomicU64, Ordering};

    let dir = std::env::temp_dir().join("crux-clipboard");
    std::fs::create_dir_all(&dir).map_err(|e| {
        log::warn!("failed to create temp dir: {e}");
        ClipboardError::WriteFailed
    })?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700))
            .map_err(|_| ClipboardError::WriteFailed)?;
    }

    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let pid = std::process::id();
    let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let path = dir.join(format!("paste-{pid}-{timestamp}-{seq}.png"));

    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .map_err(|e| {
            log::warn!("failed to create temp file: {e}");
            ClipboardError::WriteFailed
        })?;
    file.write_all(png_data).map_err(|e| {
        log::warn!("failed to write temp file: {e}");
        ClipboardError::WriteFailed
    })?;

    Ok(path)
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

    #[test]
    fn test_save_image_to_temp() {
        // Minimal valid PNG: 8-byte signature + IHDR + IEND
        let png_data = vec![
            0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, // PNG signature
        ];
        let result = save_image_to_temp(&png_data);
        assert!(result.is_ok());
        let path = result.unwrap();
        assert!(path.exists());
        assert!(path.to_string_lossy().contains("crux-clipboard"));
        assert!(path.to_string_lossy().ends_with(".png"));
        // Verify content
        let content = std::fs::read(&path).unwrap();
        assert_eq!(content, png_data);
        // Clean up
        std::fs::remove_file(path).ok();
    }

    #[test]
    fn test_save_image_to_temp_unique_names() {
        let data1 = vec![1, 2, 3];
        let data2 = vec![4, 5, 6];
        let path1 = save_image_to_temp(&data1).unwrap();
        let path2 = save_image_to_temp(&data2).unwrap();
        assert_ne!(path1, path2);
        // Clean up
        std::fs::remove_file(path1).ok();
        std::fs::remove_file(path2).ok();
    }

    #[test]
    fn test_new_error_variants() {
        let err = ClipboardError::ImageDecode("bad format".to_string());
        assert_eq!(err.to_string(), "failed to decode image: bad format");
        let err = ClipboardError::ImageEncode("write error".to_string());
        assert_eq!(err.to_string(), "failed to encode image: write error");
    }

    // Note: Testing actual NSPasteboard operations requires a macOS runtime environment
    // and may interfere with the user's actual clipboard, so we only test the type system here.
}
