use std::ops::Range;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use diff_service::diff::DiffContent;
use file_types::DiffType;
use loom::{
    Column, ColumnProps, Layout, Node, Ref, Scope, Scroll, ScrollOffset, ScrollProps, ScrollView,
    component, rsx, use_context, use_layout_effect, use_measure, use_memo, use_scroll,
};
use syntax::Span;

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
    pub(crate) visible_terminal_lines: Range<usize>,
    pub(crate) horizontal_view: ScrollView,
    pub(crate) write_horizontal_metrics: bool,
}

pub(crate) enum ContentLayout {
    Empty,
    Inline(Rc<InlineLayout>),
    SideBySide(Rc<SideBySideLayout>),
    SingleFile(Rc<SingleFileLayout>),
}

impl ContentLayout {
    fn wrapped_lines(&self) -> Option<&Rc<Vec<WrappedViewLine>>> {
        match self {
            Self::Empty => None,
            Self::Inline(layout) => Some(&layout.wrapped_lines),
            Self::SideBySide(layout) => Some(&layout.wrapped_lines),
            Self::SingleFile(layout) => Some(&layout.wrapped_lines),
        }
    }

    pub(crate) fn terminal_line_count(&self) -> u32 {
        match self {
            Self::Empty => 0,
            Self::Inline(layout) => layout.terminal_line_count,
            Self::SideBySide(layout) => layout.terminal_line_count,
            Self::SingleFile(layout) => layout.terminal_line_count,
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

    fn terminal_line_index_for(&self, line: &TerminalLine) -> Option<u32> {
        self.wrapped_lines()
            .and_then(|wrapped_lines| find_terminal_line_index(wrapped_lines, line))
    }

    fn terminal_line_at_index(&self, terminal_line_index: u32) -> Option<TerminalLine> {
        let wrapped_lines = self.wrapped_lines()?;
        let (original, modified) = wrapped_lines
            .iter()
            .flat_map(WrappedViewLine::terminal_line_pairs)
            .nth(terminal_line_index as usize)?;
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

pub(crate) fn layout_content(
    content: Option<&DiffContent>,
    width: u16,
    view_layout: DiffType,
    wrap: bool,
    compact: bool,
) -> ContentLayout {
    match content {
        Some(DiffContent::Diff(diff)) => match view_layout {
            DiffType::Inline => ContentLayout::Inline(Rc::new(layout_inline(
                &diff.alignment,
                width,
                wrap,
                compact,
            ))),
            DiffType::SideBySide => ContentLayout::SideBySide(Rc::new(layout_side_by_side(
                &diff.alignment,
                width,
                wrap,
                compact,
            ))),
            DiffType::Single => unreachable!("a diff has no single-file layout"),
        },
        Some(DiffContent::SingleFile(single)) => {
            ContentLayout::SingleFile(Rc::new(layout_single_file(single, width, wrap)))
        }
        None => ContentLayout::Empty,
    }
}

#[component]
pub fn DiffViewerContainer(
    scope: &mut Scope,
    content: Option<Arc<DiffContent>>,
    colours: Arc<Mutex<[Vec<Vec<Span>>; 2]>>,
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
    let content_id = content
        .as_ref()
        .map(|content| Arc::as_ptr(content) as usize);
    let content_for_layout = content.clone();
    let content_layout = use_memo(
        scope,
        (content_id, view_layout, wrap, compact, size.width),
        move || {
            layout_content(
                content_for_layout.as_deref(),
                size.width,
                view_layout,
                wrap,
                compact,
            )
        },
    );

    let current_state = view_state.current().clone();
    let initial_top = current_state
        .first_terminal_line
        .as_ref()
        .and_then(|line| content_layout.terminal_line_index_for(line))
        .unwrap_or(0);
    let (scroll_view, scroll_handle) = use_scroll(scope, || ScrollOffset {
        x: current_state.first_cell,
        y: initial_top,
    });
    use_layout_effect(
        scope,
        (content_id, view_layout, wrap, compact, size.width),
        move || {
            scroll_handle.scroll_to(ScrollOffset {
                x: current_state.first_cell,
                y: initial_top,
            });
        },
    );

    let terminal_line_count = content_layout.terminal_line_count();
    let offset = scroll_view.clamped_offset();
    let first_terminal_line = (offset.y as usize).min(terminal_line_count as usize);
    let last_terminal_line = first_terminal_line
        .saturating_add(usize::from(size.height).saturating_add(4))
        .min(terminal_line_count as usize);
    let visible_range = DiffVisibleRange {
        visible_terminal_lines: first_terminal_line..last_terminal_line,
        horizontal_view: scroll_view.clone().axes(true, false),
        write_horizontal_metrics: false,
    };

    if size.width > 0 && size.height > 0 {
        let requested = scroll_view.requested_offset();
        view_state.current().clone_from(&ViewState {
            first_terminal_line: (requested.y > 0)
                .then(|| content_layout.terminal_line_at_index(requested.y))
                .flatten(),
            first_cell: requested.x,
        });
    }

    let listeners = use_diff_viewer_navigation(scope, scroll_handle);
    let view_node = render_view(
        content,
        Arc::clone(colours),
        view_layout,
        content_layout.as_ref(),
        visible_range,
        auto_focus,
    );
    let scroll_content_width = content_layout
        .horizontal_scroll_extent()
        .saturating_add(u32::from(size.width));

    rsx! {
        Column {
            ref: Some(node_ref),
            listeners,
            layout: Layout { grow: 1, fill: Some(theme.normal), ..Default::default() },
            ..,
            Scroll {
                view: scroll_view.axes(false, true),
                handle: None,
                horizontal: true,
                vertical: true,
                wheel_step: 0,
                content_width: Some(scroll_content_width),
                content_height: Some(terminal_line_count),
                content_area_width: Some(size.width as u32),
                content_offset: ScrollOffset { x: 0, y: first_terminal_line as u32 },
                layout: Layout { grow: 1, fill: Some(theme.normal), ..Default::default() },
                ..,
                { view_node }
            }
        }
    }
}

fn render_view(
    content: Option<Arc<DiffContent>>,
    colours: Arc<Mutex<[Vec<Vec<Span>>; 2]>>,
    view_layout: DiffType,
    content_layout: &ContentLayout,
    visible_range: DiffVisibleRange,
    auto_focus: bool,
) -> Node {
    let Some(content) = content else {
        return rsx! { Welcome {} };
    };
    match (content.as_ref(), view_layout, content_layout) {
        (DiffContent::Diff(_), DiffType::Inline, ContentLayout::Inline(layout)) => rsx! {
            Inline {
                key: Arc::as_ptr(&content) as usize,
                content: Arc::clone(&content),
                colours,
                layout: Rc::clone(layout),
                viewport: visible_range,
                auto_focus,
            }
        },
        (DiffContent::Diff(_), DiffType::SideBySide, ContentLayout::SideBySide(layout)) => rsx! {
            SideBySide {
                key: Arc::as_ptr(&content) as usize,
                content: Arc::clone(&content),
                colours,
                layout: Rc::clone(layout),
                viewport: visible_range,
                auto_focus,
            }
        },
        (DiffContent::SingleFile(_), _, ContentLayout::SingleFile(layout)) => rsx! {
            SingleFile {
                key: Arc::as_ptr(&content) as usize,
                content: Arc::clone(&content),
                colours,
                layout: Rc::clone(layout),
                viewport: visible_range,
                auto_focus,
            }
        },
        _ => panic!("view layout does not match the selected content"),
    }
}
