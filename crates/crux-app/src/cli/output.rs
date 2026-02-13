//! Formatted table output for CLI commands (pane listing, status display).

use crux_protocol::PaneInfo;

/// Print pane list as a formatted table.
pub fn print_pane_table(panes: &[PaneInfo]) {
    println!(
        "{:<7} {:<7} {:<8} {:<12} {:<20} CWD",
        "WINID", "TABID", "PANEID", "SIZE", "TITLE"
    );
    for p in panes {
        println!(
            "{:<7} {:<7} {:<8} {:>4}x{:<6} {:<20} {}",
            p.window_id,
            p.tab_id,
            p.pane_id,
            p.size.cols,
            p.size.rows,
            truncate(&p.title, 20),
            p.cwd.as_deref().unwrap_or(""),
        );
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        let target = max.saturating_sub(3);
        let end = s.char_indices()
            .map(|(i, _)| i)
            .take_while(|&i| i <= target)
            .last()
            .unwrap_or(0);
        format!("{}...", &s[..end])
    }
}

#[cfg(test)]
mod tests {
    use super::truncate;

    #[test]
    fn test_truncate_ascii_shorter_than_max() {
        let result = truncate("hello", 10);
        assert_eq!(result, "hello");
    }

    #[test]
    fn test_truncate_ascii_exact_max_length() {
        let result = truncate("hello", 5);
        assert_eq!(result, "hello");
    }

    #[test]
    fn test_truncate_ascii_longer_than_max() {
        let result = truncate("hello world", 8);
        assert_eq!(result, "hello...");
    }

    #[test]
    fn test_truncate_multibyte_korean() {
        // "안녕하세요 세계" is 21 bytes but 8 characters (including space)
        let text = "안녕하세요 세계";
        let result = truncate(text, 10);
        // Should truncate at character boundary, not byte boundary
        // With max=10, target=7, should fit "안녕하세" (4 chars) + "..."
        assert!(result.ends_with("..."));
        assert!(result.len() <= 10 + 3); // Allow for multi-byte chars
        // Verify no panic and valid UTF-8
        assert!(result.is_char_boundary(result.len() - 3));
    }

    #[test]
    fn test_truncate_empty_string() {
        let result = truncate("", 10);
        assert_eq!(result, "");
    }

    #[test]
    fn test_truncate_max_less_than_3() {
        // Edge case: max=2, target=0 (saturating_sub)
        let result = truncate("hello", 2);
        // Should still truncate, even if odd result
        assert!(result.ends_with("..."));
    }

    #[test]
    fn test_truncate_max_zero() {
        let result = truncate("hello", 0);
        // target = 0.saturating_sub(3) = 0
        // Should return just "..." (no chars before ellipsis)
        assert_eq!(result, "...");
    }

    #[test]
    fn test_truncate_emoji_4byte_utf8() {
        // Emojis are 4-byte UTF-8 characters
        let text = "Hello 🦀 Rust 🚀 World";
        let result = truncate(text, 15);
        // Should handle 4-byte chars correctly
        assert!(result.ends_with("..."));
        assert!(result.len() <= 15 + 7); // Allow for emoji bytes
        // Verify valid UTF-8 (no panic means success)
    }

    #[test]
    fn test_truncate_mixed_multibyte() {
        // Mix of ASCII, 2-byte, 3-byte, 4-byte UTF-8
        let text = "Hi©안녕🦀";
        let result = truncate(text, 8);
        assert!(result.is_char_boundary(result.len() - 3));
    }
}
