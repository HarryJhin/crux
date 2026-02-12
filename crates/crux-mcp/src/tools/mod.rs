pub mod command;
pub mod content;
pub mod pane;
pub mod state;

pub(crate) fn extract_lines(result: &serde_json::Value) -> String {
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
