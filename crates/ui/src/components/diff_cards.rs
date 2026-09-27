//! A vertically scrollable collection of compact diff cards.

use std::rc::Rc;

use file_types::DiffType;
use loom::{
    Bubble, Column, ColumnProps, Layout, Listeners, Node, Scope, Scroll, ScrollOffset, ScrollProps,
    component, rsx, use_context, use_measure, use_scroll,
};
use pipeline::diff::DiffContent;

use super::context::Ui;
use super::diff_card::{DiffCard, DiffCardProps, card_height};
use crate::keybindings::Action;

#[component]
pub fn DiffCards(
    scope: &mut Scope,
    contents: Rc<Vec<Rc<DiffContent>>>,
    view_layout: DiffType,
    wrap: bool,
) -> Node {
    let theme = *use_context::<Ui>(scope).theme;
    let view_layout = *view_layout;
    let wrap = *wrap;
    let (node_ref, size) = use_measure(scope);
    let (view, scroll) = use_scroll(scope, || ScrollOffset::ZERO);
    let card_width = size.width.saturating_sub(2);
    let heights: Vec<u16> = contents
        .iter()
        .map(|content| {
            let alignment = content
                .alignment()
                .expect("a diff card collection requires two-sided diffs");
            card_height(alignment, card_width, view_layout, wrap)
        })
        .collect();
    let content_height = heights
        .iter()
        .enumerate()
        .map(|(index, height)| u32::from(*height).saturating_add(u32::from(index > 0)))
        .sum();

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

    let cards: Vec<Node> = contents
        .iter()
        .zip(heights)
        .enumerate()
        .map(|(index, (content, height))| {
            rsx! {
                DiffCard {
                    key: Rc::as_ptr(content) as usize,
                    content: Rc::clone(content),
                    view_layout: view_layout,
                    wrap: wrap,
                    height: Some(height),
                    auto_focus: index == 0,
                }
            }
        })
        .collect();

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
