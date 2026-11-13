//! A vertically scrollable collection of compact diff cards.
//!
//! Cards are appended in file order as their diffs arrive. The line after
//! the last card asks for the next file when it comes into view.

// The opened files are spelled out in full rather than hidden behind a name.
#![allow(clippy::type_complexity)]

use std::rc::Rc;
use std::sync::{Arc, Mutex};

use file_types::DiffType;
use loom::{
    Basis, Bubble, Column, ColumnProps, Layout, Listeners, Node, Scope, Scroll, ScrollOffset,
    ScrollProps, Text, TextProps, component, rsx, use_context, use_effect, use_measure, use_scroll,
};
use pipeline::diff::DiffContent;
use syntax::Span;

use super::context::Ui;
use super::diff_card::{DiffCard, DiffCardProps, card_height};
use crate::keybindings::Action;

#[component]
pub fn DiffCards(
    scope: &mut Scope,
    files: Rc<
        Vec<(
            Arc<Mutex<Option<Result<Arc<DiffContent>, String>>>>,
            Arc<Mutex<[Vec<Vec<Span>>; 2]>>,
        )>,
    >,
    more_files: bool,
    open_next: Rc<dyn Fn()>,
    view_layout: DiffType,
    wrap: bool,
) -> Node {
    let theme = *use_context::<Ui>(scope).theme;
    let view_layout = *view_layout;
    let wrap = *wrap;
    let (node_ref, size) = use_measure(scope);
    let (view, scroll) = use_scroll(scope, || ScrollOffset::ZERO);
    let card_width = size.width.saturating_sub(2);
    let line = |key: usize, text: String| {
        rsx! {
            Text {
                key: key,
                text: text.into(),
                layout: Layout { basis: Basis::Length(1), shrink: 0, ..Default::default() },
                style: theme.normal,
                ..
            }
        }
    };

    let mut cards = Vec::new();
    let mut heights = Vec::new();
    let mut reading = false;
    for (index, (diff, colours)) in files.iter().enumerate() {
        let diff = diff.lock().unwrap().clone();
        match diff {
            Some(Ok(content)) => {
                let alignment = content
                    .alignment()
                    .expect("a diff card collection requires two-sided diffs");
                let height = card_height(alignment, card_width, view_layout, wrap);
                heights.push(height);
                cards.push(rsx! {
                    DiffCard {
                        key: index,
                        content: content,
                        colours: Arc::clone(colours),
                        view_layout: view_layout,
                        wrap: wrap,
                        height: Some(height),
                        auto_focus: index == 0,
                    }
                });
            }
            Some(Err(error)) => {
                heights.push(1);
                cards.push(line(index, error));
            }
            None => {
                // A later file waits until this one has been read.
                reading = true;
                break;
            }
        }
    }
    let shown = cards.len();
    let mut content_height: u32 = heights
        .iter()
        .enumerate()
        .map(|(index, height)| u32::from(*height).saturating_add(u32::from(index > 0)))
        .sum();
    let has_end_line = reading || *more_files;
    let end_line_visible = if has_end_line {
        let end_line = content_height + u32::from(shown > 0);
        content_height = end_line + 1;
        cards.push(line(usize::MAX, "Loading directory diffs…".to_owned()));
        size.height > 0 && end_line < view.clamped_offset().y + u32::from(size.height)
    } else {
        false
    };
    let open_next = Rc::clone(open_next);
    use_effect(scope, (end_line_visible, shown), move || {
        if end_line_visible {
            open_next();
        }
    });

    let keybindings = use_context::<Ui>(scope).keybindings;
    let scroll_for_keys = scroll.clone();
    let listeners = Listeners::new().on_key(move |key| {
        if keybindings.matches(Action::MoveDown, key) {
            scroll_for_keys.scroll_y_by(1);
            Bubble::Stop
        } else if keybindings.matches(Action::MoveUp, key) {
            scroll_for_keys.scroll_y_by(-1);
            Bubble::Stop
        } else if keybindings.matches(Action::Start, key) {
            scroll_for_keys.scroll_to_start();
            Bubble::Stop
        } else if keybindings.matches(Action::End, key) {
            scroll_for_keys.scroll_to_end();
            Bubble::Stop
        } else {
            Bubble::Continue
        }
    });

    rsx! {
        Column {
            ref: Some(node_ref),
            focusable: true,
            auto_focus: false,
            listeners: listeners,
            layout: Layout { grow: 1, fill: Some(theme.normal), ..Default::default() },
            ..,
            Scroll {
                view: view.axes(false, true),
                handle: Some(scroll),
                horizontal: false,
                vertical: true,
                wheel_step: 3,
                content_height: Some(content_height),
                content_area_width: Some(u32::from(size.width)),
                layout: Layout { grow: 1, fill: Some(theme.normal), ..Default::default() },
                ..,
                Column {
                    layout: Layout { gap: 1, fill: Some(theme.normal), ..Default::default() },
                    ..,
                    { cards }
                }
            }
        }
    }
}
