//! macOS Text Input Source (TIS) FFI for IME auto-switching.
//!
//! Uses Carbon framework's TISSelectInputSource to switch between
//! input methods when Vim cursor shape changes are detected.

#![cfg(target_os = "macos")]

use std::ffi::c_void;

// Carbon Text Input Source Services FFI.
extern "C" {
    fn TISCopyCurrentKeyboardInputSource() -> *mut c_void;
    fn TISGetInputSourceProperty(source: *const c_void, key: *const c_void) -> *const c_void;
    fn TISCreateInputSourceList(properties: *const c_void, include_all: bool) -> *const c_void;
    fn TISSelectInputSource(source: *mut c_void) -> i32;
}

// CoreFoundation FFI.
extern "C" {
    fn CFRelease(cf: *const c_void);
    fn CFArrayGetCount(array: *const c_void) -> isize;
    fn CFArrayGetValueAtIndex(array: *const c_void, idx: isize) -> *const c_void;
    fn CFStringGetCStringPtr(string: *const c_void, encoding: u32) -> *const std::ffi::c_char;
    fn CFStringCreateWithCString(
        alloc: *const c_void,
        cstr: *const std::ffi::c_char,
        encoding: u32,
    ) -> *mut c_void;
    fn CFDictionaryCreate(
        allocator: *const c_void,
        keys: *const *const c_void,
        values: *const *const c_void,
        num_values: isize,
        key_callbacks: *const c_void,
        value_callbacks: *const c_void,
    ) -> *mut c_void;
    fn CFBooleanGetValue(boolean: *const c_void) -> bool;
    static kCFAllocatorDefault: *const c_void;
    static kCFTypeDictionaryKeyCallBacks: *const c_void;
    static kCFTypeDictionaryValueCallBacks: *const c_void;
}

// TIS property keys (Carbon framework symbols).
extern "C" {
    static kTISPropertyInputSourceID: *const c_void;
    static kTISPropertyInputSourceIsASCIICapable: *const c_void;
    static kTISPropertyInputSourceIsSelectCapable: *const c_void;
    static kTISPropertyInputSourceCategory: *const c_void;
    static kTISCategoryKeyboardInputSource: *const c_void;
}

// Link against Carbon framework.
#[link(name = "Carbon", kind = "framework")]
extern "C" {}

const K_CF_STRING_ENCODING_UTF8: u32 = 0x08000100;

/// Get the current input source identifier string.
pub fn current_input_source() -> Option<String> {
    unsafe {
        let source = TISCopyCurrentKeyboardInputSource();
        if source.is_null() {
            return None;
        }
        let id_ref = TISGetInputSourceProperty(source, kTISPropertyInputSourceID);
        let result = if !id_ref.is_null() {
            let cstr = CFStringGetCStringPtr(id_ref, K_CF_STRING_ENCODING_UTF8);
            if !cstr.is_null() {
                Some(
                    std::ffi::CStr::from_ptr(cstr)
                        .to_string_lossy()
                        .into_owned(),
                )
            } else {
                None
            }
        } else {
            None
        };
        CFRelease(source);
        result
    }
}

/// Switch to the first ASCII-capable keyboard input source.
///
/// Finds all keyboard input sources that are ASCII-capable and selectable,
/// then activates the first one (typically "ABC" or "US" keyboard).
pub fn switch_to_ascii() {
    unsafe {
        // Build filter: { kTISPropertyInputSourceCategory: kTISCategoryKeyboardInputSource }
        let keys = [kTISPropertyInputSourceCategory];
        let values = [kTISCategoryKeyboardInputSource];
        let filter = CFDictionaryCreate(
            kCFAllocatorDefault,
            keys.as_ptr(),
            values.as_ptr(),
            1,
            kCFTypeDictionaryKeyCallBacks,
            kCFTypeDictionaryValueCallBacks,
        );
        if filter.is_null() {
            return;
        }

        let sources = TISCreateInputSourceList(filter, false);
        CFRelease(filter as *const c_void);
        if sources.is_null() {
            return;
        }

        let count = CFArrayGetCount(sources);
        for i in 0..count {
            let source = CFArrayGetValueAtIndex(sources, i);
            if source.is_null() {
                continue;
            }

            // Check ASCII-capable.
            let ascii_prop =
                TISGetInputSourceProperty(source, kTISPropertyInputSourceIsASCIICapable);
            if ascii_prop.is_null() || !CFBooleanGetValue(ascii_prop) {
                continue;
            }

            // Check selectable.
            let select_prop =
                TISGetInputSourceProperty(source, kTISPropertyInputSourceIsSelectCapable);
            if select_prop.is_null() || !CFBooleanGetValue(select_prop) {
                continue;
            }

            // Select this source.
            TISSelectInputSource(source as *mut c_void);
            break;
        }

        CFRelease(sources);
    }
}

/// Switch to a specific input source by its identifier string.
///
/// Looks up the input source matching `source_id` (e.g.
/// "com.apple.inputmethod.Korean.2SetKorean") and activates it.
/// Returns `true` if the switch succeeded, `false` otherwise.
pub fn switch_to_input_source(source_id: &str) -> bool {
    use std::ffi::CString;

    let Ok(c_source_id) = CString::new(source_id) else {
        return false;
    };

    unsafe {
        let cf_source_id = CFStringCreateWithCString(
            kCFAllocatorDefault,
            c_source_id.as_ptr(),
            K_CF_STRING_ENCODING_UTF8,
        );
        if cf_source_id.is_null() {
            return false;
        }

        // Build filter: { kTISPropertyInputSourceID: <source_id> }
        let keys = [kTISPropertyInputSourceID];
        let values = [cf_source_id as *const c_void];
        let filter = CFDictionaryCreate(
            kCFAllocatorDefault,
            keys.as_ptr(),
            values.as_ptr(),
            1,
            kCFTypeDictionaryKeyCallBacks,
            kCFTypeDictionaryValueCallBacks,
        );
        CFRelease(cf_source_id as *const c_void);

        if filter.is_null() {
            return false;
        }

        let sources = TISCreateInputSourceList(filter, true);
        CFRelease(filter as *const c_void);
        if sources.is_null() {
            return false;
        }

        let count = CFArrayGetCount(sources);
        let success = if count > 0 {
            let source = CFArrayGetValueAtIndex(sources, 0);
            if !source.is_null() {
                TISSelectInputSource(source as *mut c_void) == 0
            } else {
                false
            }
        } else {
            false
        };

        CFRelease(sources);
        success
    }
}
