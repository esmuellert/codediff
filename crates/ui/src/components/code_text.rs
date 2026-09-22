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
use crate::view::terminal_lines::TerminalLine;

const TAB_WIDTH: u8 = 4;

type Emphasis = Range<u32>;

pub(crate) type CodeTextInputs = (
    Rc<str>,
    Rc<[Range<u32>]>,
    Option<u32>,
    Rc<[u32]>,
    Rc<[syntax::Span]>,
);

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

pub(crate) fn horizontal_scroll_extent(longest_line_cells: u32, viewport_width: u16) -> u32 {
    longest_line_cells.saturating_sub(u32::from(viewport_width))
}

pub(crate) fn prepare_code_text_inputs_from_decorations(
    text: &str,
    terminal_line: &TerminalLine,
    decorations: &align::LineDecorations,
    syntax: &[syntax::Span],
) -> CodeTextInputs {
    let changed_ranges: Vec<Range<u32>> = decorations
        .characters
        .iter()
        .map(|character| character.bytes.clone())
        .collect();
    let fill_from = decorations
        .characters
        .iter()
        .filter(|character| character.fill_to_edge)
        .map(|character| character.bytes.start)
        .min();
    prepare_code_text_inputs(
        text,
        terminal_line,
        &changed_ranges,
        fill_from,
        &decorations.empty_markers,
        syntax,
    )
}

pub(crate) fn prepare_code_text_inputs(
    text: &str,
    terminal_line: &TerminalLine,
    diff: &[Range<u32>],
    fill_from: Option<u32>,
    empty_markers: &[u32],
    syntax: &[syntax::Span],
) -> CodeTextInputs {
    let source_range = match terminal_line {
        TerminalLine::SourceCode { bytes, .. } => bytes.clone(),
        TerminalLine::Filler => 0..text.len() as u32,
    };
    let source_start = source_range.start;
    let source_end = source_range.end;
    let fragment = text
        .get(source_start as usize..source_end as usize)
        .unwrap_or("");
    let local_range = |range: &Range<u32>| {
        let start = range.start.max(source_start);
        let end = range.end.min(source_end);
        (start < end).then(|| (start - source_start)..(end - source_start))
    };
    let diff = diff.iter().filter_map(local_range).collect::<Vec<_>>();
    let fill_from = fill_from.and_then(|from| {
        if from <= source_start {
            Some(0)
        } else if from < source_end {
            Some(from - source_start)
        } else {
            None
        }
    });
    let empty_markers = empty_markers
        .iter()
        .filter_map(|marker| {
            if *marker >= source_start
                && (*marker < source_end
                    || (*marker == source_end && source_end == text.len() as u32))
            {
                Some(*marker - source_start)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    let syntax = syntax
        .iter()
        .filter_map(|span| {
            let bytes = local_range(&span.bytes)?;
            let mut span = span.clone();
            span.bytes = bytes;
            Some(span)
        })
        .collect::<Vec<_>>();
    (
        Rc::from(fragment),
        Rc::from(diff.into_boxed_slice()),
        fill_from,
        Rc::from(empty_markers.into_boxed_slice()),
        Rc::from(syntax.into_boxed_slice()),
    )
}

fn set_at(paint: &mut Paint<'_>, x: i64, y: i64, symbol: &str, style: Style) {
    if x >= 0 && y >= 0 && x <= i64::from(u16::MAX) && y <= i64::from(u16::MAX) {
        paint.set(x as u16, y as u16, symbol, style);
    }
}

fn set_style_at(paint: &mut Paint<'_>, x: i64, y: i64, style: Style) {
    if x >= 0 && y >= 0 && x <= i64::from(u16::MAX) && y <= i64::from(u16::MAX) {
        paint.set_style(x as u16, y as u16, style);
    }
}

fn fill_terminal_line(paint: &mut Paint<'_>, width: u16, style: Style) {
    let (origin_x, origin_y) = paint.origin();
    for cell in 0..u32::from(width) {
        set_at(
            paint,
            origin_x.saturating_add(i64::from(cell)),
            origin_y,
            " ",
            style,
        );
    }
}

fn paint_code_line(
    paint: &mut Paint<'_>,
    _line_area: Rect,
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
    let width = paint.content_area().width;
    if width == 0 {
        return;
    }
    fill_terminal_line(paint, width, base);

    let index = LineIndex::new(line, TAB_WIDTH);
    let right = first_cell.saturating_add(u32::from(width));
    let (origin_x, origin_y) = paint.origin();

    if let Some(byte) = fill_from {
        let from = index
            .byte_to_cell(ByteOff(byte))
            .get()
            .saturating_sub(first_cell)
            .min(u32::from(width));
        for offset in from..u32::from(width) {
            set_style_at(
                paint,
                origin_x.saturating_add(i64::from(offset)),
                origin_y,
                emphasis,
            );
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
                set_at(
                    paint,
                    origin_x.saturating_add(i64::from(cell.saturating_sub(first_cell))),
                    origin_y,
                    " ",
                    style,
                );
            }
            continue;
        }

        set_at(
            paint,
            origin_x.saturating_add(i64::from(from.saturating_sub(first_cell))),
            origin_y,
            &line_index::sanitize(grapheme.text),
            style,
        );
        for cell in (from + 1)..to {
            set_at(
                paint,
                origin_x.saturating_add(i64::from(cell.saturating_sub(first_cell))),
                origin_y,
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
        let x = origin_x.saturating_add(i64::from(column.saturating_sub(first_cell)));
        let style = if x >= 0
            && origin_y >= 0
            && x <= i64::from(u16::MAX)
            && origin_y <= i64::from(u16::MAX)
        {
            paint
                .style_at(x as u16, origin_y as u16)
                .unwrap_or_default()
                .add_modifier(Modifier::UNDERLINED)
        } else {
            Style::default().add_modifier(Modifier::UNDERLINED)
        };
        let style = emphasis
            .bg
            .map_or(style, |colour| style.underline_color(colour));
        set_style_at(paint, x, origin_y, style);
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
    let content_width = width_in_cells(&text).clamp(1, u32::from(u16::MAX)) as u16;

    rsx! {
        Canvas {
            layout: Layout { grow: 1, basis: Basis::Length(content_width), shrink: 0, ..Default::default() },
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
                    let (origin_x, origin_y) = paint.origin();
                    for offset in 0..u32::from(paint.content_area().width) {
                        let col = first_cell + offset;
                        if selected.contains(&col) {
                            set_style_at(
                                paint,
                                origin_x.saturating_add(i64::from(offset)),
                                origin_y,
                                theme.selection,
                            );
                        }
                    }
                }
            }),
            ..
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source(bytes: Range<u32>) -> TerminalLine {
        TerminalLine::SourceCode {
            source_line: 1,
            bytes,
        }
    }

    #[test]
    fn diff_inputs_collect_ranges_and_use_the_earliest_fill() {
        let decorations = align::LineDecorations {
            characters: vec![
                align::CharacterDecoration {
                    bytes: 5..6,
                    fill_to_edge: true,
                },
                align::CharacterDecoration {
                    bytes: 0..1,
                    fill_to_edge: false,
                },
                align::CharacterDecoration {
                    bytes: 2..6,
                    fill_to_edge: true,
                },
            ],
            empty_markers: vec![1, 6],
            ..Default::default()
        };
        let syntax = vec![syntax::Span::new(1..3, syntax::Style::pen(syntax::Pen(1)))];
        let (text, diff, fill_from, empty_markers, spans) =
            prepare_code_text_inputs_from_decorations(
                "abcdef",
                &source(0..6),
                &decorations,
                &syntax,
            );

        assert_eq!(&*text, "abcdef");
        assert_eq!(&*diff, &[5..6, 0..1, 2..6]);
        assert_eq!(fill_from, Some(2));
        assert_eq!(&*empty_markers, &[1, 6]);
        assert_eq!(spans.as_ref(), syntax.as_slice());
    }

    #[test]
    fn diff_inputs_preserve_fragment_markers_and_syntax_without_character_ranges() {
        let decorations = align::LineDecorations {
            empty_markers: vec![0, 1, 4, 5, 6],
            ..Default::default()
        };
        let syntax = vec![
            syntax::Span::new(0..4, syntax::Style::pen(syntax::Pen(1))),
            syntax::Span::new(4..6, syntax::Style::pen(syntax::Pen(2))),
        ];
        let (text, diff, fill_from, empty_markers, spans) =
            prepare_code_text_inputs_from_decorations(
                "a日bc",
                &source(1..5),
                &decorations,
                &syntax,
            );

        assert_eq!(&*text, "日b");
        assert!(diff.is_empty());
        assert_eq!(fill_from, None);
        assert_eq!(&*empty_markers, &[0, 3]);
        assert_eq!(
            spans.as_ref(),
            &[
                syntax::Span::new(0..3, syntax::Style::pen(syntax::Pen(1))),
                syntax::Span::new(3..4, syntax::Style::pen(syntax::Pen(2))),
            ]
        );
    }

    #[test]
    fn full_line_inputs_are_unchanged() {
        let syntax = vec![syntax::Span::new(1..3, syntax::Style::pen(syntax::Pen(1)))];
        let (text, diff, fill_from, empty_markers, syntax) = prepare_code_text_inputs(
            "abcdef",
            &source(0..6),
            &[0..2, 4..6],
            Some(4),
            &[2],
            &syntax,
        );

        assert_eq!(&*text, "abcdef");
        assert_eq!(&*diff, &[0..2, 4..6]);
        assert_eq!(fill_from, Some(4));
        assert_eq!(&*empty_markers, &[2]);
        assert_eq!(syntax[0].bytes, 1..3);
    }

    #[test]
    fn fragment_inputs_are_clipped_and_shifted() {
        let syntax = vec![
            syntax::Span::new(0..3, syntax::Style::pen(syntax::Pen(1))),
            syntax::Span::new(3..6, syntax::Style::pen(syntax::Pen(2))),
        ];
        let (text, diff, fill_from, empty_markers, syntax) = prepare_code_text_inputs(
            "abcdef",
            &source(2..5),
            &[0..2, 1..4, 4..6],
            Some(1),
            &[1, 2, 5],
            &syntax,
        );

        assert_eq!(&*text, "cde");
        assert_eq!(&*diff, &[0..2, 2..3]);
        assert_eq!(fill_from, Some(0));
        assert_eq!(&*empty_markers, &[0]);
        assert_eq!(syntax[0].bytes, 0..1);
        assert_eq!(syntax[1].bytes, 1..3);
    }

    #[test]
    fn fragments_preserve_utf8_boundaries() {
        let diff = std::iter::once(1..4).collect::<Vec<_>>();
        let (text, diff, _, _, _) =
            prepare_code_text_inputs("a日b", &source(1..4), &diff, None, &[], &[]);
        let expected = std::iter::once(0..3).collect::<Vec<_>>();

        assert_eq!(&*text, "日");
        assert_eq!(&*diff, expected.as_slice());
    }
}
