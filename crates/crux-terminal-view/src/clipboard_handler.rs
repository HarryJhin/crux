//! Clipboard handling for CruxTerminalView.

use gpui::*;

use crux_terminal::{Terminal, TermMode};

use crate::view::CruxTerminalView;

/// Strip dangerous control characters from clipboard text before pasting.
///
/// Allows newline, tab, and carriage return but strips all other
/// C0/C1 control characters (including ESC) to prevent ANSI injection attacks.
pub(crate) fn sanitize_paste_text(text: &str) -> String {
    text.chars()
        .filter(|c| !c.is_control() || *c == '\n' || *c == '\t' || *c == '\r')
        .collect()
}

impl CruxTerminalView {
    /// Paste content from the system clipboard into the terminal.
    ///
    /// Checks NSPasteboard for rich content (images, file paths) first,
    /// then falls back to GPUI's text-only clipboard API.
    pub(crate) fn paste_from_clipboard(&mut self, cx: &mut Context<Self>) {
        // Try rich clipboard (images, file paths) via NSPasteboard.
        #[cfg(target_os = "macos")]
        if let Some(mtm) = objc2_foundation::MainThreadMarker::new() {
            if let Ok(content) = crux_clipboard::Clipboard::read(mtm) {
                match content {
                    crux_clipboard::ClipboardContent::Image { png_data } => {
                        if let Ok(path) = crux_clipboard::save_image_to_temp(&png_data) {
                            let path_str = path.to_string_lossy().to_string();
                            self.write_to_pty_with_bracketed_paste(path_str.as_bytes());
                            return;
                        }
                    }
                    crux_clipboard::ClipboardContent::FilePaths(paths) => {
                        let text = paths
                            .iter()
                            .map(|p| shell_escape::escape(p.to_string_lossy()).to_string())
                            .collect::<Vec<_>>()
                            .join(" ");
                        self.write_to_pty_with_bracketed_paste(text.as_bytes());
                        return;
                    }
                    _ => {} // Fall through to text paste below.
                }
            }
        }

        // Default: text paste via GPUI clipboard API.
        if let Some(item) = cx.read_from_clipboard() {
            if let Some(text) = item.text() {
                if !text.is_empty() {
                    let sanitized = sanitize_paste_text(&text);
                    self.write_to_pty_with_bracketed_paste(sanitized.as_bytes());
                }
            }
        }
    }

    /// Write data to PTY, wrapping in bracketed paste mode if enabled.
    pub(crate) fn write_to_pty_with_bracketed_paste(&mut self, data: &[u8]) {
        // Use mode() instead of content().mode to avoid cloning the entire terminal content
        // just to read a single mode flag.
        let mode = self.terminal.mode();
        if mode.contains(TermMode::BRACKETED_PASTE) {
            self.terminal.write_to_pty(b"\x1b[200~");
            self.terminal.write_to_pty(data);
            self.terminal.write_to_pty(b"\x1b[201~");
        } else {
            self.terminal.write_to_pty(data);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::sanitize_paste_text;

    #[test]
    fn test_sanitize_paste_text_normal_text() {
        // Normal text should pass through unchanged
        assert_eq!(sanitize_paste_text("Hello, World!"), "Hello, World!");
        assert_eq!(sanitize_paste_text("abc123"), "abc123");
        assert_eq!(sanitize_paste_text(""), "");
    }

    #[test]
    fn test_sanitize_paste_text_strips_esc() {
        // ESC characters (\x1b) should be stripped
        assert_eq!(sanitize_paste_text("Hello\x1b[31mWorld"), "Hello[31mWorld");
        assert_eq!(sanitize_paste_text("\x1b[2J"), "[2J");
        assert_eq!(sanitize_paste_text("\x1bOH"), "OH");
    }

    #[test]
    fn test_sanitize_paste_text_strips_control_chars() {
        // Control characters (except \n, \t, \r) should be stripped
        assert_eq!(sanitize_paste_text("Hello\x00World"), "HelloWorld");
        assert_eq!(sanitize_paste_text("Test\x01\x02\x03"), "Test");
        assert_eq!(sanitize_paste_text("\x07Bell"), "Bell"); // BEL
    }

    #[test]
    fn test_sanitize_paste_text_preserves_whitespace() {
        // Newline, tab, and carriage return should be preserved
        assert_eq!(sanitize_paste_text("Line1\nLine2"), "Line1\nLine2");
        assert_eq!(sanitize_paste_text("Col1\tCol2"), "Col1\tCol2");
        assert_eq!(sanitize_paste_text("Text\r\n"), "Text\r\n");
    }

    #[test]
    fn test_sanitize_paste_text_mixed_content() {
        // Mixed content: normal + ESC sequences + control chars
        let input = "Hello\x1b[31m\x00World\nNext\tLine";
        let expected = "Hello[31mWorld\nNext\tLine";
        assert_eq!(sanitize_paste_text(input), expected);
    }

    #[test]
    fn test_sanitize_paste_text_unicode() {
        // Unicode text should be preserved
        assert_eq!(sanitize_paste_text("안녕하세요"), "안녕하세요");
        assert_eq!(sanitize_paste_text("Hello 世界 🌍"), "Hello 世界 🌍");
    }

    #[test]
    fn test_sanitize_paste_text_only_control_chars() {
        // Text with only control characters should result in empty string
        assert_eq!(sanitize_paste_text("\x00\x01\x02\x1b"), "");
    }

    #[test]
    fn test_sanitize_paste_text_ansi_injection_attack() {
        // Simulate ANSI injection attack: ESC sequences that could manipulate terminal
        let malicious = "echo 'harmless'\x1b[2K\x1b[1A\x1brm -rf /";
        let sanitized = sanitize_paste_text(malicious);
        // ESC should be stripped, preventing the attack
        assert!(!sanitized.contains('\x1b'));
        assert_eq!(sanitized, "echo 'harmless'[2K[1Arm -rf /");
    }

    #[test]
    fn test_sanitize_paste_text_c1_control_codes() {
        // C1 control codes (0x80-0x9F) should also be stripped
        let text_with_c1 = "Hello\u{0080}\u{009F}World";
        let sanitized = sanitize_paste_text(text_with_c1);
        assert_eq!(sanitized, "HelloWorld");
    }
}
