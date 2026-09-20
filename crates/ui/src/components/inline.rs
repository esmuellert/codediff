//! One full-width column showing original and modified lines in sequence.

use std::rc::Rc;

use align::DiffVersion;
use file_types::DiffType;
use loom::{
    Basis, Column, ColumnProps, Layout, Node, Ref, Row, RowProps, Scope, Scroll, ScrollOffset,
    ScrollProps, component, rsx, use_context, use_layout_effect, use_measure, use_memo, use_scroll,
};

use super::code_text::{self, CodeText, CodeTextProps, longest_line_cells};
use super::context::Ui;
use super::diff_viewer::ViewState;
use super::fold::{FoldMarker, is_fold_marker};
use super::gutter::{self, Gutter, GutterProps, width_for_line_count};
use crate::hooks::use_diff_viewer_navigation::use_diff_viewer_navigation;
use crate::hooks::use_syntax::use_syntax;
use crate::services::syntax::SyntaxService;
use crate::view::compact::view_lines as compact_view_lines;
use crate::view::terminal_lines::{
    TerminalLine, WrappedViewLine, find_terminal_line_index, terminal_view_lines,
};

#[component]
pub fn Inline(
    scope: &mut Scope,
    content: Rc<pipeline::diff::DiffContent>,
    view_state: Ref<ViewState>,
    wrap: bool,
    compact: bool,
    auto_focus: bool,
) -> Node {
    let wrap = *wrap;
    let compact = *compact;
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
    let wrapped_lines = use_memo(scope, (content_id, code_width, wrap, compact), || {
        terminal_view_lines(
            alignment,
            DiffType::Inline,
            compact_view_lines(alignment, DiffType::Inline, compact),
            code_width,
            code_width,
            wrap,
        )
    });
    let maximum_line_cells = use_memo(scope, (content_id, code_width, wrap, compact), || {
        if wrap || compact {
            u32::from(code_width)
        } else {
            longest_line_cells(alignment.lines(DiffVersion::Original))
                .max(longest_line_cells(alignment.lines(DiffVersion::Modified)))
        }
    });
    let initial_top = current_view_state
        .first_terminal_line
        .as_ref()
        .and_then(|line| find_terminal_line_index(&wrapped_lines, line))
        .unwrap_or(0);
    let (vertical_view, vertical_handle) = use_scroll(scope, || ScrollOffset {
        x: 0,
        y: initial_top,
    });
    let (horizontal_view, horizontal_handle) = use_scroll(scope, || ScrollOffset {
        x: current_view_state.first_cell,
        y: 0,
    });
    let restore_vertical = vertical_handle.clone();
    let restore_horizontal = horizontal_handle.clone();
    use_layout_effect(scope, (content_id, code_width, wrap, compact), move || {
        restore_vertical.scroll_to(ScrollOffset {
            x: 0,
            y: initial_top,
        });
        restore_horizontal.scroll_to(ScrollOffset {
            x: current_view_state.first_cell,
            y: 0,
        });
    });
    let total_terminal_lines = wrapped_lines
        .iter()
        .flat_map(WrappedViewLine::terminal_line_pairs)
        .count() as u32;
    let visible_start = vertical_view
        .clamped_offset()
        .y
        .min(total_terminal_lines.saturating_sub(1));
    let visible_count = u32::from(size.height).saturating_add(4);
    let vertical_position = vertical_view.requested_offset().y;
    let horizontal_position = horizontal_view.requested_offset().x;
    if size.width > 0 && size.height > 0 {
        *view_state_ref.current() = ViewState {
            first_terminal_line: (vertical_position > 0)
                .then(|| {
                    wrapped_lines
                        .iter()
                        .flat_map(WrappedViewLine::terminal_line_pairs)
                        .nth(vertical_position as usize)
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
            first_cell: horizontal_position,
        };
    }
    let listeners = use_diff_viewer_navigation(
        scope,
        vertical_handle.clone(),
        horizontal_handle.clone(),
        None,
    );
    let syntax = use_syntax(
        scope,
        ctx.syntax_service.as_ref().map(Rc::clone),
        Rc::clone(content),
        DiffType::Inline,
        &wrapped_lines,
    );
    let syntax = syntax.as_deref();

    let mut original_gutters = Vec::with_capacity(visible_count as usize);
    let mut modified_gutters = Vec::with_capacity(visible_count as usize);
    let mut code_rows = Vec::with_capacity(visible_count as usize);
    for (offset, (original, modified)) in wrapped_lines
        .iter()
        .flat_map(WrappedViewLine::terminal_line_pairs)
        .skip(visible_start as usize)
        .take(visible_count as usize)
        .enumerate()
    {
        let view_line_index = visible_start + offset as u32;
        if is_fold_marker(original, modified) {
            let blank = theme.normal;
            original_gutters.push(rsx! {
                Row {
                    key: view_line_index,
                    layout: Layout { basis: Basis::Length(1), shrink: 0, ..Default::default() },
                    ..,
                    Gutter {
                        key: 0u32,
                        number: None,
                        style: blank,
                        blank: blank,
                        width: original_gutter_width,
                    }
                }
            });
            modified_gutters.push(rsx! {
                Row {
                    key: view_line_index,
                    layout: Layout { basis: Basis::Length(1), shrink: 0, ..Default::default() },
                    ..,
                    Gutter {
                        key: 0u32,
                        number: None,
                        style: blank,
                        blank: blank,
                        width: modified_gutter_width,
                    }
                }
            });
            code_rows.push(rsx! {
                Row {
                    key: view_line_index,
                    layout: Layout { basis: Basis::Length(1), shrink: 0, ..Default::default() },
                    ..,
                    FoldMarker { key: 0u32 }
                }
            });
            continue;
        }

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
        let original_number = original.gutter_number();
        let modified_number = modified.gutter_number();
        let decorations = alignment.decorations(version, line_number);
        let code_styles = code_text::styles_for_diff(theme, version, decorations.line_background);
        let gutter_style = gutter::style_for_diff(theme, version, decorations.gutter_background);
        let text = alignment.line(version, line_number).unwrap_or("");
        let syntax_spans = syntax
            .map(|store| SyntaxService::line_spans(store, &diff.file, version, line_number))
            .unwrap_or_default();
        let (text, changed_ranges, fill_from, empty_markers, syntax_spans) =
            code_text::prepare_code_text_inputs_from_decorations(
                text,
                terminal_line,
                &decorations,
                &syntax_spans,
            );

        original_gutters.push(rsx! {
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
            }
        });
        modified_gutters.push(rsx! {
            Row {
                key: view_line_index,
                layout: Layout { basis: Basis::Length(1), shrink: 0, ..Default::default() },
                ..,
                Gutter {
                    key: 0u32,
                    number: modified_number,
                    style: gutter_style,
                    blank: gutter_style,
                    width: modified_gutter_width,
                }
            }
        });
        code_rows.push(rsx! {
            Row {
                key: view_line_index,
                layout: Layout { basis: Basis::Length(1), shrink: 0, ..Default::default() },
                ..,
                CodeText {
                    key: 0u32,
                    text: text,
                    first_cell: 0,
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
            Scroll {
                view: vertical_view,
                handle: Some(vertical_handle),
                horizontal: false,
                vertical: true,
                wheel_step: 3,
                content_height: Some(total_terminal_lines),
                content_offset: ScrollOffset { x: 0, y: visible_start },
                layout: Layout { grow: 1, fill: Some(theme.normal), ..Default::default() },
                ..,
                Row {
                    layout: Layout { grow: 1, ..Default::default() },
                    ..,
                    Column {
                        layout: Layout { basis: Basis::Length(original_gutter_width), shrink: 0, ..Default::default() },
                        ..,
                        { original_gutters }
                    }
                    Column {
                        layout: Layout { basis: Basis::Length(modified_gutter_width), shrink: 0, ..Default::default() },
                        ..,
                        { modified_gutters }
                    }
                    Scroll {
                        view: horizontal_view,
                        handle: Some(horizontal_handle),
                        horizontal: true,
                        vertical: false,
                        wheel_step: 3,
                        content_width: Some(*maximum_line_cells),
                        layout: Layout { grow: 1, shrink: 0, fill: Some(theme.normal), ..Default::default() },
                        ..,
                        Column {
                            layout: Layout { grow: 1, fill: Some(theme.normal), ..Default::default() },
                            ..,
                            { code_rows }
                        }
                    }
                }
            }
        }
    }
}
