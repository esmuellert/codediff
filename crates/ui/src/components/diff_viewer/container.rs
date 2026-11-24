use std::ops::Range;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use diff::DiffContent;
use file_types::ViewerMode;
use loom::{
    Column, ColumnProps, Layout, Node, Ref, Scope, Scroll, ScrollOffset, ScrollProps, ScrollView,
    component, rsx, use_context, use_layout_effect, use_measure, use_memo, use_scroll,
};
use syntax::Span;

use super::ScrollPosition;
use super::inline::{Inline, InlineGeometry, InlineProps, inline_geometry};
use super::navigation::use_diff_viewer_navigation;
use super::side_by_side::{SideBySide, SideBySideGeometry, SideBySideProps, side_by_side_geometry};
use super::single_file::{SingleFile, SingleFileGeometry, SingleFileProps, single_file_geometry};
use super::terminal_lines::{TerminalLine, WrappedViewLine, find_terminal_line_index};
use super::welcome::Welcome;
use crate::components::context::Ui;

/// The part of the Loom viewport that a diff renderer is allowed to read.
#[derive(Clone)]
pub(crate) struct DiffVisibleRange {
    pub(crate) visible_terminal_lines: Range<usize>,
    pub(crate) horizontal_view: ScrollView,
    pub(crate) write_horizontal_metrics: bool,
}

pub(crate) enum DiffGeometry {
    Empty,
    Inline(Rc<InlineGeometry>),
    SideBySide(Rc<SideBySideGeometry>),
    SingleFile(Rc<SingleFileGeometry>),
}

impl DiffGeometry {
    fn wrapped_lines(&self) -> Option<&Rc<Vec<WrappedViewLine>>> {
        match self {
            Self::Empty => None,
            Self::Inline(geometry) => Some(&geometry.wrapped_lines),
            Self::SideBySide(geometry) => Some(&geometry.wrapped_lines),
            Self::SingleFile(geometry) => Some(&geometry.wrapped_lines),
        }
    }

    pub(crate) fn terminal_line_count(&self) -> u32 {
        match self {
            Self::Empty => 0,
            Self::Inline(geometry) => geometry.terminal_line_count,
            Self::SideBySide(geometry) => geometry.terminal_line_count,
            Self::SingleFile(geometry) => geometry.terminal_line_count,
        }
    }

    fn horizontal_scroll_extent(&self) -> u32 {
        match self {
            Self::Empty => 0,
            Self::Inline(geometry) => geometry.horizontal_scroll_extent,
            Self::SideBySide(geometry) => geometry
                .original_horizontal_scroll_extent
                .max(geometry.modified_horizontal_scroll_extent),
            Self::SingleFile(geometry) => geometry.horizontal_scroll_extent,
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

pub(crate) fn diff_geometry(
    content: Option<&DiffContent>,
    width: u16,
    viewer_mode: ViewerMode,
    wrap: bool,
    compact: bool,
) -> DiffGeometry {
    match content {
        Some(DiffContent::Diff(diff)) => match viewer_mode {
            ViewerMode::Inline => DiffGeometry::Inline(Rc::new(inline_geometry(
                &diff.alignment,
                width,
                wrap,
                compact,
            ))),
            ViewerMode::SideBySide => DiffGeometry::SideBySide(Rc::new(side_by_side_geometry(
                &diff.alignment,
                width,
                wrap,
                compact,
            ))),
            ViewerMode::Single => unreachable!("a diff has no single-file layout"),
        },
        Some(DiffContent::SingleFile(single)) => {
            DiffGeometry::SingleFile(Rc::new(single_file_geometry(single, width, wrap)))
        }
        None => DiffGeometry::Empty,
    }
}

#[component]
pub fn DiffViewerContainer(
    scope: &mut Scope,
    content: Option<Arc<DiffContent>>,
    colours: Arc<Mutex<[Vec<Vec<Span>>; 2]>>,
    viewer_mode: ViewerMode,
    scroll_position: Ref<ScrollPosition>,
    wrap: bool,
    compact: bool,
    auto_focus: bool,
) -> Node {
    let content = content.clone();
    let viewer_mode = *viewer_mode;
    let wrap = *wrap;
    let compact = *compact;
    let auto_focus = *auto_focus;
    let scroll_position = *scroll_position;
    let ctx = use_context::<Ui>(scope);
    let theme = &ctx.theme;
    let (node_ref, size) = use_measure(scope);
    let content_id = content
        .as_ref()
        .map(|content| Arc::as_ptr(content) as usize);
    let content_for_geometry = content.clone();
    let geometry = use_memo(
        scope,
        (content_id, viewer_mode, wrap, compact, size.width),
        move || {
            diff_geometry(
                content_for_geometry.as_deref(),
                size.width,
                viewer_mode,
                wrap,
                compact,
            )
        },
    );

    let current_position = scroll_position.current().clone();
    let initial_top = current_position
        .first_terminal_line
        .as_ref()
        .and_then(|line| geometry.terminal_line_index_for(line))
        .unwrap_or(0);
    let (scroll_view, scroll_handle) = use_scroll(scope, || ScrollOffset {
        x: current_position.first_cell,
        y: initial_top,
    });
    use_layout_effect(
        scope,
        (content_id, viewer_mode, wrap, compact, size.width),
        move || {
            scroll_handle.scroll_to(ScrollOffset {
                x: current_position.first_cell,
                y: initial_top,
            });
        },
    );

    let terminal_line_count = geometry.terminal_line_count();
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
        scroll_position.current().clone_from(&ScrollPosition {
            first_terminal_line: (requested.y > 0)
                .then(|| geometry.terminal_line_at_index(requested.y))
                .flatten(),
            first_cell: requested.x,
        });
    }

    let listeners = use_diff_viewer_navigation(scope, scroll_handle);
    let view_node = render_view(
        content,
        Arc::clone(colours),
        viewer_mode,
        geometry.as_ref(),
        visible_range,
        auto_focus,
    );
    let scroll_content_width = geometry
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
    viewer_mode: ViewerMode,
    geometry: &DiffGeometry,
    visible_range: DiffVisibleRange,
    auto_focus: bool,
) -> Node {
    let Some(content) = content else {
        return rsx! { Welcome {} };
    };
    match (content.as_ref(), viewer_mode, geometry) {
        (DiffContent::Diff(_), ViewerMode::Inline, DiffGeometry::Inline(geometry)) => rsx! {
            Inline {
                key: Arc::as_ptr(&content) as usize,
                content: Arc::clone(&content),
                colours,
                geometry: Rc::clone(geometry),
                viewport: visible_range,
                auto_focus,
            }
        },
        (DiffContent::Diff(_), ViewerMode::SideBySide, DiffGeometry::SideBySide(geometry)) => {
            rsx! {
                SideBySide {
                    key: Arc::as_ptr(&content) as usize,
                    content: Arc::clone(&content),
                    colours,
                    geometry: Rc::clone(geometry),
                    viewport: visible_range,
                    auto_focus,
                }
            }
        }
        (DiffContent::SingleFile(_), _, DiffGeometry::SingleFile(geometry)) => rsx! {
            SingleFile {
                key: Arc::as_ptr(&content) as usize,
                content: Arc::clone(&content),
                colours,
                geometry: Rc::clone(geometry),
                viewport: visible_range,
                auto_focus,
            }
        },
        _ => panic!("view layout does not match the selected content"),
    }
}
