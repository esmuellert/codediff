//! Top header bar and `/` search input row for [`super::RevisionPicker`].

use std::rc::Rc;

use crokey::crossterm::event::MouseButton;
use loom::{
    Basis, Bubble, Column, ColumnProps, Gap, GapProps, Layout, Listeners, Node as LoomNode, Row,
    RowProps, Text, TextProps, rsx,
};

use crate::theme::Theme;

fn cell_width(text: &str) -> u16 {
    line_index::LineIndex::new(text, 1).width().0 as u16
}

pub(super) fn header(
    searching: bool,
    query: &str,
    theme: &Theme,
    toggle_search: impl Fn() + 'static,
) -> LoomNode {
    let top = top_bar(searching, theme, toggle_search);
    if !searching {
        return top;
    }
    let input = search_bar(query, theme);
    rsx! {
        Column {
            key: 0u32,
            layout: Layout {
                basis: Basis::Length(2),
                shrink: 0,
                fill: Some(theme.normal),
                ..Default::default()
            },
            ..,
            { top }
            { input }
        }
    }
}

fn top_bar(searching: bool, theme: &Theme, toggle_search: impl Fn() + 'static) -> LoomNode {
    let base = theme.normal;
    let action_text = if searching {
        "[Esc] close"
    } else {
        "[/] search"
    };
    let action_width = cell_width(action_text);
    let action_style = base.fg(theme.tree.previous);
    let listeners = Listeners::new().on_mouse_down(move |mouse| {
        if mouse.button == Some(MouseButton::Left) {
            toggle_search();
            Bubble::Stop
        } else {
            Bubble::Continue
        }
    });

    rsx! {
        Row {
            key: 0u32,
            layout: Layout {
                basis: Basis::Length(1),
                shrink: 0,
                clip: true,
                fill: Some(base),
                ..Default::default()
            },
            ..,
            Text {
                key: 0u32,
                text: Rc::from("git log"),
                style: base.fg(theme.tree.heading),
                layout: Layout {
                    basis: Basis::Length(cell_width("git log")),
                    shrink: 0,
                    ..Default::default()
                },
                ..
            }
            Gap {
                key: 1u32,
                layout: Layout {
                    grow: 1,
                    shrink: 1,
                    ..Default::default()
                },
                ..
            }
            Row {
                key: 2u32,
                listeners,
                layout: Layout {
                    basis: Basis::Length(action_width),
                    shrink: 0,
                    clip: true,
                    fill: Some(action_style),
                    ..Default::default()
                },
                ..,
                Text {
                    text: Rc::from(action_text),
                    style: action_style,
                    layout: Layout {
                        basis: Basis::Length(action_width),
                        shrink: 0,
                        ..Default::default()
                    },
                    ..
                }
            }
        }
    }
}

fn search_bar(query: &str, theme: &Theme) -> LoomNode {
    let style = theme.normal.patch(theme.cursor_line);
    let prompt_style = style.fg(theme.tree.heading);
    let (body_text, body_style): (Rc<str>, _) = if query.is_empty() {
        (Rc::from("Search revisions…"), style.fg(theme.tree.count))
    } else {
        (Rc::from(query), style.fg(theme.tree.name))
    };

    rsx! {
        Row {
            key: 1u32,
            layout: Layout {
                basis: Basis::Length(1),
                shrink: 0,
                clip: true,
                fill: Some(style),
                ..Default::default()
            },
            ..,
            Text {
                key: 0u32,
                text: Rc::from(" > "),
                style: prompt_style,
                layout: Layout {
                    basis: Basis::Length(3),
                    shrink: 0,
                    ..Default::default()
                },
                ..
            }
            Text {
                key: 1u32,
                text: body_text,
                style: body_style,
                layout: Layout {
                    grow: 1,
                    shrink: 1,
                    clip: true,
                    ..Default::default()
                },
                ..
            }
        }
    }
}
