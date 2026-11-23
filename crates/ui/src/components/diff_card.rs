//! A compact, horizontally scrollable diff card with a file header.

use std::ops::Range;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use diff_service::diff::DiffContent;
use file_types::DiffType;
use loom::{
    Basis, Bubble, Column, ColumnProps, Layout, Listeners, Node, Row, RowProps, Scope, Text,
    TextProps, component, rsx, use_context, use_measure, use_memo, use_scroll,
};
use syntax::Span;

use super::border::{Border, BorderProps};
use super::context::Ui;
use super::diff_viewer_container::{ContentLayout, DiffVisibleRange, layout_content};
use super::inline::{Inline, InlineProps, layout_inline};
use super::side_by_side::{SideBySide, SideBySideProps, layout_side_by_side};
use super::single_file::{SingleFile, SingleFileProps, layout_single_file};
use crate::keybindings::Action;

pub(crate) fn card_height(
    content: &DiffContent,
    width: u16,
    view_layout: DiffType,
    wrap: bool,
) -> u16 {
    let terminal_line_count = match content {
        DiffContent::Diff(diff) => match view_layout {
            DiffType::Inline => {
                layout_inline(&diff.alignment, width, wrap, true).terminal_line_count
            }
            DiffType::SideBySide => {
                layout_side_by_side(&diff.alignment, width, wrap, true).terminal_line_count
            }
            DiffType::Single => unreachable!("a diff has no single-file layout"),
        },
        DiffContent::SingleFile(single) => {
            layout_single_file(single, width, wrap).terminal_line_count
        }
    };
    terminal_line_count
        .saturating_add(3)
        .min(u32::from(u16::MAX)) as u16
}

fn horizontal_listeners(scope: &mut Scope, scroll: loom::ScrollHandle) -> Listeners {
    let keybindings = use_context::<Ui>(scope).keybindings;
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
                scroll.scroll_x_by(wheel.horizontal.saturating_mul(3));
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
            layout_content(
                Some(content_for_layout.as_ref()),
                size.width,
                view_layout,
                wrap,
                true,
            )
        },
    );

    let (scroll_view, scroll_handle) = use_scroll(scope, loom::ScrollOffset::default);
    let listeners = horizontal_listeners(scope, scroll_handle);
    let terminal_line_count = card_layout.terminal_line_count() as usize;
    let visible_terminal_line_count = usize::from(size.height.saturating_sub(1));
    let visible_range = DiffVisibleRange {
        visible_terminal_lines: Range {
            start: 0,
            end: terminal_line_count.min(visible_terminal_line_count),
        },
        horizontal_view: scroll_view.axes(true, false),
        write_horizontal_metrics: true,
    };
    let view_node = match card_layout.as_ref() {
        ContentLayout::Inline(layout) => rsx! {
            Inline {
                key: content_id,
                content: Arc::clone(content),
                colours: Arc::clone(colours),
                layout: Rc::clone(layout),
                viewport: visible_range,
                auto_focus: false,
            }
        },
        ContentLayout::SideBySide(layout) => rsx! {
            SideBySide {
                key: content_id,
                content: Arc::clone(content),
                colours: Arc::clone(colours),
                layout: Rc::clone(layout),
                viewport: visible_range,
                auto_focus: false,
            }
        },
        ContentLayout::SingleFile(layout) => rsx! {
            SingleFile {
                key: content_id,
                content: Arc::clone(content),
                colours: Arc::clone(colours),
                layout: Rc::clone(layout),
                viewport: visible_range,
                auto_focus: false,
            }
        },
        ContentLayout::Empty => unreachable!("a diff card always has content"),
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
            max_height: Some(*height),
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
        if stats.added > 0 {
            header.push(rsx! {
                Text {
                    layout: Layout { shrink: 0, ..Default::default() },
                    text: format!(" +{} ", stats.added).into(),
                    style: theme.status.fg(theme.change.gained),
                    ..
                }
            });
        }
        if stats.removed > 0 {
            let prefix = if stats.added > 0 { "" } else { " " };
            header.push(rsx! {
                Text {
                    layout: Layout { shrink: 0, ..Default::default() },
                    text: format!("{prefix}-{} ", stats.removed).into(),
                    style: theme.status.fg(theme.change.lost),
                    ..
                }
            });
        }
    }

    rsx! {
        Border {
            layout: border_layout,
            Column {
                ref: Some(node_ref),
                listeners,
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
