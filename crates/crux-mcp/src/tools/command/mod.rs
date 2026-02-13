mod common;
mod execute_command;
mod get_output;
mod send_keys;
mod send_text;
mod wait_for_output;

use crate::server::CruxMcpServer;

pub(crate) fn router() -> rmcp::handler::server::router::tool::ToolRouter<CruxMcpServer> {
    execute_command::router()
        + send_keys::router()
        + send_text::router()
        + get_output::router()
        + wait_for_output::router()
}

#[cfg(test)]
mod tests {
    use crate::tools::extract_lines;

    #[test]
    fn test_extract_lines_with_lines_array() {
        let result = serde_json::json!({
            "lines": ["line 1", "line 2", "line 3"]
        });
        let output = extract_lines(&result);
        assert_eq!(output, "line 1\nline 2\nline 3");
    }

    #[test]
    fn test_extract_lines_empty_array() {
        let result = serde_json::json!({
            "lines": []
        });
        let output = extract_lines(&result);
        assert_eq!(output, "");
    }

    #[test]
    fn test_extract_lines_mixed_types() {
        let result = serde_json::json!({
            "lines": ["line 1", 123, "line 3", null]
        });
        let output = extract_lines(&result);
        // Only strings are extracted
        assert_eq!(output, "line 1\nline 3");
    }

    #[test]
    fn test_extract_lines_no_lines_field() {
        let result = serde_json::json!({
            "data": "some value"
        });
        let output = extract_lines(&result);
        // Should return pretty-printed JSON
        assert!(output.contains("\"data\""));
        assert!(output.contains("\"some value\""));
    }

    #[test]
    fn test_extract_lines_lines_not_array() {
        let result = serde_json::json!({
            "lines": "not an array"
        });
        let output = extract_lines(&result);
        // Should fall back to pretty-printed JSON
        assert!(output.contains("\"lines\""));
    }
}
