mod actions;
mod app;
mod cli;
mod dock;
mod ipc_dispatch;

use clap::Parser;
use gpui::*;

fn main() {
    env_logger::init();

    let args = cli::CliArgs::parse();

    // If a CLI subcommand was given, run it and exit.
    if let Some(cli::commands::CliCommand::Cli { action }) = args.command {
        if let Err(e) = run_cli(action) {
            eprintln!("error: {e}");
            std::process::exit(1);
        }
        return;
    }

    // Otherwise, start the GUI application.
    crux_terminal_view::ensure_terminfo_installed();

    // Load config for window dimensions.
    let config = crux_config::CruxConfig::load().unwrap_or_else(|e| {
        eprintln!("warning: failed to load config: {}, using defaults", e);
        crux_config::CruxConfig::default()
    });

    let application = Application::new().with_assets(gpui_component_assets::Assets);
    application.run(move |cx: &mut App| {
        gpui_component::init(cx);

        // Register panel factory for session restore.
        dock::terminal_panel::register(cx);

        // Register keybindings.
        cx.bind_keys([
            KeyBinding::new("cmd-t", actions::NewTab, None),
            KeyBinding::new("cmd-w", actions::CloseTab, None),
            KeyBinding::new("cmd-shift-w", actions::ForceCloseTab, None),
            KeyBinding::new("ctrl-tab", actions::NextTab, None),
            KeyBinding::new("ctrl-shift-tab", actions::PrevTab, None),
            KeyBinding::new("cmd-d", actions::SplitRight, None),
            KeyBinding::new("cmd-shift-d", actions::SplitDown, None),
            KeyBinding::new("cmd-ctrl-d", actions::WindowSplitRight, None),
            KeyBinding::new("cmd-ctrl-shift-d", actions::WindowSplitDown, None),
            KeyBinding::new("cmd-shift-enter", actions::ZoomPane, None),
            KeyBinding::new("cmd-1", actions::SelectTab1, None),
            KeyBinding::new("cmd-2", actions::SelectTab2, None),
            KeyBinding::new("cmd-3", actions::SelectTab3, None),
            KeyBinding::new("cmd-4", actions::SelectTab4, None),
            KeyBinding::new("cmd-5", actions::SelectTab5, None),
            KeyBinding::new("cmd-6", actions::SelectTab6, None),
            KeyBinding::new("cmd-7", actions::SelectTab7, None),
            KeyBinding::new("cmd-8", actions::SelectTab8, None),
            KeyBinding::new("cmd-9", actions::SelectTab9, None),
            KeyBinding::new("cmd-]", actions::FocusNextPane, None),
            KeyBinding::new("cmd-[", actions::FocusPrevPane, None),
            KeyBinding::new("cmd-up", actions::PrevPrompt, None),
            KeyBinding::new("cmd-down", actions::NextPrompt, None),
        ]);

        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds {
                    origin: point(px(0.0), px(0.0)),
                    size: size(px(config.window.width), px(config.window.height)),
                })),
                ..Default::default()
            },
            |window, cx| {
                let view = cx.new(|cx| app::CruxApp::new(window, cx));
                cx.new(|cx| gpui_component::Root::new(view, window, cx))
            },
        )
        .unwrap();
    });
}

fn run_cli(action: cli::commands::CliAction) -> anyhow::Result<()> {
    use cli::commands::CliAction;
    use crux_ipc::IpcTransport;
    use crux_protocol::*;

    let client = cli::client::connect()?;

    match action {
        CliAction::SplitPane {
            direction,
            percent,
            pane_id,
            cwd,
            command,
        } => {
            let dir = match direction.as_str() {
                "left" => SplitDirection::Left,
                "top" => SplitDirection::Top,
                "bottom" => SplitDirection::Bottom,
                _ => SplitDirection::Right,
            };
            let params = SplitPaneParams {
                target_pane_id: pane_id.map(PaneId),
                direction: dir,
                size: percent.map(SplitSize::Percent),
                cwd,
                command: if command.is_empty() {
                    None
                } else {
                    Some(command)
                },
                env: None,
            };
            let result = client.call(method::PANE_SPLIT, serde_json::to_value(&params)?)?;
            let result: SplitPaneResult = serde_json::from_value(result)?;
            // Print new pane ID to stdout for scripting.
            println!("{}", result.pane_id);
        }

        CliAction::SendText {
            pane_id,
            no_paste,
            text,
        } => {
            let text = match text {
                Some(t) => t,
                None => {
                    let mut buf = String::new();
                    std::io::Read::read_to_string(&mut std::io::stdin(), &mut buf)?;
                    buf
                }
            };
            let pane_id = pane_id
                .or_else(|| std::env::var("CRUX_PANE").ok()?.parse().ok())
                .map(PaneId);
            let params = SendTextParams {
                pane_id,
                text,
                bracketed_paste: !no_paste,
            };
            client.call(method::PANE_SEND_TEXT, serde_json::to_value(&params)?)?;
        }

        CliAction::GetText {
            pane_id,
            start_line,
            end_line,
            escapes,
        } => {
            let pane_id = pane_id
                .or_else(|| std::env::var("CRUX_PANE").ok()?.parse().ok())
                .map(PaneId);
            let params = GetTextParams {
                pane_id,
                start_line,
                end_line,
                include_escapes: escapes,
            };
            let result = client.call(method::PANE_GET_TEXT, serde_json::to_value(&params)?)?;
            let result: GetTextResult = serde_json::from_value(result)?;
            for line in &result.lines {
                println!("{line}");
            }
        }

        CliAction::List { format } => {
            let result = client.call(method::PANE_LIST, serde_json::json!({}))?;
            let result: ListPanesResult = serde_json::from_value(result)?;
            if format == "json" {
                println!("{}", serde_json::to_string_pretty(&result.panes)?);
            } else {
                cli::output::print_pane_table(&result.panes);
            }
        }

        CliAction::ActivatePane { pane_id } => {
            let params = ActivatePaneParams {
                pane_id: PaneId(pane_id),
            };
            client.call(method::PANE_ACTIVATE, serde_json::to_value(&params)?)?;
        }

        CliAction::ClosePane { pane_id, force } => {
            let params = ClosePaneParams {
                pane_id: PaneId(pane_id),
                force,
            };
            client.call(method::PANE_CLOSE, serde_json::to_value(&params)?)?;
        }

        CliAction::WindowCreate {
            title,
            width,
            height,
        } => {
            let params = WindowCreateParams {
                title,
                width,
                height,
            };
            let result = client.call(method::WINDOW_CREATE, serde_json::to_value(&params)?)?;
            let result: WindowCreateResult = serde_json::from_value(result)?;
            println!("{}", result.window_id);
        }

        CliAction::WindowList { format } => {
            let result = client.call(method::WINDOW_LIST, serde_json::json!({}))?;
            let result: WindowListResult = serde_json::from_value(result)?;
            if format == "json" {
                println!("{}", serde_json::to_string_pretty(&result.windows)?);
            } else {
                for w in &result.windows {
                    println!(
                        "Window {} | {} panes | focused={}",
                        w.window_id, w.pane_count, w.is_focused
                    );
                }
            }
        }
    }

    Ok(())
}
