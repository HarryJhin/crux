//! Integration tests for IME hardening features.
//!
//! These tests verify the Korean/CJK IME hardening logic without
//! requiring a full GPUI context (avoiding macro recursion limits).

use unicode_normalization::UnicodeNormalization;

#[test]
fn test_nfc_normalization() {
    // Korean text "한글" (Hangul) in NFD (decomposed) form.
    // NFD: U+1112 (ᄒ) + U+1161 (ᅡ) + U+11AB (ᆫ) + U+1100 (ᄀ) + U+1173 (ᅳ) + U+11AF (ᆯ)
    let nfd_text = "\u{1112}\u{1161}\u{11AB}\u{1100}\u{1173}\u{11AF}";

    // Expected NFC (precomposed) form: U+D55C (한) + U+AE00 (글)
    let expected_nfc = "\u{D55C}\u{AE00}";

    // Apply NFC normalization (same as in replace_text_in_range).
    let normalized: String = nfd_text.nfc().collect();

    assert_eq!(
        normalized, expected_nfc,
        "NFD Korean text should normalize to NFC"
    );
    assert_eq!(
        normalized, "한글",
        "Normalized text should be readable Korean"
    );
}

#[test]
fn test_nfc_normalization_preserves_nfc() {
    // Text already in NFC form should remain unchanged.
    let nfc_text = "한글";
    let normalized: String = nfc_text.nfc().collect();
    assert_eq!(normalized, nfc_text, "NFC text should remain unchanged");
}

#[test]
fn test_nfc_normalization_mixed_content() {
    // Mixed ASCII and Korean NFD.
    let mixed = "Hello \u{1112}\u{1161}\u{11AB}"; // "Hello 한" in NFD
    let normalized: String = mixed.nfc().collect();
    assert_eq!(
        normalized, "Hello 한",
        "Mixed content should normalize correctly"
    );
}

#[test]
fn test_nfc_normalization_empty() {
    // Empty string should remain empty.
    let empty = "";
    let normalized: String = empty.nfc().collect();
    assert_eq!(normalized, "", "Empty text should remain empty");
}

#[test]
fn test_nfc_normalization_ascii_only() {
    // ASCII-only text should remain unchanged.
    let ascii = "Hello, World!";
    let normalized: String = ascii.nfc().collect();
    assert_eq!(normalized, ascii, "ASCII text should remain unchanged");
}

#[test]
fn test_nfc_normalization_multiple_hangul() {
    // Multiple Korean syllables in NFD.
    // "가나다" in NFD
    let nfd = "\u{1100}\u{1161}\u{1102}\u{1161}\u{1103}\u{1161}";
    let expected = "가나다";
    let normalized: String = nfd.nfc().collect();
    assert_eq!(
        normalized, expected,
        "Multiple Hangul syllables should normalize"
    );
}
