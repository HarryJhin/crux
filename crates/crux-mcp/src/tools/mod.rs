pub mod command;
pub mod content;
pub mod pane;
pub mod state;

pub use state::{PaneIdParam, ScrollbackParams};

use std::sync::OnceLock;

/// Compiled regex for stripping ANSI escape sequences.
static ANSI_REGEX: OnceLock<regex::Regex> = OnceLock::new();

/// Strip ANSI escape sequences from text.
///
/// Removes:
/// - CSI sequences: `\x1b[...m` (SGR color/style)
/// - OSC sequences: `\x1b]...\x07` or `\x1b]...\x1b\\` (title, hyperlinks)
/// - Simple escapes: `\x1b(B`, `\x1b)0`, etc. (charset selection)
/// - Other C1 controls: `\x1b[@-_]`
pub(crate) fn strip_ansi(text: &str) -> String {
    let regex = ANSI_REGEX.get_or_init(|| {
        regex::Regex::new(
            r"(?x)
            \x1b\[[0-9;]*[A-Za-z]      # CSI sequences
            |\x1b\][^\x07\x1b]*(?:\x07|\x1b\\)  # OSC sequences
            |\x1b[()][0-9A-B]          # Simple escapes
            |\x1b[@-_]                 # Other C1 controls
            ",
        )
        .expect("invalid ANSI regex")
    });
    regex.replace_all(text, "").to_string()
}

/// Extract lines from IPC result, stripping ANSI escape sequences.
///
/// This is the default extractor used by most tools to ensure clean text output.
pub(crate) fn extract_lines(result: &serde_json::Value) -> String {
    if let Some(lines) = result.get("lines").and_then(|v| v.as_array()) {
        let joined = lines
            .iter()
            .filter_map(|l| l.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        strip_ansi(&joined)
    } else {
        serde_json::to_string_pretty(result).unwrap_or_else(|_| result.to_string())
    }
}

/// Extract lines from IPC result without stripping ANSI escape sequences.
///
/// Use this for tools that explicitly need raw ANSI output (e.g., `crux_get_formatted_output`).
pub(crate) fn extract_lines_raw(result: &serde_json::Value) -> String {
    if let Some(lines) = result.get("lines").and_then(|v| v.as_array()) {
        lines
            .iter()
            .filter_map(|l| l.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    } else {
        serde_json::to_string_pretty(result).unwrap_or_else(|_| result.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_ansi_basic_sgr() {
        let input = "\x1b[31mred\x1b[0m";
        let output = strip_ansi(input);
        assert_eq!(output, "red");
    }

    #[test]
    fn test_strip_ansi_osc_title() {
        let input = "\x1b]0;title\x07text";
        let output = strip_ansi(input);
        assert_eq!(output, "text");
    }

    #[test]
    fn test_strip_ansi_osc_st_terminator() {
        let input = "\x1b]0;title\x1b\\text";
        let output = strip_ansi(input);
        assert_eq!(output, "text");
    }

    #[test]
    fn test_strip_ansi_cursor_movement() {
        let input = "\x1b[2Ahello";
        let output = strip_ansi(input);
        assert_eq!(output, "hello");
    }

    #[test]
    fn test_strip_ansi_no_escapes() {
        let input = "plain text";
        let output = strip_ansi(input);
        assert_eq!(output, "plain text");
    }

    #[test]
    fn test_strip_ansi_mixed() {
        let input = "\x1b[1;32mbold green\x1b[0m normal";
        let output = strip_ansi(input);
        assert_eq!(output, "bold green normal");
    }

    #[test]
    fn test_strip_ansi_simple_escapes() {
        let input = "\x1b(Btext\x1b)0more";
        let output = strip_ansi(input);
        assert_eq!(output, "textmore");
    }

    #[test]
    fn test_strip_ansi_c1_controls() {
        let input = "\x1b@before\x1b_after";
        let output = strip_ansi(input);
        assert_eq!(output, "beforeafter");
    }

    #[test]
    fn test_extract_lines_strips_ansi() {
        let result = serde_json::json!({
            "lines": ["\x1b[31mred\x1b[0m", "\x1b[32mgreen\x1b[0m"]
        });
        let output = extract_lines(&result);
        assert_eq!(output, "red\ngreen");
    }

    #[test]
    fn test_extract_lines_raw_preserves_ansi() {
        let result = serde_json::json!({
            "lines": ["\x1b[31mred\x1b[0m", "\x1b[32mgreen\x1b[0m"]
        });
        let output = extract_lines_raw(&result);
        assert_eq!(output, "\x1b[31mred\x1b[0m\n\x1b[32mgreen\x1b[0m");
    }
}
