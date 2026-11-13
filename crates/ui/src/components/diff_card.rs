//! A compact, horizontally scrollable diff card with a file header.

use std::ops::Range;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use align::Alignment;
use diff_service::diff::DiffContent;
use file_types::DiffType;
use loom::{
    Basis, Bubble, Column, ColumnProps, Layout, Listeners, Node, Row, RowProps, Scope, Text,
    TextProps, component, rsx, use_context, use_measure, use_memo, use_scroll,
};
use syntax::Span;

use super::border::{Border, BorderProps};
use super::code_text::{horizontal_scroll_extent, longest_line_cells};
use super::context::Ui;
use super::diff_viewer_container::DiffVisibleRange;
use super::inline::{Inline, InlineLayout, InlineProps, layout_inline};
use super::side_by_side::{SideBySide, SideBySideLayout, SideBySideProps, layout_side_by_side};
use crate::keybindings::Action;

enum CardLayout {
    Inline(InlineLayout),
    SideBySide(SideBySideLayout),
}

pub(crate) fn card_height(
    alignment: &Alignment,
    width: u16,
    view_layout: DiffType,
    wrap: bool,
) -> u16 {
    let terminal_line_count = match view_layout {
        DiffType::Inline => layout_inline(alignment, width, wrap, true).terminal_line_count,
        DiffType::SideBySide => {
            layout_side_by_side(alignment, width, wrap, true).terminal_line_count
        }
        DiffType::Single => unreachable!("a diff card cannot render a single file"),
    };
    terminal_line_count
        .saturating_add(3)
        .min(u32::from(u16::MAX)) as u16
}

fn horizontal_listeners(scope: &mut Scope, scroll: loom::ScrollHandle) -> Listeners {
    let keybindings = use_context::<Ui>(scope).keybindings;
    let wheel_scroll = scroll.clone();
    Listeners::new()
        .on_key(move |key| match key {
            key if keybindings.matches(Action::MoveLeft, key) => {
                scroll.scroll_x_by(-1);
                Bubble::Stop
            }
            key if keybindings.matches(Action::MoveRight, key) => {
                scroll.scroll_x_by(1);
                Bubble::Stop
            }
            key if keybindings.matches(Action::Start, key) => {
                scroll.scroll_x_to_start();
                Bubble::Stop
            }
            key if keybindings.matches(Action::End, key) => {
                scroll.scroll_x_to_end();
                Bubble::Stop
            }
            _ => Bubble::Continue,
        })
        .on_wheel(move |wheel| {
            if wheel.horizontal == 0 {
                Bubble::Continue
            } else {
                wheel_scroll.scroll_x_by(wheel.horizontal.saturating_mul(3));
                Bubble::Stop
            }
        })
}

#[component]
pub fn DiffCard(
    scope: &mut Scope,
    content: Arc<DiffContent>,
    colours: Arc<Mutex<[Vec<Vec<Span>>; 2]>>,
    view_layout: DiffType,
    wrap: bool,
    height: Option<u16>,
    auto_focus: bool,
) -> Node {
    let theme = *use_context::<Ui>(scope).theme;
    let view_layout = *view_layout;
    let wrap = *wrap;
    let (node_ref, size) = use_measure(scope);
    let content_id = Arc::as_ptr(content) as usize;
    let content_for_layout = Arc::clone(content);
    let card_layout = use_memo(
        scope,
        (content_id, view_layout, wrap, size.width),
        move || {
            let DiffContent::Diff(diff) = content_for_layout.as_ref() else {
                unreachable!("a diff card requires two-sided diff content")
            };
            match view_layout {
                DiffType::Inline => {
                    let mut layout = layout_inline(&diff.alignment, size.width, wrap, true);
                    if !wrap {
                        let longest = longest_line_cells(
                            diff.alignment.lines(file_types::DiffVersion::Original),
                        )
                        .max(longest_line_cells(
                            diff.alignment.lines(file_types::DiffVersion::Modified),
                        ));
                        layout.horizontal_scroll_extent =
                            horizontal_scroll_extent(longest, layout.code_width);
                    }
                    CardLayout::Inline(layout)
                }
                DiffType::SideBySide => {
                    let mut layout = layout_side_by_side(&diff.alignment, size.width, wrap, true);
                    if !wrap {
                        layout.original_horizontal_scroll_extent = horizontal_scroll_extent(
                            longest_line_cells(
                                diff.alignment.lines(file_types::DiffVersion::Original),
                            ),
                            layout.original_width,
                        );
                        layout.modified_horizontal_scroll_extent = horizontal_scroll_extent(
                            longest_line_cells(
                                diff.alignment.lines(file_types::DiffVersion::Modified),
                            ),
                            layout.modified_width,
                        );
                    }
                    CardLayout::SideBySide(layout)
                }
                DiffType::Single => unreachable!("a diff card cannot render a single file"),
            }
        },
    );

    let (scroll_view, scroll_handle) = use_scroll(scope, loom::ScrollOffset::default);
    let listeners = horizontal_listeners(scope, scroll_handle);
    let terminal_line_count = match card_layout.as_ref() {
        CardLayout::Inline(layout) => layout.terminal_line_count,
        CardLayout::SideBySide(layout) => layout.terminal_line_count,
    } as usize;
    let visible_terminal_line_count = usize::from(size.height.saturating_sub(1));
    let visible_range = DiffVisibleRange {
        visible_terminal_lines: Range {
            start: 0,
            end: terminal_line_count.min(visible_terminal_line_count),
        },
        horizontal_view: scroll_view.axes(true, false),
        write_horizontal_metrics: true,
    };
    let view_node = match (view_layout, card_layout.as_ref()) {
        (DiffType::Inline, CardLayout::Inline(layout)) => rsx! {
            Inline {
                key: content_id,
                content: Arc::clone(content),
                colours: Arc::clone(colours),
                wrapped_lines: Rc::clone(&layout.wrapped_lines),
                viewport: visible_range.clone(),
                original_gutter_width: layout.original_gutter_width,
                modified_gutter_width: layout.modified_gutter_width,
                code_width: layout.code_width,
                horizontal_scroll_extent: layout.horizontal_scroll_extent,
                auto_focus: false,
            }
        },
        (DiffType::SideBySide, CardLayout::SideBySide(layout)) => rsx! {
            SideBySide {
                key: content_id,
                content: Arc::clone(content),
                colours: Arc::clone(colours),
                wrapped_lines: Rc::clone(&layout.wrapped_lines),
                viewport: visible_range,
                original_gutter_width: layout.original_gutter_width,
                modified_gutter_width: layout.modified_gutter_width,
                original_width: layout.original_width,
                modified_width: layout.modified_width,
                original_horizontal_scroll_extent: layout.original_horizontal_scroll_extent,
                modified_horizontal_scroll_extent: layout.modified_horizontal_scroll_extent,
                auto_focus: false,
            }
        },
        _ => unreachable!("card layout does not match its view layout"),
    };

    let file = content.file();
    let change = file.get_change_type();
    let path = file.path().as_str().to_owned();
    let status = super::explorer::letter(change);
    let status_width = (status.chars().count() + 2) as u16;
    let stats = file.get_stats().filter(|stats| !stats.is_empty());
    let border_layout = match height {
        Some(height) => Layout {
            basis: Basis::Length(height.saturating_sub(4)),
            grow: 0,
            shrink: 0,
            ..Default::default()
        },
        None => Layout {
            grow: 1,
            ..Default::default()
        },
    };

    let mut header = vec![
        rsx! {
            Text {
                layout: Layout { basis: Basis::Length(status_width), shrink: 0, ..Default::default() },
                text: format!(" {status} ").into(),
                style: theme.status.fg(theme.change.of(change)),
                ..
            }
        },
        rsx! {
            Text {
                layout: Layout { grow: 1, shrink: 1, clip: true, ..Default::default() },
                text: path.into(),
                style: theme.status,
                ..
            }
        },
    ];
    if let Some(stats) = stats {
        header.push(rsx! {
            Text {
                layout: Layout { shrink: 0, ..Default::default() },
                text: format!(" +{} ", stats.added).into(),
                style: theme.status.fg(theme.change.gained),
                ..
            }
        });
        header.push(rsx! {
            Text {
                layout: Layout { shrink: 0, ..Default::default() },
                text: format!("-{} ", stats.removed).into(),
                style: theme.status.fg(theme.change.lost),
                ..
            }
        });
    }

    rsx! {
        Border {
            layout: border_layout,
            Column {
                ref: Some(node_ref),
                listeners: listeners,
                focusable: true,
                auto_focus: *auto_focus,
                layout: Layout { grow: 1, ..Default::default() },
                ..,
                Row {
                    layout: Layout { basis: Basis::Length(1), shrink: 0, fill: Some(theme.status), ..Default::default() },
                    ..,
                    { header }
                }
                { view_node }
            }
        }
    }
}
