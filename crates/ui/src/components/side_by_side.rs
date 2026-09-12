//! Two columns showing both versions of a file, paired line by line.

use std::ops::Range;
use std::rc::Rc;

use align::DiffVersion;
use file_types::DiffType;
use loom::{
    Basis, Column, ColumnProps, Divider, DividerProps, Layout, Node, Ref, Row, RowProps, Scope,
    component, rsx, use_context, use_memo,
};

use super::code_text::{self, CodeText, CodeTextProps, longest_line_cells};
use super::context::Ui;
use super::diff_viewer::ViewState;
use super::filler::Filler;
use super::gutter::{self, Gutter, GutterProps, width_for_line_count};
use super::wrap::{TerminalLine, WrappedViewLine, find_terminal_line_index, unwrapped_view_lines};
use crate::hooks::use_diff_viewer_navigation::{HorizontalDimensions, use_diff_viewer_navigation};
use crate::hooks::use_horizontal_scroll::use_horizontal_scroll;
use crate::hooks::use_scroll::use_scroll;
use crate::hooks::use_syntax::use_syntax;
use crate::services::syntax::SyntaxService;

#[component]
pub fn SideBySide(
    scope: &mut Scope,
    content: Rc<pipeline::diff::DiffContent>,
    view_state: Ref<ViewState>,
    auto_focus: bool,
) -> Node {
    let ctx = use_context::<Ui>(scope);
    let theme = &ctx.theme;
    let pipeline::diff::DiffContent::Diff(diff) = content.as_ref() else {
        unreachable!("DiffViewer sends diffs to SideBySide")
    };
    let alignment = &diff.alignment;
    let view_state_ref = *view_state;
    let current_view_state = view_state_ref.current().clone();
    let original_line_count = alignment.lines(DiffVersion::Original).len() as u32;
    let modified_line_count = alignment.lines(DiffVersion::Modified).len() as u32;
    let original_gutter_width = width_for_line_count(original_line_count);
    let modified_gutter_width = width_for_line_count(modified_line_count);
    let content_id = Rc::as_ptr(content) as usize;
    let wrapped_lines = use_memo(scope, content_id, || {
        unwrapped_view_lines(alignment, DiffType::SideBySide)
    });
    let maximum_line_cells = use_memo(scope, content_id, || {
        (
            longest_line_cells(alignment.lines(DiffVersion::Original)),
            longest_line_cells(alignment.lines(DiffVersion::Modified)),
        )
    });
    let initial_top = current_view_state
        .first_terminal_line
        .as_ref()
        .and_then(|line| find_terminal_line_index(&wrapped_lines, line))
        .unwrap_or(0);
    let (view, vertical_handle) = use_scroll(scope, wrapped_lines.len() as u32, initial_top);
    let horizontal_limits = HorizontalDimensions::SideBySide {
        original_longest_line_cells: maximum_line_cells.0,
        modified_longest_line_cells: maximum_line_cells.1,
        original_gutter_cells: original_gutter_width,
        modified_gutter_cells: modified_gutter_width,
        divider_cells: 1,
    }
    .limits(view.width);
    let (horizontal_view, horizontal_handle) = use_horizontal_scroll(
        scope,
        horizontal_limits.maximum_first_cell(),
        current_view_state.first_cell,
    );
    let horizontal = horizontal_limits.view(horizontal_view.first_cell);
    *view_state_ref.current() = ViewState {
        first_terminal_line: (view.top > 0)
            .then(|| wrapped_lines.get(view.top as usize))
            .flatten()
            .and_then(WrappedViewLine::selected_terminal_line),
        first_cell: horizontal.requested_first_cell,
    };
    let listeners = use_diff_viewer_navigation(vertical_handle, horizontal_handle);

    let visible_wrapped_lines =
        &wrapped_lines[view.view_lines.start as usize..view.view_lines.end as usize];

    let syntax = use_syntax(
        scope,
        ctx.syntax_service.as_ref().map(Rc::clone),
        Rc::clone(content),
        DiffType::SideBySide,
        visible_wrapped_lines,
    );
    let syntax = syntax.as_deref();
    let divider_style = theme.normal.patch(theme.divider);

    let mut rows: Vec<Node> = Vec::with_capacity(visible_wrapped_lines.len());
    for (offset, (original, modified)) in visible_wrapped_lines
        .iter()
        .flat_map(|wrapped_line| wrapped_line.original.iter().zip(&wrapped_line.modified))
        .enumerate()
    {
        let view_line = view.view_lines.start + offset as u32;
        let make_side =
            |version: DiffVersion, line: &TerminalLine, gutter_width: u16| -> Vec<Node> {
                match line {
                    TerminalLine::SourceCode {
                        source_line: line_number,
                        ..
                    } => {
                        let line_number = *line_number;
                        let decorations = alignment.decorations(version, line_number);
                        let code_styles =
                            code_text::styles_for_diff(theme, version, decorations.line_background);
                        let gutter_style =
                            gutter::style_for_diff(theme, version, decorations.gutter_background);
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
                            .map(|store| {
                                SyntaxService::line_spans(store, &diff.file, version, line_number)
                            })
                            .unwrap_or_default();
                        let (text, changed_ranges, fill_from, empty_markers, syntax_spans) =
                            code_text::prepare_code_text_inputs(
                                text,
                                line,
                                &changed_ranges,
                                fill_from,
                                &decorations.empty_markers,
                                &syntax_spans,
                            );
                        vec![
                            rsx! {
                                Gutter {
                                    key: 0u32,
                                    number: Some(line_number),
                                    style: gutter_style,
                                    blank: gutter_style,
                                    width: gutter_width,
                                }
                            },
                            rsx! {
                                CodeText {
                                    key: 1u32,
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
                            },
                        ]
                    }
                    TerminalLine::Filler => {
                        let blank = theme.normal.patch(theme.filler);
                        vec![
                            rsx! {
                                Gutter {
                                    key: 0u32,
                                    number: None,
                                    style: blank,
                                    blank: blank,
                                    width: gutter_width,
                                }
                            },
                            rsx! { Filler { key: 1u32 } },
                        ]
                    }
                }
            };

        let original_nodes = make_side(DiffVersion::Original, original, original_gutter_width);
        let modified_nodes = make_side(DiffVersion::Modified, modified, modified_gutter_width);

        rows.push(rsx! {
            Row {
                key: view_line,
                layout: Layout { basis: Basis::Length(1), shrink: 0, ..Default::default() },
                ..,
                Row {
                    key: 0u32,
                    layout: Layout { grow: 1, ..Default::default() },
                    ..,
                    { original_nodes }
                }
                Divider {
                    key: 1u32,
                    layout: Layout { basis: Basis::Length(1), shrink: 0, ..Default::default() },
                    symbol: "│",
                    style: divider_style,
                    ..
                }
                Row {
                    key: 2u32,
                    layout: Layout { grow: 1, ..Default::default() },
                    ..,
                    { modified_nodes }
                }
            }
        });
    }

    rsx! {
        Column {
            ref: Some(view.node_ref),
            focusable: true,
            auto_focus: *auto_focus,
            listeners: listeners,
            layout: Layout { grow: 1, fill: Some(theme.normal), ..Default::default() },
            ..,
            { rows }
        }
    }
}
