//! MCP prompts — pre-configured conversation templates for AI agents.

use rmcp::model::*;
use rmcp::ErrorData as McpError;

/// Return the list of available prompts.
pub fn list() -> Vec<Prompt> {
    vec![
        Prompt::new(
            "terminal-debug",
            Some("Debug a failed command in a terminal pane"),
            Some(vec![
                PromptArgument {
                    name: "pane_id".into(),
                    title: None,
                    description: Some("Pane ID to inspect (uses active pane if omitted)".into()),
                    required: Some(false),
                },
                PromptArgument {
                    name: "command".into(),
                    title: None,
                    description: Some("The command that failed".into()),
                    required: Some(true),
                },
            ]),
        ),
        Prompt::new(
            "pane-overview",
            Some("Get an overview of all terminal panes and their states"),
            None,
        ),
        Prompt::new(
            "command-workflow",
            Some("Execute a multi-step command workflow in terminal"),
            Some(vec![
                PromptArgument {
                    name: "pane_id".into(),
                    title: None,
                    description: Some("Pane ID to use (uses active pane if omitted)".into()),
                    required: Some(false),
                },
                PromptArgument {
                    name: "steps".into(),
                    title: None,
                    description: Some("Comma-separated list of commands to execute".into()),
                    required: Some(true),
                },
            ]),
        ),
    ]
}

/// Handle a get_prompt request.
pub fn get(
    name: &str,
    arguments: &Option<serde_json::Map<String, serde_json::Value>>,
) -> Result<GetPromptResult, McpError> {
    match name {
        "terminal-debug" => terminal_debug(arguments),
        "pane-overview" => pane_overview(arguments),
        "command-workflow" => command_workflow(arguments),
        _ => Err(McpError::invalid_params(
            format!("unknown prompt: {name}"),
            None,
        )),
    }
}

fn terminal_debug(
    arguments: &Option<serde_json::Map<String, serde_json::Value>>,
) -> Result<GetPromptResult, McpError> {
    let args = arguments
        .as_ref()
        .ok_or_else(|| McpError::invalid_params("terminal-debug requires arguments", None))?;

    let command = args
        .get("command")
        .and_then(|v| v.as_str())
        .ok_or_else(|| McpError::invalid_params("missing required argument: command", None))?;

    let pane_id = args
        .get("pane_id")
        .and_then(|v| v.as_str())
        .unwrap_or("active pane");

    let message = format!(
        r#"Analyze the terminal output from pane {} after running `{}`.
Use crux_get_output to see the recent output, then:
1. Identify the error message
2. Determine the root cause
3. Suggest a fix"#,
        pane_id, command
    );

    Ok(GetPromptResult {
        description: Some(format!("Debug `{}` in pane {}", command, pane_id)),
        messages: vec![PromptMessage::new_text(PromptMessageRole::User, message)],
    })
}

fn pane_overview(
    _arguments: &Option<serde_json::Map<String, serde_json::Value>>,
) -> Result<GetPromptResult, McpError> {
    let message = r#"List all terminal panes using crux_list_panes, then for each pane:
1. Check the current working directory and running process via crux_get_pane_state
2. Get the last few lines of output via crux_get_output
3. Summarize each pane's purpose and status"#;

    Ok(GetPromptResult {
        description: Some("Overview of all terminal panes".into()),
        messages: vec![PromptMessage::new_text(PromptMessageRole::User, message)],
    })
}

fn command_workflow(
    arguments: &Option<serde_json::Map<String, serde_json::Value>>,
) -> Result<GetPromptResult, McpError> {
    let args = arguments
        .as_ref()
        .ok_or_else(|| McpError::invalid_params("command-workflow requires arguments", None))?;

    let steps = args
        .get("steps")
        .and_then(|v| v.as_str())
        .ok_or_else(|| McpError::invalid_params("missing required argument: steps", None))?;

    let pane_id = args
        .get("pane_id")
        .and_then(|v| v.as_str())
        .unwrap_or("active pane");

    let message = format!(
        r#"Execute the following commands sequentially in pane {}:
{}

For each command:
1. Run it with crux_execute_command
2. Check output for errors with crux_get_output
3. If an error occurs, stop and report the issue
4. Wait for completion with crux_wait_for_output if needed"#,
        pane_id, steps
    );

    Ok(GetPromptResult {
        description: Some(format!("Execute workflow in pane {}", pane_id)),
        messages: vec![PromptMessage::new_text(PromptMessageRole::User, message)],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_list_returns_three_prompts() {
        let prompts = list();
        assert_eq!(prompts.len(), 3);
        assert_eq!(prompts[0].name, "terminal-debug");
        assert_eq!(prompts[1].name, "pane-overview");
        assert_eq!(prompts[2].name, "command-workflow");
    }

    #[test]
    fn test_terminal_debug_with_valid_args() {
        let mut args = serde_json::Map::new();
        args.insert(
            "command".into(),
            serde_json::Value::String("npm test".into()),
        );
        args.insert("pane_id".into(), serde_json::Value::String("123".into()));

        let result = get("terminal-debug", &Some(args));
        assert!(result.is_ok());

        let prompt_result = result.unwrap();
        assert_eq!(prompt_result.messages.len(), 1);
        assert!(matches!(
            prompt_result.messages[0].role,
            PromptMessageRole::User
        ));
    }

    #[test]
    fn test_terminal_debug_without_pane_id() {
        let mut args = serde_json::Map::new();
        args.insert(
            "command".into(),
            serde_json::Value::String("cargo build".into()),
        );

        let result = get("terminal-debug", &Some(args));
        assert!(result.is_ok());

        let prompt_result = result.unwrap();
        assert_eq!(prompt_result.messages.len(), 1);
    }

    #[test]
    fn test_terminal_debug_without_required_command() {
        let args = serde_json::Map::new();
        let result = get("terminal-debug", &Some(args));
        assert!(result.is_err());
    }

    #[test]
    fn test_terminal_debug_without_arguments() {
        let result = get("terminal-debug", &None);
        assert!(result.is_err());
    }

    #[test]
    fn test_pane_overview_with_no_args() {
        let result = get("pane-overview", &None);
        assert!(result.is_ok());

        let prompt_result = result.unwrap();
        assert_eq!(prompt_result.messages.len(), 1);
        assert!(matches!(
            prompt_result.messages[0].role,
            PromptMessageRole::User
        ));
    }

    #[test]
    fn test_command_workflow_with_valid_args() {
        let mut args = serde_json::Map::new();
        args.insert(
            "steps".into(),
            serde_json::Value::String("cargo build, cargo test, cargo clippy".into()),
        );
        args.insert("pane_id".into(), serde_json::Value::String("456".into()));

        let result = get("command-workflow", &Some(args));
        assert!(result.is_ok());

        let prompt_result = result.unwrap();
        assert_eq!(prompt_result.messages.len(), 1);
    }

    #[test]
    fn test_command_workflow_without_pane_id() {
        let mut args = serde_json::Map::new();
        args.insert(
            "steps".into(),
            serde_json::Value::String("git status, git diff".into()),
        );

        let result = get("command-workflow", &Some(args));
        assert!(result.is_ok());
    }

    #[test]
    fn test_command_workflow_without_required_steps() {
        let args = serde_json::Map::new();
        let result = get("command-workflow", &Some(args));
        assert!(result.is_err());
    }

    #[test]
    fn test_unknown_prompt_returns_error() {
        let result = get("unknown-prompt", &None);
        assert!(result.is_err());
    }
}
