use gpui::*;

use crux_terminal_view::CruxTerminalView;

fn main() {
    env_logger::init();

    // Ensure xterm-crux terminfo is installed before starting the application
    crux_terminal_view::ensure_terminfo_installed();

    Application::new().run(|cx: &mut App| {
        cx.open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds {
                    origin: point(px(0.0), px(0.0)),
                    size: size(px(800.0), px(600.0)),
                })),
                ..Default::default()
            },
            |window, cx| {
                let entity = cx.new(CruxTerminalView::new);
                let focus = entity.read(cx).focus_handle(cx);
                focus.focus(window);
                entity
            },
        )
        .unwrap();
    });
}
