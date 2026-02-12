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
        format!("{}...", &s[..max - 3])
    }
}
