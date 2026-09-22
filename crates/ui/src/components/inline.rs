//! One full-width column showing original and modified lines in sequence.

use std::rc::Rc;

use align::{Alignment, DiffVersion};
use file_types::DiffType;
use loom::{
    Basis, Column, ColumnProps, Layout, Node, Row, RowProps, Scope, Scroll, ScrollProps, component,
    rsx, use_context,
};

use super::code_text::{
    self, CodeText, CodeTextProps, horizontal_scroll_extent, longest_line_cells,
};
use super::context::Ui;
use super::diff_viewer_container::DiffVisibleRange;
use super::fold::{FoldMarker, is_fold_marker};
use super::gutter::{self, Gutter, GutterProps, width_for_line_count};
use crate::hooks::use_syntax::use_syntax;
use crate::services::syntax::SyntaxService;
use crate::view::compact::view_lines as compact_view_lines;
use crate::view::terminal_lines::{
    TerminalLine, WrappedViewLine, terminal_line_count, terminal_view_lines,
};

pub(crate) struct InlineLayout {
    pub(crate) wrapped_lines: Rc<Vec<WrappedViewLine>>,
    pub(crate) original_gutter_width: u16,
    pub(crate) modified_gutter_width: u16,
    pub(crate) code_width: u16,
    pub(crate) horizontal_scroll_extent: u32,
    pub(crate) terminal_line_count: u32,
}

pub(crate) fn layout_inline(
    alignment: &Alignment,
    width: u16,
    wrap: bool,
    compact: bool,
) -> InlineLayout {
    let original_line_count = alignment.lines(DiffVersion::Original).len() as u32;
    let modified_line_count = alignment.lines(DiffVersion::Modified).len() as u32;
    let original_gutter_width = width_for_line_count(original_line_count);
    let modified_gutter_width = width_for_line_count(modified_line_count);
    let code_width = width
        .saturating_sub(original_gutter_width)
        .saturating_sub(modified_gutter_width);
    let wrapped_lines = terminal_view_lines(
        alignment,
        DiffType::Inline,
        compact_view_lines(alignment, DiffType::Inline, compact),
        code_width,
        code_width,
        wrap,
    );
    let maximum_line_cells = if wrap || compact {
        u32::from(code_width)
    } else {
        longest_line_cells(alignment.lines(DiffVersion::Original))
            .max(longest_line_cells(alignment.lines(DiffVersion::Modified)))
    };
    let terminal_line_count = terminal_line_count(&wrapped_lines);
    InlineLayout {
        wrapped_lines: Rc::new(wrapped_lines),
        original_gutter_width,
        modified_gutter_width,
        code_width,
        horizontal_scroll_extent: horizontal_scroll_extent(maximum_line_cells, code_width),
        terminal_line_count,
    }
}

#[component]
pub(crate) fn Inline(
    scope: &mut Scope,
    content: Rc<pipeline::diff::DiffContent>,
    wrapped_lines: Rc<Vec<WrappedViewLine>>,
    viewport: DiffVisibleRange,
    original_gutter_width: u16,
    modified_gutter_width: u16,
    code_width: u16,
    horizontal_scroll_extent: u32,
    auto_focus: bool,
) -> Node {
    let original_gutter_width = *original_gutter_width;
    let modified_gutter_width = *modified_gutter_width;
    let code_width = *code_width;
    let horizontal_scroll_extent = *horizontal_scroll_extent;
    let viewport = viewport.clone();
    let ctx = use_context::<Ui>(scope);
    let theme = &ctx.theme;
    let pipeline::diff::DiffContent::Diff(diff) = content.as_ref() else {
        unreachable!("DiffViewer sends diffs to Inline")
    };
    let alignment = &diff.alignment;
    let wrapped_lines = wrapped_lines.as_ref();
    let visible_terminal_line_count = viewport
        .visible_terminal_lines
        .end
        .saturating_sub(viewport.visible_terminal_lines.start);
    let syntax = use_syntax(
        scope,
        ctx.syntax_service.as_ref().map(Rc::clone),
        Rc::clone(content),
        DiffType::Inline,
        wrapped_lines,
    );
    let syntax = syntax.as_deref();

    let mut original_gutters = Vec::with_capacity(visible_terminal_line_count);
    let mut modified_gutters = Vec::with_capacity(visible_terminal_line_count);
    let mut code_lines = Vec::with_capacity(visible_terminal_line_count);
    for (offset, (original, modified)) in wrapped_lines
        .iter()
        .flat_map(WrappedViewLine::terminal_line_pairs)
        .skip(viewport.visible_terminal_lines.start)
        .take(visible_terminal_line_count)
        .enumerate()
    {
        let view_line_index = viewport.visible_terminal_lines.start + offset;
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
            code_lines.push(rsx! {
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
        code_lines.push(rsx! {
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
            focusable: true,
            auto_focus: *auto_focus,
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
                    view: viewport.horizontal_view,
                    handle: None,
                    horizontal: true,
                    vertical: false,
                    wheel_step: 0,
                    write_metrics: false,
                    content_width: Some(horizontal_scroll_extent.saturating_add(u32::from(code_width))),
                    layout: Layout { grow: 1, shrink: 0, fill: Some(theme.normal), ..Default::default() },
                    ..,
                    Column {
                        layout: Layout { grow: 1, fill: Some(theme.normal), ..Default::default() },
                        ..,
                        { code_lines }
                    }
                }
            }
        }
    }
}
