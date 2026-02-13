//! Clipboard provider trait and platform implementations.
//!
//! The [`ClipboardProvider`] trait defines a platform-independent clipboard API.
//! The macOS implementation uses NSPasteboard via objc2 bindings.

use std::path::PathBuf;

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

/// Platform-independent clipboard access.
///
/// Implementations provide read/write access to the system clipboard.
/// The trait uses `&self` instance methods so that implementations can
/// store platform-specific handles (e.g., `MainThreadMarker` on macOS).
pub trait ClipboardProvider: Send + Sync {
    /// Read the current clipboard content, auto-detecting the type.
    ///
    /// Priority: FilePaths > Image > Html > Text
    fn read_clipboard(&self) -> Result<ClipboardContent, ClipboardError>;

    /// Read plain text from the clipboard.
    fn read_text(&self) -> Result<String, ClipboardError>;

    /// Read image data from the clipboard as PNG bytes.
    fn read_image(&self) -> Result<Vec<u8>, ClipboardError>;

    /// Write plain text to the clipboard.
    fn write_text(&self, text: &str) -> Result<(), ClipboardError>;

    /// Write PNG image data to the clipboard.
    fn write_image(&self, png_data: &[u8]) -> Result<(), ClipboardError>;

    /// Return a list of available type identifiers in the clipboard.
    fn available_types(&self) -> Vec<String>;
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

// -- macOS implementation -------------------------------------------------

#[cfg(target_os = "macos")]
mod macos;

#[cfg(target_os = "macos")]
pub use macos::Clipboard;

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
