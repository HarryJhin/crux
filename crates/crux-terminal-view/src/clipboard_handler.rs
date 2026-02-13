//! Clipboard handling for CruxTerminalView.

use gpui::*;

use crux_terminal::TermMode;

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
        let mode = self.terminal.content().mode;
        if mode.contains(TermMode::BRACKETED_PASTE) {
            self.terminal.write_to_pty(b"\x1b[200~");
            self.terminal.write_to_pty(data);
            self.terminal.write_to_pty(b"\x1b[201~");
        } else {
            self.terminal.write_to_pty(data);
        }
    }
}
