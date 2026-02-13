//! macOS clipboard implementation using NSPasteboard.

use std::path::PathBuf;

use objc2_app_kit::{
    NSPasteboard, NSPasteboardTypeFileURL, NSPasteboardTypePNG, NSPasteboardTypeString,
    NSPasteboardTypeTIFF,
};
use objc2_foundation::{MainThreadMarker, NSArray, NSData, NSString};

use crate::{ClipboardContent, ClipboardError, ClipboardProvider};

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

/// macOS clipboard backed by NSPasteboard.
///
/// Requires a `MainThreadMarker` at construction time to prove we are on
/// the main thread (NSPasteboard is not thread-safe).
///
/// # Safety
///
/// `MainThreadMarker` is `!Send + !Sync`, but `Clipboard` needs to be
/// `Send + Sync` for the `ClipboardProvider` trait. This is sound because
/// all NSPasteboard calls happen on the main thread — the marker proves
/// thread affinity was checked at construction time, and GPUI ensures the
/// view code that calls clipboard methods always runs on the main thread.
pub struct Clipboard {
    _marker: MainThreadMarker,
}

// SAFETY: Clipboard is only constructed on the main thread (requires MainThreadMarker)
// and all GPUI view/entity code that uses it also runs on the main thread.
unsafe impl Send for Clipboard {}
// SAFETY: See above — all access is through GPUI's main-thread dispatch.
unsafe impl Sync for Clipboard {}

impl Clipboard {
    /// Create a new clipboard handle. Must be called on the main thread.
    pub fn new(mtm: MainThreadMarker) -> Self {
        Self { _marker: mtm }
    }

    /// Static convenience: read clipboard content (backward-compatible API).
    pub fn read(mtm: MainThreadMarker) -> Result<ClipboardContent, ClipboardError> {
        let cb = Self::new(mtm);
        cb.read_clipboard()
    }

    /// Static convenience: read text (backward-compatible API).
    pub fn read_text_static(mtm: MainThreadMarker) -> Result<String, ClipboardError> {
        let cb = Self::new(mtm);
        ClipboardProvider::read_text(&cb)
    }

    /// Static convenience: read image (backward-compatible API).
    pub fn read_image_static(mtm: MainThreadMarker) -> Result<Vec<u8>, ClipboardError> {
        let cb = Self::new(mtm);
        ClipboardProvider::read_image(&cb)
    }

    /// Static convenience: write text (backward-compatible API).
    pub fn write_text(text: &str, mtm: MainThreadMarker) -> Result<(), ClipboardError> {
        let cb = Self::new(mtm);
        ClipboardProvider::write_text(&cb, text)
    }

    /// Static convenience: write image (backward-compatible API).
    pub fn write_image(png_data: &[u8], mtm: MainThreadMarker) -> Result<(), ClipboardError> {
        let cb = Self::new(mtm);
        ClipboardProvider::write_image(&cb, png_data)
    }

    /// Static convenience: available types (backward-compatible API).
    pub fn available_types_static(mtm: MainThreadMarker) -> Vec<String> {
        let cb = Self::new(mtm);
        ClipboardProvider::available_types(&cb)
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

impl ClipboardProvider for Clipboard {
    fn read_clipboard(&self) -> Result<ClipboardContent, ClipboardError> {
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

    fn read_text(&self) -> Result<String, ClipboardError> {
        // SAFETY: generalPasteboard() returns a process-lifetime singleton; called from main thread.
        let pasteboard = unsafe { NSPasteboard::generalPasteboard() };
        Self::read_text_internal(&pasteboard)
    }

    fn read_image(&self) -> Result<Vec<u8>, ClipboardError> {
        // SAFETY: generalPasteboard() returns a process-lifetime singleton; called from main thread.
        let pasteboard = unsafe { NSPasteboard::generalPasteboard() };
        Self::read_image_internal(&pasteboard)
    }

    fn write_text(&self, text: &str) -> Result<(), ClipboardError> {
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

    fn write_image(&self, png_data: &[u8]) -> Result<(), ClipboardError> {
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

    fn available_types(&self) -> Vec<String> {
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
}
