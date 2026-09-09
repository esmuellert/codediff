//! One line of code with diff and syntax highlighting.

use std::ops::Range;
use std::rc::Rc;

use file_types::DiffVersion;
use line_index::{ByteOff, CellCol, LineIndex};
use loom::{Basis, Canvas, CanvasProps, Layout, Node, Paint, Scope, component, rsx, use_context};
use ratatui::{
    layout::Rect,
    style::{Modifier, Style},
};

use super::context::Ui;
use crate::theme::Code;

const TAB_WIDTH: u8 = 4;

type Emphasis = Range<u32>;

/// Diff backgrounds and syntax foregrounds for one source line.
#[derive(Debug, Clone, Copy)]
struct CodeTextInk<'a> {
    base: Style,
    emphasis: Style,
    spans: &'a [Emphasis],
    fill_from: Option<u32>,
    empty_markers: &'a [u32],
    syntax: &'a [syntax::Span],
    code: &'a Code,
}

pub(crate) struct DiffTextStyles {
    pub unchanged: Style,
    pub changed: Style,
}

pub(crate) fn styles_for_diff(
    theme: &crate::theme::Theme,
    version: DiffVersion,
    line_background: bool,
) -> DiffTextStyles {
    let line_change = match version {
        DiffVersion::Original => theme.deleted,
        DiffVersion::Modified => theme.inserted,
    };
    let changed_text = match version {
        DiffVersion::Original => theme.deleted_text,
        DiffVersion::Modified => theme.inserted_text,
    };
    DiffTextStyles {
        unchanged: if line_background {
            theme.normal.patch(line_change)
        } else {
            theme.normal
        },
        changed: theme.normal.patch(changed_text),
    }
}

fn width_in_cells(text: &str) -> u32 {
    LineIndex::new(text, TAB_WIDTH).width().0
}

pub(crate) fn longest_line_cells(lines: &[String]) -> u32 {
    lines
        .iter()
        .map(|line| width_in_cells(line))
        .max()
        .unwrap_or(0)
}

fn fill_terminal_line(paint: &mut Paint<'_>, line: Rect, style: Style) {
    for x in line.x..line.right() {
        paint.set(x, line.y, " ", style);
    }
}

fn paint_code_line(
    paint: &mut Paint<'_>,
    line_area: Rect,
    line: &str,
    first_cell: u32,
    ink: CodeTextInk<'_>,
) {
    let CodeTextInk {
        base,
        emphasis,
        spans,
        fill_from,
        empty_markers,
        syntax,
        code,
    } = ink;
    fill_terminal_line(paint, line_area, base);
    if line_area.width == 0 {
        return;
    }

    let index = LineIndex::new(line, TAB_WIDTH);
    let right = first_cell.saturating_add(u32::from(line_area.width));

    if let Some(byte) = fill_from {
        let from = index
            .byte_to_cell(ByteOff(byte))
            .get()
            .saturating_sub(first_cell)
            .min(u32::from(line_area.width));
        for offset in from..u32::from(line_area.width) {
            paint.set_style(line_area.x + offset as u16, line_area.y, emphasis);
        }
    }

    for grapheme in index.graphemes_in_cells(CellCol(first_cell)..CellCol(right)) {
        let cells = grapheme.cells();
        let byte = grapheme.byte.get();
        let under = if byte_in_span(spans, byte) || fill_from.is_some_and(|start| byte >= start) {
            emphasis
        } else {
            base
        };
        let style = under.patch(syntax_style_at(syntax, code, byte));

        let clipped_left = cells.start < first_cell;
        let clipped_right = cells.end > right;
        let from = cells.start.max(first_cell);
        let to = cells.end.min(right);

        if clipped_left || clipped_right || grapheme.is_tab() {
            for cell in from..to {
                paint.set(
                    line_area.x + (cell - first_cell) as u16,
                    line_area.y,
                    " ",
                    style,
                );
            }
            continue;
        }

        paint.set(
            line_area.x + (from - first_cell) as u16,
            line_area.y,
            &line_index::sanitize(grapheme.text),
            style,
        );
        for cell in (from + 1)..to {
            paint.set(
                line_area.x + (cell - first_cell) as u16,
                line_area.y,
                "",
                style,
            );
        }
    }

    for byte in empty_markers {
        let column = index.byte_to_cell(ByteOff(*byte)).get();
        if column < first_cell || column >= right {
            continue;
        }
        let x = line_area.x + (column - first_cell) as u16;
        let mut style = paint
            .style_at(x, line_area.y)
            .unwrap_or_default()
            .add_modifier(Modifier::UNDERLINED);
        if let Some(colour) = emphasis.bg {
            style = style.underline_color(colour);
        }
        paint.set_style(x, line_area.y, style);
    }
}

fn byte_in_span(spans: &[Emphasis], byte: u32) -> bool {
    spans.iter().any(|span| span.contains(&byte))
}

fn syntax_style_at(spans: &[syntax::Span], code: &Code, byte: u32) -> Style {
    let Some(span) = spans.iter().find(|span| span.bytes.contains(&byte)) else {
        return Style::new();
    };
    let mut style = match code.pen(span.style.pen) {
        Some(colour) => Style::new().fg(colour),
        None => Style::new(),
    };
    for (on, modifier) in [
        (span.style.bold, Modifier::BOLD),
        (span.style.italic, Modifier::ITALIC),
        (span.style.underline, Modifier::UNDERLINED),
        (span.style.strikethrough, Modifier::CROSSED_OUT),
    ] {
        if on {
            style = style.add_modifier(modifier);
        }
    }
    style
}

#[component]
pub fn CodeText(
    scope: &mut Scope,
    text: Rc<str>,
    first_cell: u32,
    diff: Rc<[Range<u32>]>,
    fill_from: Option<u32>,
    empty_markers: Rc<[u32]>,
    syntax: Rc<[syntax::Span]>,
    unchanged_style: Style,
    changed_style: Style,
    selection: Option<Range<u32>>,
) -> Node {
    let theme = use_context::<Ui>(scope).theme;

    let text = Rc::clone(text);
    let first_cell = *first_cell;
    let diff = Rc::clone(diff);
    let fill_from = *fill_from;
    let empty_markers = Rc::clone(empty_markers);
    let syntax = Rc::clone(syntax);
    let unchanged_style = *unchanged_style;
    let changed_style = *changed_style;
    let selection = selection.clone();

    rsx! {
        Canvas {
            layout: Layout { grow: 1, basis: Basis::Length(1), shrink: 0, ..Default::default() },
            paint: Rc::new(move |paint: &mut loom::Paint<'_>| {
                let area = paint.area();
                paint_code_line(
                    paint,
                    area,
                    &text,
                    first_cell,
                    CodeTextInk {
                        base: unchanged_style,
                        emphasis: changed_style,
                        spans: &diff,
                        fill_from,
                        empty_markers: &empty_markers,
                        syntax: &syntax,
                        code: &theme.code,
                    },
                );

                if let Some(ref selected) = selection {
                    for x in area.x..area.right() {
                        let col = first_cell + u32::from(x - area.x);
                        if selected.contains(&col) {
                            paint.set_style(x, area.y, theme.selection);
                        }
                    }
                }
            }),
            ..
        }
    }
}
