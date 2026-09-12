//! One full-width column showing original and modified lines in sequence.

use std::ops::Range;
use std::rc::Rc;

use align::DiffVersion;
use file_types::DiffType;
use loom::{
    Basis, Column, ColumnProps, Layout, Node, Ref, Row, RowProps, Scope, component, rsx,
    use_context, use_measure, use_memo,
};

use super::code_text::{self, CodeText, CodeTextProps};
use super::context::Ui;
use super::diff_viewer::ViewState;
use super::gutter::{self, Gutter, GutterProps, width_for_line_count};
use super::wrap::{
    TerminalLine, WrappedViewLine, find_terminal_line_index, terminal_line_cells, wrap_view_lines,
    wrapped_view_line_range_for_terminal_lines,
};
use crate::hooks::use_diff_viewer_navigation::{HorizontalDimensions, use_diff_viewer_navigation};
use crate::hooks::use_horizontal_scroll::use_horizontal_scroll;
use crate::hooks::use_scroll::use_scroll;
use crate::hooks::use_syntax::use_syntax;
use crate::services::syntax::SyntaxService;

fn longest_inline_terminal_line_cells(
    lines: &[WrappedViewLine],
    original_lines: &[String],
    modified_lines: &[String],
) -> u32 {
    lines
        .iter()
        .flat_map(|line| line.original.iter().zip(&line.modified))
        .map(|(original, modified)| {
            if matches!(modified, TerminalLine::SourceCode { .. }) {
                terminal_line_cells(modified, modified_lines)
            } else {
                terminal_line_cells(original, original_lines)
            }
        })
        .max()
        .unwrap_or(0)
}

#[component]
pub fn Inline(
    scope: &mut Scope,
    content: Rc<pipeline::diff::DiffContent>,
    view_state: Ref<ViewState>,
    auto_focus: bool,
) -> Node {
    let ctx = use_context::<Ui>(scope);
    let theme = &ctx.theme;
    let pipeline::diff::DiffContent::Diff(diff) = content.as_ref() else {
        unreachable!("DiffViewer sends diffs to Inline")
    };
    let alignment = &diff.alignment;
    let original_line_count = alignment.lines(DiffVersion::Original).len() as u32;
    let modified_line_count = alignment.lines(DiffVersion::Modified).len() as u32;
    let original_gutter_width = width_for_line_count(original_line_count);
    let modified_gutter_width = width_for_line_count(modified_line_count);
    let (node_ref, size) = use_measure(scope);
    let view_state_ref = *view_state;
    let current_view_state = view_state_ref.current().clone();
    let content_id = Rc::as_ptr(content) as usize;
    let code_width = size
        .width
        .saturating_sub(original_gutter_width)
        .saturating_sub(modified_gutter_width);
    let wrapped_lines = use_memo(scope, (content_id, code_width), || {
        wrap_view_lines(alignment, DiffType::Inline, code_width, code_width)
    });
    let maximum_line_cells = use_memo(scope, (content_id, code_width), || {
        longest_inline_terminal_line_cells(
            &wrapped_lines,
            alignment.lines(DiffVersion::Original),
            alignment.lines(DiffVersion::Modified),
        )
    });
    let initial_top = current_view_state
        .first_terminal_line
        .as_ref()
        .and_then(|line| find_terminal_line_index(&wrapped_lines, line))
        .unwrap_or(0);
    let terminal_line_count = wrapped_lines
        .iter()
        .map(|line| line.original.len() as u32)
        .sum();
    let (view, vertical_handle) = use_scroll(scope, terminal_line_count, initial_top, size.height);
    let horizontal_limits = HorizontalDimensions::Inline {
        longest_line_cells: *maximum_line_cells,
        original_gutter_cells: original_gutter_width,
        modified_gutter_cells: modified_gutter_width,
    }
    .limits(size.width);
    let (horizontal_view, horizontal_handle) = use_horizontal_scroll(
        scope,
        horizontal_limits.maximum_first_cell(),
        current_view_state.first_cell,
    );
    let horizontal = horizontal_limits.view(horizontal_view.first_cell);
    if size.width > 0 && size.height > 0 {
        *view_state_ref.current() = ViewState {
            first_terminal_line: (view.top > 0)
                .then(|| {
                    wrapped_lines
                        .iter()
                        .flat_map(|line| line.original.iter().zip(&line.modified))
                        .nth(view.top as usize)
                })
                .flatten()
                .and_then(|(original, modified)| match modified {
                    TerminalLine::SourceCode { .. } => Some(modified),
                    TerminalLine::Filler => match original {
                        TerminalLine::SourceCode { .. } => Some(original),
                        TerminalLine::Filler => None,
                    },
                })
                .cloned(),
            first_cell: horizontal.requested_first_cell,
        };
    }
    let listeners = use_diff_viewer_navigation(vertical_handle, horizontal_handle);
    let visible_wrapped_lines = &wrapped_lines[wrapped_view_line_range_for_terminal_lines(
        &wrapped_lines,
        view.view_lines.start,
        view.view_lines.end,
    )];
    let syntax = use_syntax(
        scope,
        ctx.syntax_service.as_ref().map(Rc::clone),
        Rc::clone(content),
        DiffType::Inline,
        visible_wrapped_lines,
    );
    let syntax = syntax.as_deref();

    let mut rows = Vec::with_capacity(view.view_lines.len());
    for (offset, (original, modified)) in wrapped_lines
        .iter()
        .flat_map(|wrapped_line| wrapped_line.original.iter().zip(&wrapped_line.modified))
        .skip(view.view_lines.start as usize)
        .take(view.view_lines.len())
        .enumerate()
    {
        let view_line_index = view.view_lines.start + offset as u32;
        let terminal_line = match modified {
            TerminalLine::SourceCode { .. } => modified,
            TerminalLine::Filler => match original {
                TerminalLine::SourceCode { .. } => original,
                TerminalLine::Filler => continue,
            },
        };
        let TerminalLine::SourceCode {
            source_line: line_number,
            ..
        } = terminal_line
        else {
            continue;
        };
        let line_number = *line_number;
        let version = if matches!(modified, TerminalLine::SourceCode { .. }) {
            DiffVersion::Modified
        } else {
            DiffVersion::Original
        };
        let original_number = match original {
            TerminalLine::SourceCode { source_line, bytes } if bytes.start == 0 => {
                Some(*source_line)
            }
            _ => None,
        };
        let modified_number = match modified {
            TerminalLine::SourceCode { source_line, bytes } if bytes.start == 0 => {
                Some(*source_line)
            }
            _ => None,
        };
        let decorations = alignment.decorations(version, line_number);
        let code_styles = code_text::styles_for_diff(theme, version, decorations.line_background);
        let gutter_style = gutter::style_for_diff(theme, version, decorations.gutter_background);
        let text = alignment.line(version, line_number).unwrap_or("");
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
        let syntax_spans = syntax
            .map(|store| SyntaxService::line_spans(store, &diff.file, version, line_number))
            .unwrap_or_default();
        let (text, changed_ranges, fill_from, empty_markers, syntax_spans) =
            code_text::prepare_code_text_inputs(
                text,
                terminal_line,
                &changed_ranges,
                fill_from,
                &decorations.empty_markers,
                &syntax_spans,
            );

        rows.push(rsx! {
            Row {
                key: view_line_index,
                layout: Layout { basis: Basis::Length(1), shrink: 0, ..Default::default() },
                ..,
                Gutter {
                    key: 0u32,
                    number: original_number,
                    style: gutter_style,
                    blank: gutter_style,
                    width: original_gutter_width,
                }
                Gutter {
                    key: 1u32,
                    number: modified_number,
                    style: gutter_style,
                    blank: gutter_style,
                    width: modified_gutter_width,
                }
                CodeText {
                    key: 2u32,
                    text: text,
                    first_cell: horizontal.first_cell(version),
                    diff: changed_ranges,
                    fill_from: fill_from,
                    empty_markers: empty_markers,
                    syntax: syntax_spans,
                    unchanged_style: code_styles.unchanged,
                    changed_style: code_styles.changed,
                    selection: None,
                }
            }
        });
    }

    rsx! {
        Column {
            ref: Some(node_ref),
            focusable: true,
            auto_focus: *auto_focus,
            listeners: listeners,
            layout: Layout { grow: 1, fill: Some(theme.normal), ..Default::default() },
            ..,
            { rows }
        }
    }
}
