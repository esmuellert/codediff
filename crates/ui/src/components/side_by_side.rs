//! Two columns showing both versions of a file, paired line by line.

use std::rc::Rc;

use align::{Alignment, DiffVersion};
use file_types::DiffType;
use loom::{
    Basis, Column, ColumnProps, Divider, DividerProps, Layout, Node, Row, RowProps, Scope, Scroll,
    ScrollProps, component, rsx, use_context,
};

use super::code_text::{
    self, CodeText, CodeTextProps, horizontal_scroll_extent, longest_line_cells,
};
use super::context::Ui;
use super::diff_viewer_container::DiffVisibleRange;
use super::filler::Filler;
use super::fold::{FoldMarker, is_fold_marker};
use super::gutter::{self, Gutter, GutterProps, width_for_line_count};
use crate::hooks::use_syntax::use_syntax;
use crate::services::syntax::SyntaxService;
use crate::view::compact::view_lines as compact_view_lines;
use crate::view::terminal_lines::{
    TerminalLine, WrappedViewLine, terminal_line_count, terminal_view_lines,
};

pub(crate) struct SideBySideLayout {
    pub(crate) wrapped_lines: Rc<Vec<WrappedViewLine>>,
    pub(crate) original_gutter_width: u16,
    pub(crate) modified_gutter_width: u16,
    pub(crate) original_width: u16,
    pub(crate) modified_width: u16,
    pub(crate) original_horizontal_scroll_extent: u32,
    pub(crate) modified_horizontal_scroll_extent: u32,
    pub(crate) terminal_line_count: u32,
}

pub(crate) fn layout_side_by_side(
    alignment: &Alignment,
    width: u16,
    wrap: bool,
    compact: bool,
) -> SideBySideLayout {
    let original_line_count = alignment.lines(DiffVersion::Original).len() as u32;
    let modified_line_count = alignment.lines(DiffVersion::Modified).len() as u32;
    let original_gutter_width = width_for_line_count(original_line_count);
    let modified_gutter_width = width_for_line_count(modified_line_count);
    let text_width = width
        .saturating_sub(1)
        .saturating_sub(original_gutter_width)
        .saturating_sub(modified_gutter_width);
    let original_width = text_width.div_ceil(2);
    let modified_width = text_width / 2;
    let wrapped_lines = terminal_view_lines(
        alignment,
        DiffType::SideBySide,
        compact_view_lines(alignment, DiffType::SideBySide, compact),
        original_width,
        modified_width,
        wrap,
    );
    let (original_longest, modified_longest) = if wrap || compact {
        (u32::from(original_width), u32::from(modified_width))
    } else {
        (
            longest_line_cells(alignment.lines(DiffVersion::Original)),
            longest_line_cells(alignment.lines(DiffVersion::Modified)),
        )
    };
    let terminal_line_count = terminal_line_count(&wrapped_lines);
    SideBySideLayout {
        wrapped_lines: Rc::new(wrapped_lines),
        original_gutter_width,
        modified_gutter_width,
        original_width,
        modified_width,
        original_horizontal_scroll_extent: horizontal_scroll_extent(
            original_longest,
            original_width,
        ),
        modified_horizontal_scroll_extent: horizontal_scroll_extent(
            modified_longest,
            modified_width,
        ),
        terminal_line_count,
    }
}

#[component]
pub(crate) fn SideBySide(
    scope: &mut Scope,
    content: Rc<pipeline::diff::DiffContent>,
    wrapped_lines: Rc<Vec<WrappedViewLine>>,
    viewport: DiffVisibleRange,
    original_gutter_width: u16,
    modified_gutter_width: u16,
    original_width: u16,
    modified_width: u16,
    original_horizontal_scroll_extent: u32,
    modified_horizontal_scroll_extent: u32,
    auto_focus: bool,
) -> Node {
    let original_gutter_width = *original_gutter_width;
    let modified_gutter_width = *modified_gutter_width;
    let original_width = *original_width;
    let modified_width = *modified_width;
    let original_horizontal_scroll_extent = *original_horizontal_scroll_extent;
    let modified_horizontal_scroll_extent = *modified_horizontal_scroll_extent;
    let viewport = viewport.clone();
    let ctx = use_context::<Ui>(scope);
    let theme = &ctx.theme;
    let pipeline::diff::DiffContent::Diff(diff) = content.as_ref() else {
        unreachable!("DiffViewer sends diffs to SideBySide")
    };
    let wrapped_lines = wrapped_lines.as_ref();
    let visible_terminal_line_count = viewport
        .visible_terminal_lines
        .end
        .saturating_sub(viewport.visible_terminal_lines.start);
    let syntax = use_syntax(
        scope,
        ctx.syntax_service.as_ref().map(Rc::clone),
        Rc::clone(content),
        DiffType::SideBySide,
        wrapped_lines,
    );
    let syntax = syntax.as_deref();
    let divider_style = theme.normal.patch(theme.divider);

    let mut original_gutters = Vec::with_capacity(visible_terminal_line_count);
    let mut original_code_lines = Vec::with_capacity(visible_terminal_line_count);
    let mut dividers = Vec::with_capacity(visible_terminal_line_count);
    let mut modified_gutters = Vec::with_capacity(visible_terminal_line_count);
    let mut modified_code_lines = Vec::with_capacity(visible_terminal_line_count);
    for (offset, (original, modified)) in wrapped_lines
        .iter()
        .flat_map(WrappedViewLine::terminal_line_pairs)
        .skip(viewport.visible_terminal_lines.start)
        .take(visible_terminal_line_count)
        .enumerate()
    {
        let view_line = viewport.visible_terminal_lines.start + offset;
        let folded = is_fold_marker(original, modified);
        let (original_gutter, original_code) = if folded {
            let blank = theme.normal;
            (
                rsx! {
                    Gutter {
                        key: 0u32,
                        number: None,
                        style: blank,
                        blank: blank,
                        width: original_gutter_width,
                    }
                },
                rsx! { FoldMarker { key: 0u32 } },
            )
        } else {
            make_side(
                DiffVersion::Original,
                original,
                original_gutter_width,
                diff,
                theme,
                syntax,
            )
        };
        let (modified_gutter, modified_code) = if folded {
            let blank = theme.normal;
            (
                rsx! {
                    Gutter {
                        key: 0u32,
                        number: None,
                        style: blank,
                        blank: blank,
                        width: modified_gutter_width,
                    }
                },
                rsx! { FoldMarker { key: 0u32 } },
            )
        } else {
            make_side(
                DiffVersion::Modified,
                modified,
                modified_gutter_width,
                diff,
                theme,
                syntax,
            )
        };

        original_gutters.push(rsx! {
            Row {
                key: view_line,
                layout: Layout { basis: Basis::Length(1), shrink: 0, ..Default::default() },
                ..,
                { original_gutter }
            }
        });
        original_code_lines.push(rsx! {
            Row {
                key: view_line,
                layout: Layout { basis: Basis::Length(1), shrink: 0, ..Default::default() },
                ..,
                { original_code }
            }
        });
        dividers.push(rsx! {
            Row {
                key: view_line,
                layout: Layout { basis: Basis::Length(1), shrink: 0, ..Default::default() },
                ..,
                Divider {
                    key: 0u32,
                    layout: Layout { basis: Basis::Length(1), shrink: 0, ..Default::default() },
                    symbol: "│",
                    style: divider_style,
                    ..
                }
            }
        });
        modified_gutters.push(rsx! {
            Row {
                key: view_line,
                layout: Layout { basis: Basis::Length(1), shrink: 0, ..Default::default() },
                ..,
                { modified_gutter }
            }
        });
        modified_code_lines.push(rsx! {
            Row {
                key: view_line,
                layout: Layout { basis: Basis::Length(1), shrink: 0, ..Default::default() },
                ..,
                { modified_code }
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
                Scroll {
                    view: viewport.horizontal_view.clone(),
                    handle: None,
                    horizontal: true,
                    vertical: false,
                    wheel_step: 0,
                    write_metrics: false,
                    content_width: Some(
                        original_horizontal_scroll_extent
                            .saturating_add(u32::from(original_width)),
                    ),
                    layout: Layout { grow: 1, shrink: 0, fill: Some(theme.normal), ..Default::default() },
                    ..,
                    Column {
                        layout: Layout { grow: 1, fill: Some(theme.normal), ..Default::default() },
                        ..,
                        { original_code_lines }
                    }
                }
                Column {
                    layout: Layout { basis: Basis::Length(1), shrink: 0, ..Default::default() },
                    ..,
                    { dividers }
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
                    content_width: Some(
                        modified_horizontal_scroll_extent
                            .saturating_add(u32::from(modified_width)),
                    ),
                    layout: Layout { grow: 1, shrink: 0, fill: Some(theme.normal), ..Default::default() },
                    ..,
                    Column {
                        layout: Layout { grow: 1, fill: Some(theme.normal), ..Default::default() },
                        ..,
                        { modified_code_lines }
                    }
                }
            }
        }
    }
}

fn make_side(
    version: DiffVersion,
    line: &TerminalLine,
    gutter_width: u16,
    diff: &pipeline::diff::Diff,
    theme: &crate::theme::Theme,
    syntax: Option<&syntax::Store>,
) -> (Node, Node) {
    let alignment = &diff.alignment;
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
            let gutter_number = line.gutter_number();
            let text = alignment.line(version, line_number).unwrap_or("");
            let syntax_spans = syntax
                .map(|store| SyntaxService::line_spans(store, &diff.file, version, line_number))
                .unwrap_or_default();
            let (text, changed_ranges, fill_from, empty_markers, syntax_spans) =
                code_text::prepare_code_text_inputs_from_decorations(
                    text,
                    line,
                    &decorations,
                    &syntax_spans,
                );
            (
                rsx! {
                    Gutter {
                        key: 0u32,
                        number: gutter_number,
                        style: gutter_style,
                        blank: gutter_style,
                        width: gutter_width,
                    }
                },
                rsx! {
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
                },
            )
        }
        TerminalLine::Filler => {
            let blank = theme.normal.patch(theme.filler);
            (
                rsx! {
                    Gutter {
                        key: 0u32,
                        number: None,
                        style: blank,
                        blank: blank,
                        width: gutter_width,
                    }
                },
                rsx! { Filler { key: 0u32 } },
            )
        }
    }
}
