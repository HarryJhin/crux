//! Canvas-based terminal element rendering.
//!
//! Uses GPUI's `canvas()` to render terminal cells with text shaping,
//! background colors, selection highlight, and cursor drawing.

use gpui::*;

use crux_terminal::{CellFlags, CursorShape, Line, Point, TerminalContent};

use crate::colors;

/// Semi-transparent selection highlight color.
const SELECTION_ALPHA: f32 = 0.3;

/// Prepaint state collected during the prepaint phase for use in paint.
pub struct TerminalPrepaintState {
    shaped_lines: Vec<ShapedLine>,
    bg_quads: Vec<PaintQuad>,
    selection_quads: Vec<PaintQuad>,
    cursor_quad: Option<PaintQuad>,
    bell_flash: bool,
    /// IME composition overlay: (shaped_line, origin, background_quad).
    composition: Option<(ShapedLine, gpui::Point<Pixels>, PaintQuad)>,
}

/// Render the terminal content as a canvas element.
///
/// Returns a sized canvas that paints terminal cells, backgrounds, selection, and cursor.
#[allow(clippy::too_many_arguments)]
pub fn render_terminal_canvas(
    content: TerminalContent,
    cell_width: Pixels,
    cell_height: Pixels,
    font: Font,
    font_size: Pixels,
    focused: bool,
    bell_active: bool,
    cursor_visible: bool,
    marked_text: Option<String>,
) -> impl IntoElement {
    let fg_color = colors::foreground_hsla();
    let bg_color = colors::background_hsla();
    let cursor_color = colors::cursor_hsla();

    // Selection highlight: use the foreground color with reduced alpha.
    let selection_color = Hsla {
        a: SELECTION_ALPHA,
        ..fg_color
    };

    canvas(
        // Prepaint: shape text lines and collect background/selection quads.
        move |bounds: Bounds<Pixels>, window: &mut Window, _cx: &mut App| {
            let text_system = window.text_system().clone();
            let origin = bounds.origin;

            let mut shaped_lines = Vec::with_capacity(content.rows);
            let mut bg_quads = Vec::new();
            let mut selection_quads = Vec::new();

            // Build line text and text runs for each row.
            for row in 0..content.rows {
                let mut line_text = String::with_capacity(content.cols);
                let mut text_runs: Vec<TextRun> = Vec::new();

                // Track background runs for merging adjacent cells with same bg color.
                let mut bg_run_start: Option<usize> = None;
                let mut bg_run_color: Option<Hsla> = None;

                for col in 0..content.cols {
                    let cell_idx = row * content.cols + col;
                    let (ch, cell_fg, cell_bg, cell_flags) = if cell_idx < content.cells.len() {
                        let cell = &content.cells[cell_idx];
                        let ch = if cell.c == '\0' { ' ' } else { cell.c };
                        (ch, cell.fg, cell.bg, cell.flags)
                    } else {
                        (
                            ' ',
                            crux_terminal::Color::Named(crux_terminal::NamedColor::Foreground),
                            crux_terminal::Color::Named(crux_terminal::NamedColor::Background),
                            CellFlags::empty(),
                        )
                    };

                    let cell_fg_hsla = colors::color_to_hsla(cell_fg);
                    let cell_bg_hsla = colors::color_to_hsla(cell_bg);

                    // Merge horizontally adjacent cells with same non-default background.
                    if cell_bg_hsla != bg_color {
                        if bg_run_color == Some(cell_bg_hsla) {
                            // Continue the current run (no action needed yet).
                        } else {
                            // Flush previous run if it exists.
                            if let (Some(start_col), Some(color)) = (bg_run_start, bg_run_color) {
                                let run_width = (col - start_col) as f32 * cell_width;
                                bg_quads.push(fill(
                                    Bounds::new(
                                        point(
                                            origin.x + cell_width * start_col as f32,
                                            origin.y + cell_height * row as f32,
                                        ),
                                        size(run_width, cell_height),
                                    ),
                                    color,
                                ));
                            }
                            // Start a new run.
                            bg_run_start = Some(col);
                            bg_run_color = Some(cell_bg_hsla);
                        }
                    } else {
                        // Flush run when hitting default background.
                        if let (Some(start_col), Some(color)) = (bg_run_start, bg_run_color) {
                            let run_width = (col - start_col) as f32 * cell_width;
                            bg_quads.push(fill(
                                Bounds::new(
                                    point(
                                        origin.x + cell_width * start_col as f32,
                                        origin.y + cell_height * row as f32,
                                    ),
                                    size(run_width, cell_height),
                                ),
                                color,
                            ));
                        }
                        bg_run_start = None;
                        bg_run_color = None;
                    }

                    // Check if this cell is part of the selection.
                    if let Some(ref sel) = content.selection {
                        let cell_point = Point::new(Line(row as i32), crux_terminal::Column(col));
                        if sel.contains(cell_point) {
                            selection_quads.push(fill(
                                Bounds::new(
                                    point(
                                        origin.x + cell_width * col as f32,
                                        origin.y + cell_height * row as f32,
                                    ),
                                    size(cell_width, cell_height),
                                ),
                                selection_color,
                            ));
                        }
                    }

                    // Skip wide character spacer cells (second cell of a 2-wide char).
                    // Background and selection quads above are still rendered for this cell,
                    // but the text was already emitted by the preceding wide character cell.
                    if cell_flags.contains(CellFlags::WIDE_CHAR_SPACER) {
                        continue;
                    }

                    // Build font properties from cell flags.
                    let cell_font_weight = if cell_flags.contains(CellFlags::BOLD) {
                        FontWeight::BOLD
                    } else {
                        font.weight
                    };

                    let cell_font_style = if cell_flags.contains(CellFlags::ITALIC) {
                        FontStyle::Italic
                    } else {
                        FontStyle::Normal
                    };

                    let cell_font = Font {
                        weight: cell_font_weight,
                        style: cell_font_style,
                        ..font.clone()
                    };

                    // Build underline and strikethrough styles from cell flags.
                    let cell_underline = if cell_flags.intersects(CellFlags::ALL_UNDERLINES) {
                        Some(UnderlineStyle {
                            thickness: px(1.0),
                            color: Some(cell_fg_hsla),
                            wavy: cell_flags.contains(CellFlags::UNDERCURL),
                        })
                    } else {
                        None
                    };

                    let cell_strikethrough = if cell_flags.contains(CellFlags::STRIKEOUT) {
                        Some(StrikethroughStyle {
                            thickness: px(1.0),
                            color: Some(cell_fg_hsla),
                        })
                    } else {
                        None
                    };

                    let char_len = ch.len_utf8();
                    line_text.push(ch);

                    // Try to extend the last run if same style.
                    let can_extend = text_runs.last().is_some_and(|last| {
                        last.color == cell_fg_hsla
                            && last.font.weight == cell_font_weight
                            && last.font.style == cell_font_style
                            && last.underline == cell_underline
                            && last.strikethrough == cell_strikethrough
                    });
                    if can_extend {
                        text_runs.last_mut().unwrap().len += char_len;
                    } else {
                        text_runs.push(TextRun {
                            len: char_len,
                            font: cell_font,
                            color: cell_fg_hsla,
                            background_color: None,
                            underline: cell_underline,
                            strikethrough: cell_strikethrough,
                        });
                    }
                }

                // Flush any remaining background run at end of row.
                if let (Some(start_col), Some(color)) = (bg_run_start, bg_run_color) {
                    let run_width = (content.cols - start_col) as f32 * cell_width;
                    bg_quads.push(fill(
                        Bounds::new(
                            point(
                                origin.x + cell_width * start_col as f32,
                                origin.y + cell_height * row as f32,
                            ),
                            size(run_width, cell_height),
                        ),
                        color,
                    ));
                }

                // Shape the line text.
                if line_text.is_empty() || text_runs.is_empty() {
                    let shaped = text_system.shape_line(
                        SharedString::from(" "),
                        font_size,
                        &[TextRun {
                            len: 1,
                            font: font.clone(),
                            color: fg_color,
                            background_color: None,
                            underline: None,
                            strikethrough: None,
                        }],
                        Some(cell_width),
                    );
                    shaped_lines.push(shaped);
                } else {
                    let shaped = text_system.shape_line(
                        SharedString::from(line_text),
                        font_size,
                        &text_runs,
                        Some(cell_width),
                    );
                    shaped_lines.push(shaped);
                }
            }

            // Build cursor quad (only if visible in blink cycle).
            let cursor_quad = if cursor_visible
                && content.mode.contains(crux_terminal::TermMode::SHOW_CURSOR)
            {
                let cursor_row = content.cursor.point.line.0 as usize;
                let cursor_col = content.cursor.point.column.0;
                let cx_pos = point(
                    origin.x + cell_width * cursor_col as f32,
                    origin.y + cell_height * cursor_row as f32,
                );
                let cell_bounds = Bounds::new(cx_pos, size(cell_width, cell_height));

                match content.cursor.shape {
                    CursorShape::Block if focused => Some(fill(cell_bounds, cursor_color)),
                    CursorShape::Block => {
                        Some(outline(cell_bounds, cursor_color, BorderStyle::Solid))
                    }
                    CursorShape::Beam => Some(fill(
                        Bounds::new(cx_pos, size(px(2.0), cell_height)),
                        cursor_color,
                    )),
                    CursorShape::Underline => {
                        let underline_y = cx_pos.y + cell_height - px(2.0);
                        Some(fill(
                            Bounds::new(point(cx_pos.x, underline_y), size(cell_width, px(2.0))),
                            cursor_color,
                        ))
                    }
                    _ => {
                        if focused {
                            Some(fill(cell_bounds, cursor_color))
                        } else {
                            Some(outline(cell_bounds, cursor_color, BorderStyle::Solid))
                        }
                    }
                }
            } else {
                None
            };

            // Shape IME composition (preedit) overlay text.
            let composition = marked_text.as_ref().and_then(|text| {
                if text.is_empty() {
                    return None;
                }
                let cursor_row = content.cursor.point.line.0 as usize;
                let cursor_col = content.cursor.point.column.0;
                let comp_origin = point(
                    origin.x + cell_width * cursor_col as f32,
                    origin.y + cell_height * cursor_row as f32,
                );
                let run = TextRun {
                    len: text.len(),
                    font: font.clone(),
                    color: fg_color,
                    background_color: None,
                    underline: Some(UnderlineStyle {
                        thickness: px(1.0),
                        color: Some(fg_color),
                        wavy: false,
                    }),
                    strikethrough: None,
                };
                let shaped = text_system.shape_line(
                    SharedString::from(text.clone()),
                    font_size,
                    &[run],
                    None,
                );
                let comp_bg = fill(
                    Bounds::new(comp_origin, size(shaped.width, cell_height)),
                    Hsla {
                        h: 0.6,
                        s: 0.4,
                        l: 0.85,
                        a: 0.95,
                    },
                );
                Some((shaped, comp_origin, comp_bg))
            });

            TerminalPrepaintState {
                shaped_lines,
                bg_quads,
                selection_quads,
                cursor_quad,
                bell_flash: bell_active,
                composition,
            }
        },
        // Paint: draw backgrounds, selection, text lines, and cursor.
        move |bounds: Bounds<Pixels>,
              state: TerminalPrepaintState,
              window: &mut Window,
              cx: &mut App| {
            let origin = bounds.origin;

            // 1. Paint full background.
            window.paint_quad(fill(bounds, bg_color));

            // 2. Paint non-default cell backgrounds.
            for quad in state.bg_quads {
                window.paint_quad(quad);
            }

            // 3. Paint selection highlight.
            for quad in state.selection_quads {
                window.paint_quad(quad);
            }

            // 4. Paint text lines.
            for (row, shaped_line) in state.shaped_lines.iter().enumerate() {
                let line_origin = point(origin.x, origin.y + cell_height * row as f32);
                if let Err(e) = shaped_line.paint(line_origin, cell_height, window, cx) {
                    log::warn!("failed to paint terminal line {}: {}", row, e);
                }
            }

            // 5. Paint cursor.
            if let Some(cursor_quad) = state.cursor_quad {
                window.paint_quad(cursor_quad);
            }

            // 6. Paint IME composition (preedit) overlay.
            if let Some((shaped, comp_origin, bg_quad)) = state.composition {
                window.paint_quad(bg_quad);
                if let Err(e) = shaped.paint(comp_origin, cell_height, window, cx) {
                    log::warn!("failed to paint IME composition: {}", e);
                }
            }

            // 7. Paint bell flash overlay.
            if state.bell_flash {
                let flash_color = Hsla {
                    h: 0.0,
                    s: 0.0,
                    l: 1.0,
                    a: 0.1,
                };
                window.paint_quad(fill(bounds, flash_color));
            }
        },
    )
    .size_full()
}
