//! URL and file path detection for terminal link handling.

use once_cell::sync::Lazy;
use regex::Regex;

/// Compiled URL regex pattern (compile once, reuse for performance).
/// Matches common URL schemes: http(s), ftp, file, mailto.
static URL_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"(?i)(https?://|ftp://|file://|mailto:)[^\s<>\[\]{}|\\^`"']+"#).unwrap()
});

/// Compiled file path regex pattern for editor integration.
/// Matches patterns like: /path/to/file.rs:42:10, src/main.rs:15, ./relative/path.py:7:3
static FILE_PATH_REGEX: Lazy<Regex> =
    Lazy::new(|| Regex::new(r#"(?:^|[\s(])([./~]?[^\s:]+\.[a-zA-Z]+):(\d+)(?::(\d+))?"#).unwrap());

/// A detected URL match within a line of text.
#[derive(Debug, Clone, PartialEq)]
pub struct UrlMatch {
    pub url: String,
    pub start_col: usize,
    pub end_col: usize,
}

/// A detected file path match for editor integration.
#[derive(Debug, Clone, PartialEq)]
pub struct FileMatch {
    pub path: String,
    pub line: usize,
    pub col: Option<usize>,
    pub start_col: usize,
    pub end_col: usize,
}

/// Detect URLs in a single line of text.
pub fn detect_urls(text: &str) -> Vec<UrlMatch> {
    URL_REGEX
        .find_iter(text)
        .map(|m| {
            let mut url = m.as_str().to_string();
            // Strip trailing punctuation that's likely not part of the URL.
            while let Some(last) = url.chars().last() {
                if matches!(last, '.' | ',' | ';' | ':' | '!' | '?' | ')' | ']' | '}') {
                    url.pop();
                } else {
                    break;
                }
            }
            let url_len = url.len();
            UrlMatch {
                url,
                start_col: m.start(),
                end_col: m.start() + url_len,
            }
        })
        .collect()
}

/// Detect file paths with line/column numbers in a single line of text.
pub fn detect_file_paths(text: &str) -> Vec<FileMatch> {
    FILE_PATH_REGEX
        .captures_iter(text)
        .filter_map(|cap| {
            let full_match = cap.get(0)?;
            let path = cap.get(1)?.as_str().to_string();
            let line_str = cap.get(2)?.as_str();
            let line = line_str.parse::<usize>().ok()?;
            let col = cap.get(3).and_then(|m| m.as_str().parse::<usize>().ok());

            Some(FileMatch {
                path,
                line,
                col,
                start_col: full_match.start(),
                end_col: full_match.end(),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_http_urls() {
        let text = "Visit https://example.com for more info";
        let matches = detect_urls(text);
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].url, "https://example.com");
    }

    #[test]
    fn test_detect_https_urls() {
        let text = "Check http://rust-lang.org and https://docs.rs";
        let matches = detect_urls(text);
        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0].url, "http://rust-lang.org");
        assert_eq!(matches[1].url, "https://docs.rs");
    }

    #[test]
    fn test_detect_ftp_urls() {
        let text = "Download from ftp://files.example.org/archive";
        let matches = detect_urls(text);
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].url, "ftp://files.example.org/archive");
    }

    #[test]
    fn test_detect_file_urls() {
        let text = "Open file:///Users/name/document.pdf";
        let matches = detect_urls(text);
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].url, "file:///Users/name/document.pdf");
    }

    #[test]
    fn test_detect_mailto_urls() {
        let text = "Contact mailto:user@example.com for support";
        let matches = detect_urls(text);
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].url, "mailto:user@example.com");
    }

    #[test]
    fn test_url_with_query_string() {
        let text = "Search https://example.com/search?q=rust&lang=en";
        let matches = detect_urls(text);
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].url, "https://example.com/search?q=rust&lang=en");
    }

    #[test]
    fn test_url_with_parentheses() {
        let text = "Wiki link https://en.wikipedia.org/wiki/Rust_(programming_language)";
        let matches = detect_urls(text);
        assert_eq!(matches.len(), 1);
        assert!(matches[0]
            .url
            .starts_with("https://en.wikipedia.org/wiki/Rust_"));
    }

    #[test]
    fn test_url_with_trailing_punctuation() {
        let text = "Visit https://example.com. It's great!";
        let matches = detect_urls(text);
        assert_eq!(matches.len(), 1);
        // Trailing period should be stripped.
        assert_eq!(matches[0].url, "https://example.com");
    }

    #[test]
    fn test_url_in_sentence_with_comma() {
        let text = "Check https://example.com, then proceed";
        let matches = detect_urls(text);
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].url, "https://example.com");
    }

    #[test]
    fn test_no_url_in_text() {
        let text = "This is plain text without any URLs";
        let matches = detect_urls(text);
        assert_eq!(matches.len(), 0);
    }

    #[test]
    fn test_multiple_urls_in_line() {
        let text = "Visit https://example.com and ftp://files.org for resources";
        let matches = detect_urls(text);
        assert_eq!(matches.len(), 2);
    }

    #[test]
    fn test_detect_absolute_file_path() {
        let text = "Error in /path/to/file.rs:42:10";
        let matches = detect_file_paths(text);
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].path, "/path/to/file.rs");
        assert_eq!(matches[0].line, 42);
        assert_eq!(matches[0].col, Some(10));
    }

    #[test]
    fn test_detect_relative_file_path() {
        let text = "See src/main.rs:15 for details";
        let matches = detect_file_paths(text);
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].path, "src/main.rs");
        assert_eq!(matches[0].line, 15);
        assert_eq!(matches[0].col, None);
    }

    #[test]
    fn test_detect_dotslash_file_path() {
        let text = "Check ./relative/path.py:7:3";
        let matches = detect_file_paths(text);
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].path, "./relative/path.py");
        assert_eq!(matches[0].line, 7);
        assert_eq!(matches[0].col, Some(3));
    }

    #[test]
    fn test_detect_tilde_file_path() {
        let text = "Config at ~/config/app.toml:100";
        let matches = detect_file_paths(text);
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].path, "~/config/app.toml");
        assert_eq!(matches[0].line, 100);
        assert_eq!(matches[0].col, None);
    }

    #[test]
    fn test_no_file_path_in_text() {
        let text = "Just some plain text without file paths";
        let matches = detect_file_paths(text);
        assert_eq!(matches.len(), 0);
    }

    #[test]
    fn test_multiple_file_paths_in_line() {
        let text = "Errors in src/foo.rs:10 and tests/bar.rs:25:5";
        let matches = detect_file_paths(text);
        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0].path, "src/foo.rs");
        assert_eq!(matches[0].line, 10);
        assert_eq!(matches[1].path, "tests/bar.rs");
        assert_eq!(matches[1].line, 25);
        assert_eq!(matches[1].col, Some(5));
    }
}
