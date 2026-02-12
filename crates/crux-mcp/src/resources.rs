use rmcp::model::*;

use crate::ipc_client::IpcClient;

pub fn resource_templates() -> Vec<ResourceTemplate> {
    vec![
        Annotated::new(
            RawResourceTemplate {
                uri_template: "crux://pane/{pane_id}/scrollback".into(),
                name: "Pane Scrollback".into(),
                title: Some("Pane Scrollback".into()),
                description: Some("Terminal scrollback buffer content for a pane".into()),
                mime_type: Some("text/plain".into()),
                icons: None,
            },
            None,
        ),
        Annotated::new(
            RawResourceTemplate {
                uri_template: "crux://pane/{pane_id}/state".into(),
                name: "Pane State".into(),
                title: Some("Pane State".into()),
                description: Some("Full pane state as JSON (size, title, cursor, cwd)".into()),
                mime_type: Some("application/json".into()),
                icons: None,
            },
            None,
        ),
    ]
}

/// Parse a resource URI like "crux://pane/{id}/scrollback" or "crux://pane/{id}/state"
pub fn parse_resource_uri(uri: &str) -> Option<(u64, &str)> {
    let rest = uri.strip_prefix("crux://pane/")?;
    let (id_str, resource_type) = rest.split_once('/')?;
    let id: u64 = id_str.parse().ok()?;
    Some((id, resource_type))
}

/// Read resource data from a pane via IPC.
pub fn read_resource_data(
    ipc: &IpcClient,
    pane_id: u64,
    resource_type: &str,
) -> Result<ResourceContents, String> {
    match resource_type {
        "scrollback" => {
            let result = ipc
                .call(
                    crux_protocol::method::PANE_GET_TEXT,
                    serde_json::json!({ "pane_id": pane_id }),
                )
                .map_err(|e| format!("IPC error: {e}"))?;

            let text = if let Some(lines) = result.get("lines").and_then(|v| v.as_array()) {
                lines
                    .iter()
                    .filter_map(|l| l.as_str())
                    .collect::<Vec<_>>()
                    .join("\n")
            } else {
                String::new()
            };

            Ok(ResourceContents::TextResourceContents {
                uri: format!("crux://pane/{pane_id}/scrollback"),
                mime_type: Some("text/plain".into()),
                text,
                meta: None,
            })
        }
        "state" => {
            let result = ipc
                .call(crux_protocol::method::PANE_LIST, serde_json::json!({}))
                .map_err(|e| format!("IPC error: {e}"))?;

            let panes = result
                .get("panes")
                .and_then(|v| v.as_array())
                .ok_or("unexpected pane list format")?;

            let pane = panes
                .iter()
                .find(|p| {
                    p.get("pane_id")
                        .and_then(|v| v.as_u64())
                        .is_some_and(|id| id == pane_id)
                })
                .ok_or(format!("pane {pane_id} not found"))?;

            let json = serde_json::to_string_pretty(pane).unwrap_or_else(|_| pane.to_string());

            Ok(ResourceContents::TextResourceContents {
                uri: format!("crux://pane/{pane_id}/state"),
                mime_type: Some("application/json".into()),
                text: json,
                meta: None,
            })
        }
        other => Err(format!("unknown resource type: {other}")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resource_templates_count() {
        let templates = resource_templates();
        assert_eq!(templates.len(), 2, "should have 2 resource templates");
    }

    #[test]
    fn test_resource_templates_scrollback() {
        let templates = resource_templates();
        let scrollback = templates.iter().find(|t| {
            t.raw.uri_template == "crux://pane/{pane_id}/scrollback"
        });
        assert!(scrollback.is_some(), "scrollback template should exist");
        let template = &scrollback.unwrap().raw;
        assert_eq!(template.name, "Pane Scrollback");
        assert_eq!(template.mime_type, Some("text/plain".into()));
    }

    #[test]
    fn test_resource_templates_state() {
        let templates = resource_templates();
        let state = templates.iter().find(|t| {
            t.raw.uri_template == "crux://pane/{pane_id}/state"
        });
        assert!(state.is_some(), "state template should exist");
        let template = &state.unwrap().raw;
        assert_eq!(template.name, "Pane State");
        assert_eq!(template.mime_type, Some("application/json".into()));
    }

    #[test]
    fn test_parse_resource_uri_scrollback() {
        let result = parse_resource_uri("crux://pane/42/scrollback");
        assert_eq!(result, Some((42, "scrollback")));
    }

    #[test]
    fn test_parse_resource_uri_state() {
        let result = parse_resource_uri("crux://pane/123/state");
        assert_eq!(result, Some((123, "state")));
    }

    #[test]
    fn test_parse_resource_uri_zero_id() {
        let result = parse_resource_uri("crux://pane/0/scrollback");
        assert_eq!(result, Some((0, "scrollback")));
    }

    #[test]
    fn test_parse_resource_uri_large_id() {
        let result = parse_resource_uri("crux://pane/99999999/state");
        assert_eq!(result, Some((99999999, "state")));
    }

    #[test]
    fn test_parse_resource_uri_invalid_scheme() {
        let result = parse_resource_uri("http://pane/42/scrollback");
        assert_eq!(result, None);
    }

    #[test]
    fn test_parse_resource_uri_missing_prefix() {
        let result = parse_resource_uri("pane/42/scrollback");
        assert_eq!(result, None);
    }

    #[test]
    fn test_parse_resource_uri_invalid_id() {
        let result = parse_resource_uri("crux://pane/notanumber/scrollback");
        assert_eq!(result, None);
    }

    #[test]
    fn test_parse_resource_uri_missing_resource_type() {
        let result = parse_resource_uri("crux://pane/42");
        assert_eq!(result, None);
    }

    #[test]
    fn test_parse_resource_uri_missing_id() {
        let result = parse_resource_uri("crux://pane//scrollback");
        assert_eq!(result, None);
    }

    #[test]
    fn test_parse_resource_uri_empty_string() {
        let result = parse_resource_uri("");
        assert_eq!(result, None);
    }

    #[test]
    fn test_parse_resource_uri_trailing_slash() {
        let result = parse_resource_uri("crux://pane/42/scrollback/");
        // This should parse the resource_type as "scrollback/"
        assert_eq!(result, Some((42, "scrollback/")));
    }

    #[test]
    fn test_parse_resource_uri_extra_segments() {
        let result = parse_resource_uri("crux://pane/42/scrollback/extra");
        // The parser splits on first '/', so resource_type will be "scrollback/extra"
        assert_eq!(result, Some((42, "scrollback/extra")));
    }
}
