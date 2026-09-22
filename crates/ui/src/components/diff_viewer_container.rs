use std::ops::Range;
use std::rc::Rc;

use file_types::DiffType;
use loom::{
    Column, ColumnProps, Layout, Node, Ref, Scope, Scroll, ScrollOffset, ScrollProps, ScrollView,
    component, rsx, use_context, use_layout_effect, use_measure, use_memo, use_scroll,
};
use pipeline::diff::DiffContent;

use super::context::Ui;
use super::diff_viewer::ViewState;
use super::inline::{Inline, InlineLayout, InlineProps, layout_inline};
use super::side_by_side::{SideBySide, SideBySideLayout, SideBySideProps, layout_side_by_side};
use super::single_file::{SingleFile, SingleFileLayout, SingleFileProps, layout_single_file};
use super::welcome::Welcome;
use crate::hooks::use_diff_viewer_navigation::use_diff_viewer_navigation;
use crate::view::terminal_lines::{TerminalLine, WrappedViewLine, find_terminal_line_index};

/// The part of the Loom viewport that a diff renderer is allowed to read.
#[derive(Clone)]
pub(crate) struct DiffVisibleRange {
    pub(crate) visible_rows: Range<usize>,
    pub(crate) horizontal_view: ScrollView,
}

enum ViewLayout {
    Empty,
    Inline(InlineLayout),
    SideBySide(SideBySideLayout),
    SingleFile(SingleFileLayout),
}

impl ViewLayout {
    fn wrapped_lines(&self) -> Option<&Rc<Vec<WrappedViewLine>>> {
        match self {
            Self::Empty => None,
            Self::Inline(layout) => Some(&layout.wrapped_lines),
            Self::SideBySide(layout) => Some(&layout.wrapped_lines),
            Self::SingleFile(layout) => Some(&layout.wrapped_lines),
        }
    }

    fn row_count(&self) -> u32 {
        match self {
            Self::Empty => 0,
            Self::Inline(layout) => layout.row_count,
            Self::SideBySide(layout) => layout.row_count,
            Self::SingleFile(layout) => layout.row_count,
        }
    }

    fn horizontal_scroll_extent(&self) -> u32 {
        match self {
            Self::Empty => 0,
            Self::Inline(layout) => layout.horizontal_scroll_extent,
            Self::SideBySide(layout) => layout
                .original_horizontal_scroll_extent
                .max(layout.modified_horizontal_scroll_extent),
            Self::SingleFile(layout) => layout.horizontal_scroll_extent,
        }
    }

    fn row_for_terminal_line(&self, line: &TerminalLine) -> Option<u32> {
        self.wrapped_lines()
            .and_then(|wrapped_lines| find_terminal_line_index(wrapped_lines, line))
    }

    fn terminal_line_at_row(&self, row: u32) -> Option<TerminalLine> {
        let wrapped_lines = self.wrapped_lines()?;
        let (original, modified) = wrapped_lines
            .iter()
            .flat_map(WrappedViewLine::terminal_line_pairs)
            .nth(row as usize)?;
        match self {
            Self::Empty => None,
            Self::SingleFile(_) | Self::Inline(_) | Self::SideBySide(_) => match modified {
                TerminalLine::SourceCode { .. } => Some(modified.clone()),
                TerminalLine::Filler => match original {
                    TerminalLine::SourceCode { .. } => Some(original.clone()),
                    TerminalLine::Filler => None,
                },
            },
        }
    }
}

#[component]
pub fn DiffViewerContainer(
    scope: &mut Scope,
    content: Option<Rc<DiffContent>>,
    view_layout: DiffType,
    view_state: Ref<ViewState>,
    wrap: bool,
    compact: bool,
    auto_focus: bool,
) -> Node {
    let content = content.clone();
    let view_layout = *view_layout;
    let wrap = *wrap;
    let compact = *compact;
    let auto_focus = *auto_focus;
    let view_state = *view_state;
    let ctx = use_context::<Ui>(scope);
    let theme = &ctx.theme;
    let (node_ref, size) = use_measure(scope);
    let content_id = content.as_ref().map(|content| Rc::as_ptr(content) as usize);
    let content_for_layout = content.clone();
    let layout_data = use_memo(
        scope,
        (content_id, view_layout, wrap, compact, size.width),
        move || match content_for_layout.as_deref() {
            Some(DiffContent::Diff(diff)) => match view_layout {
                DiffType::Inline => {
                    ViewLayout::Inline(layout_inline(&diff.alignment, size.width, wrap, compact))
                }
                DiffType::SideBySide => ViewLayout::SideBySide(layout_side_by_side(
                    &diff.alignment,
                    size.width,
                    wrap,
                    compact,
                )),
                DiffType::Single => unreachable!("a diff has no single-file layout"),
            },
            Some(DiffContent::SingleFile(single)) => {
                ViewLayout::SingleFile(layout_single_file(single, size.width, wrap))
            }
            None => ViewLayout::Empty,
        },
    );

    let current_state = view_state.current().clone();
    let initial_top = current_state
        .first_terminal_line
        .as_ref()
        .and_then(|line| layout_data.row_for_terminal_line(line))
        .unwrap_or(0);
    let (scroll_view, scroll_handle) = use_scroll(scope, || ScrollOffset {
        x: current_state.first_cell,
        y: initial_top,
    });
    let restore = scroll_handle.clone();
    use_layout_effect(
        scope,
        (content_id, view_layout, wrap, compact, size.width),
        move || {
            restore.scroll_to(ScrollOffset {
                x: current_state.first_cell,
                y: initial_top,
            });
        },
    );

    let row_count = layout_data.row_count();
    let offset = scroll_view.clamped_offset();
    let first_row = (offset.y as usize).min(row_count as usize);
    let last_row = first_row
        .saturating_add(usize::from(size.height).saturating_add(4))
        .min(row_count as usize);
    let visible_range = DiffVisibleRange {
        visible_rows: first_row..last_row,
        horizontal_view: scroll_view.clone().axes(true, false),
    };

    if size.width > 0 && size.height > 0 {
        let requested = scroll_view.requested_offset();
        view_state.current().clone_from(&ViewState {
            first_terminal_line: (requested.y > 0)
                .then(|| layout_data.terminal_line_at_row(requested.y))
                .flatten(),
            first_cell: requested.x,
        });
    }

    let listeners = use_diff_viewer_navigation(scope, scroll_handle.clone(), scroll_handle, None);
    let view_node = render_view(
        content,
        view_layout,
        layout_data.as_ref(),
        visible_range,
        auto_focus,
    );
    let scroll_content_width = layout_data
        .horizontal_scroll_extent()
        .saturating_add(u32::from(size.width));

    rsx! {
        Column {
            ref: Some(node_ref),
            listeners: listeners,
            layout: Layout { grow: 1, fill: Some(theme.normal), ..Default::default() },
            ..,
            Scroll {
                view: scroll_view.axes(false, true),
                handle: None,
                horizontal: true,
                vertical: true,
                wheel_step: 0,
                content_width: Some(scroll_content_width),
                content_height: Some(row_count),
                content_area_width: Some(size.width as u32),
                content_offset: ScrollOffset { x: 0, y: first_row as u32 },
                layout: Layout { grow: 1, fill: Some(theme.normal), ..Default::default() },
                ..,
                { view_node }
            }
        }
    }
}

fn render_view(
    content: Option<Rc<DiffContent>>,
    view_layout: DiffType,
    layout_data: &ViewLayout,
    visible_range: DiffVisibleRange,
    auto_focus: bool,
) -> Node {
    let Some(content) = content else {
        return rsx! { Welcome {} };
    };
    match (content.as_ref(), view_layout, layout_data) {
        (DiffContent::Diff(_), DiffType::Inline, ViewLayout::Inline(inline_layout)) => rsx! {
            Inline {
                key: Rc::as_ptr(&content) as usize,
                content: Rc::clone(&content),
                wrapped_lines: Rc::clone(&inline_layout.wrapped_lines),
                viewport: visible_range,
                original_gutter_width: inline_layout.original_gutter_width,
                modified_gutter_width: inline_layout.modified_gutter_width,
                code_width: inline_layout.code_width,
                horizontal_scroll_extent: inline_layout.horizontal_scroll_extent,
                auto_focus: auto_focus,
            }
        },
        (
            DiffContent::Diff(_),
            DiffType::SideBySide,
            ViewLayout::SideBySide(side_by_side_layout),
        ) => rsx! {
            SideBySide {
                key: Rc::as_ptr(&content) as usize,
                content: Rc::clone(&content),
                wrapped_lines: Rc::clone(&side_by_side_layout.wrapped_lines),
                viewport: visible_range,
                original_gutter_width: side_by_side_layout.original_gutter_width,
                modified_gutter_width: side_by_side_layout.modified_gutter_width,
                original_width: side_by_side_layout.original_width,
                modified_width: side_by_side_layout.modified_width,
                original_horizontal_scroll_extent:
                    side_by_side_layout.original_horizontal_scroll_extent,
                modified_horizontal_scroll_extent:
                    side_by_side_layout.modified_horizontal_scroll_extent,
                auto_focus: auto_focus,
            }
        },
        (DiffContent::SingleFile(_), _, ViewLayout::SingleFile(single_file_layout)) => {
            rsx! {
                SingleFile {
                    key: Rc::as_ptr(&content) as usize,
                    content: Rc::clone(&content),
                    wrapped_lines: Rc::clone(&single_file_layout.wrapped_lines),
                    viewport: visible_range,
                    gutter_width: single_file_layout.gutter_width,
                    code_width: single_file_layout.code_width,
                    horizontal_scroll_extent: single_file_layout.horizontal_scroll_extent,
                    auto_focus: auto_focus,
                }
            }
        }
        _ => panic!("view layout does not match the selected content"),
    }
}
