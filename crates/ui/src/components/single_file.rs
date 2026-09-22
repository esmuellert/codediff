//! One full-width file with no diff decorations.

use std::rc::Rc;

use align::{ViewLine, ViewLineContent, ViewLineType};
use file_types::{DiffType, DiffVersion};
use loom::{
    Basis, Column, ColumnProps, Layout, Node, Row, RowProps, Scope, Scroll, ScrollProps, component,
    rsx, use_context,
};

use super::code_text::{CodeText, CodeTextProps, horizontal_scroll_extent, longest_line_cells};
use super::context::Ui;
use super::diff_viewer_container::DiffVisibleRange;
use super::gutter::{Gutter, GutterProps, width_for_line_count};
use crate::hooks::use_syntax::use_syntax;
use crate::services::syntax::SyntaxService;
use crate::view::terminal_lines::{
    TerminalLine, WrappedViewLine, terminal_line_count, wrap_view_line,
};

pub(crate) struct SingleFileLayout {
    pub(crate) wrapped_lines: Rc<Vec<WrappedViewLine>>,
    pub(crate) gutter_width: u16,
    pub(crate) code_width: u16,
    pub(crate) horizontal_scroll_extent: u32,
    pub(crate) terminal_line_count: u32,
}

pub(crate) fn layout_single_file(
    single: &pipeline::diff::SingleFile,
    width: u16,
    wrap: bool,
) -> SingleFileLayout {
    let line_count = single.lines.len() as u32;
    let gutter_width = width_for_line_count(line_count);
    let code_width = width.saturating_sub(gutter_width);
    let wrapped_lines = wrapped_lines(single, single.side(), code_width, wrap);
    let maximum_line_cells = if wrap {
        u32::from(code_width)
    } else {
        longest_line_cells(&single.lines)
    };
    let terminal_line_count = terminal_line_count(&wrapped_lines);
    SingleFileLayout {
        wrapped_lines: Rc::new(wrapped_lines),
        gutter_width,
        code_width,
        horizontal_scroll_extent: horizontal_scroll_extent(maximum_line_cells, code_width),
        terminal_line_count,
    }
}

#[component]
pub(crate) fn SingleFile(
    scope: &mut Scope,
    content: Rc<pipeline::diff::DiffContent>,
    wrapped_lines: Rc<Vec<WrappedViewLine>>,
    viewport: DiffVisibleRange,
    gutter_width: u16,
    code_width: u16,
    horizontal_scroll_extent: u32,
    auto_focus: bool,
) -> Node {
    let gutter_width = *gutter_width;
    let code_width = *code_width;
    let horizontal_scroll_extent = *horizontal_scroll_extent;
    let viewport = viewport.clone();
    let ctx = use_context::<Ui>(scope);
    let pipeline::diff::DiffContent::SingleFile(single) = content.as_ref() else {
        unreachable!("DiffViewer sends one-sided files to SingleFile")
    };
    let version = single.side();
    let wrapped_lines = wrapped_lines.as_ref();
    let visible_terminal_line_count = viewport
        .visible_terminal_lines
        .end
        .saturating_sub(viewport.visible_terminal_lines.start);
    let syntax = use_syntax(
        scope,
        ctx.syntax_service.as_ref().map(Rc::clone),
        Rc::clone(content),
        DiffType::Single,
        wrapped_lines,
    );
    let syntax = syntax.as_deref();

    let base = ctx.theme.normal;
    let number_style = base.patch(ctx.theme.line_number);
    let mut gutter_rows = Vec::with_capacity(visible_terminal_line_count);
    let mut code_rows = Vec::with_capacity(visible_terminal_line_count);
    for (offset, (original, modified)) in wrapped_lines
        .iter()
        .flat_map(WrappedViewLine::terminal_line_pairs)
        .skip(viewport.visible_terminal_lines.start)
        .take(visible_terminal_line_count)
        .enumerate()
    {
        let line_index = viewport.visible_terminal_lines.start + offset;
        let terminal_line = match version {
            DiffVersion::Original => original,
            DiffVersion::Modified => modified,
        };
        let TerminalLine::SourceCode { source_line, .. } = terminal_line else {
            continue;
        };
        let number = *source_line;
        let Some(text) = single.lines.get(number.saturating_sub(1) as usize) else {
            continue;
        };
        let syntax_spans = syntax
            .map(|store| SyntaxService::line_spans(store, &single.file, version, number))
            .unwrap_or_default();
        let (text, diff, fill_from, empty_markers, syntax_spans) =
            super::code_text::prepare_code_text_inputs(
                text,
                terminal_line,
                &[],
                None,
                &[],
                &syntax_spans,
            );
        gutter_rows.push(rsx! {
            Row {
                key: line_index,
                layout: Layout { basis: Basis::Length(1), shrink: 0, ..Default::default() },
                ..,
                Gutter {
                    key: 0u32,
                    number: terminal_line.gutter_number(),
                    style: number_style,
                    blank: base,
                    width: gutter_width,
                }
            }
        });
        code_rows.push(rsx! {
            Row {
                key: line_index,
                layout: Layout { basis: Basis::Length(1), shrink: 0, ..Default::default() },
                ..,
                CodeText {
                    key: 0u32,
                    text: text,
                    first_cell: 0,
                    diff: diff,
                    fill_from: fill_from,
                    empty_markers: empty_markers,
                    syntax: syntax_spans,
                    unchanged_style: base,
                    changed_style: base,
                    selection: None,
                }
            }
        });
    }

    rsx! {
        Column {
            focusable: true,
            auto_focus: *auto_focus,
            layout: Layout { grow: 1, fill: Some(base), ..Default::default() },
            ..,
            Row {
                layout: Layout { grow: 1, ..Default::default() },
                ..,
                Column {
                    layout: Layout { basis: Basis::Length(gutter_width), shrink: 0, ..Default::default() },
                    ..,
                    { gutter_rows }
                }
                Scroll {
                    view: viewport.horizontal_view,
                    handle: None,
                    horizontal: true,
                    vertical: false,
                    wheel_step: 0,
                    write_metrics: false,
                    content_width: Some(horizontal_scroll_extent.saturating_add(u32::from(code_width))),
                    layout: Layout { grow: 1, shrink: 0, fill: Some(base), ..Default::default() },
                    ..,
                    Column {
                        layout: Layout { grow: 1, fill: Some(base), ..Default::default() },
                        ..,
                        { code_rows }
                    }
                }
            }
        }
    }
}

fn wrapped_lines(
    single: &pipeline::diff::SingleFile,
    version: DiffVersion,
    code_width: u16,
    wrap: bool,
) -> Vec<WrappedViewLine> {
    single
        .lines
        .iter()
        .enumerate()
        .map(|(index, text)| {
            let source_line = index as u32 + 1;
            let view_line = match version {
                DiffVersion::Original => ViewLine {
                    original: ViewLineContent::SourceLine(source_line),
                    modified: ViewLineContent::Filler,
                    kind: ViewLineType::Deleted,
                },
                DiffVersion::Modified => ViewLine {
                    original: ViewLineContent::Filler,
                    modified: ViewLineContent::SourceLine(source_line),
                    kind: ViewLineType::Inserted,
                },
            };
            let (original, modified) = match version {
                DiffVersion::Original => (Some(text.as_str()), None),
                DiffVersion::Modified => (None, Some(text.as_str())),
            };
            if wrap {
                wrap_view_line(
                    view_line,
                    original,
                    modified,
                    code_width,
                    code_width,
                    DiffType::SideBySide,
                )
            } else {
                WrappedViewLine::from_view_line(view_line, original, modified)
            }
        })
        .collect::<Vec<_>>()
}
