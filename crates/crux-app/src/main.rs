mod actions;
mod app;
mod dock;

use gpui::*;

fn main() {
    env_logger::init();

    // Ensure xterm-crux terminfo is installed before starting the application.
    crux_terminal_view::ensure_terminfo_installed();

    let application = Application::new().with_assets(gpui_component_assets::Assets);
    application.run(move |cx: &mut App| {
        gpui_component::init(cx);

        // Register keybindings.
        cx.bind_keys([
            KeyBinding::new("cmd-t", actions::NewTab, None),
            KeyBinding::new("cmd-w", actions::CloseTab, None),
            KeyBinding::new("ctrl-tab", actions::NextTab, None),
            KeyBinding::new("ctrl-shift-tab", actions::PrevTab, None),
            KeyBinding::new("cmd-d", actions::SplitRight, None),
            KeyBinding::new("cmd-shift-d", actions::SplitDown, None),
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
        ]);

        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds {
                    origin: point(px(0.0), px(0.0)),
                    size: size(px(800.0), px(600.0)),
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
