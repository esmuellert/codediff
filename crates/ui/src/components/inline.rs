//! One full-width column showing original and modified lines in sequence.

use std::ops::Range;
use std::rc::Rc;

use align::DiffVersion;
use file_types::DiffType;
use loom::{
    Basis, Column, ColumnProps, Layout, Node, Ref, Row, RowProps, Scope, component, rsx,
    use_context, use_memo,
};

use super::code_text::{self, CodeText, CodeTextProps, longest_line_cells};
use super::context::Ui;
use super::diff_viewer::{ViewState, find_view_line_index, first_view_line};
use super::gutter::{self, Gutter, GutterProps, width_for_line_count};
use crate::hooks::use_diff_viewer_navigation::{HorizontalDimensions, use_diff_viewer_navigation};
use crate::hooks::use_horizontal_scroll::use_horizontal_scroll;
use crate::hooks::use_scroll::use_scroll;
use crate::hooks::use_syntax::use_syntax;
use crate::services::syntax::SyntaxService;

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
    let view_line_count = alignment.view_line_count(DiffType::Inline);
    let view_state_ref = *view_state;
    let current_view_state = *view_state_ref.current();
    let content_id = Rc::as_ptr(content) as usize;
    let maximum_line_cells = use_memo(scope, content_id, || {
        longest_line_cells(alignment.lines(DiffVersion::Original))
            .max(longest_line_cells(alignment.lines(DiffVersion::Modified)))
    });
    let initial_top = current_view_state
        .first_view_line
        .and_then(|line| find_view_line_index(alignment, DiffType::Inline, line))
        .unwrap_or(0);
    let (view, vertical_handle) = use_scroll(scope, view_line_count, initial_top);
    let horizontal_limits = HorizontalDimensions::Inline {
        longest_line_cells: *maximum_line_cells,
        original_gutter_cells: original_gutter_width,
        modified_gutter_cells: modified_gutter_width,
    }
    .limits(view.width);
    let (horizontal_view, horizontal_handle) = use_horizontal_scroll(
        scope,
        horizontal_limits.maximum_first_cell(),
        current_view_state.first_cell,
    );
    let horizontal = horizontal_limits.view(horizontal_view.first_cell);
    *view_state_ref.current() = ViewState {
        first_view_line: (view.top > 0)
            .then(|| first_view_line(alignment, DiffType::Inline, view.top))
            .flatten(),
        first_cell: horizontal.requested_first_cell,
    };
    let listeners = use_diff_viewer_navigation(vertical_handle, horizontal_handle);
    let view_lines: Vec<align::ViewLine> = alignment
        .view_lines_from(DiffType::Inline, view.view_lines.start)
        .take(view.view_lines.len())
        .collect();
    let syntax = use_syntax(
        scope,
        ctx.syntax_service.as_ref().map(Rc::clone),
        Rc::clone(content),
        DiffType::Inline,
        view.view_lines.clone(),
    );
    let syntax = syntax.as_deref();

    let mut rows = Vec::with_capacity(view_lines.len());
    for (offset, view_line) in view_lines.iter().enumerate() {
        let view_line_index = view.view_lines.start + offset as u32;
        let (version, line_number) = match (view_line.modified.line(), view_line.original.line()) {
            (Some(line_number), _) => (DiffVersion::Modified, line_number),
            (None, Some(line_number)) => (DiffVersion::Original, line_number),
            (None, None) => continue,
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

        rows.push(rsx! {
            Row {
                key: view_line_index,
                layout: Layout { basis: Basis::Length(1), shrink: 0, ..Default::default() },
                ..,
                Gutter {
                    key: 0u32,
                    number: view_line.original.line(),
                    style: gutter_style,
                    blank: gutter_style,
                    width: original_gutter_width,
                }
                Gutter {
                    key: 1u32,
                    number: view_line.modified.line(),
                    style: gutter_style,
                    blank: gutter_style,
                    width: modified_gutter_width,
                }
                CodeText {
                    key: 2u32,
                    text: Rc::from(text),
                    first_cell: horizontal.first_cell(version),
                    diff: Rc::from(changed_ranges.as_slice()),
                    fill_from: fill_from,
                    empty_markers: Rc::from(decorations.empty_markers.as_slice()),
                    syntax: Rc::from(
                        syntax
                            .map(|store| SyntaxService::line_spans(store, &diff.file, version, line_number))
                            .unwrap_or_default()
                            .as_slice()
                    ),
                    unchanged_style: code_styles.unchanged,
                    changed_style: code_styles.changed,
                    selection: None,
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
