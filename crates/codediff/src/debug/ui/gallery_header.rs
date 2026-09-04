//! Shared, colour-coded navigation bar for the catalog and previews.

use std::rc::Rc;

use loom::{
    Basis, Column, ColumnProps, Layout, Node, Row, RowProps, Scope, Text, TextProps, component,
    rsx, use_context,
};
use ui::components::Ui;
use ui::ratatui::style::Style;

#[derive(Clone)]
pub struct Shortcut {
    pub key: &'static str,
    pub label: &'static str,
}

#[component]
pub fn GalleryHeader(
    scope: &mut Scope,
    title: Rc<str>,
    context: Rc<str>,
    shortcuts: Rc<[Shortcut]>,
) -> Node {
    let theme = use_context::<Ui>(scope).theme;
    let title_style = theme.status.fg(theme.tree.heading);
    let context_style = theme.status.fg(theme.tree.name);
    let key_style = theme.status.fg(theme.tree.directory);
    let action_style = theme.status.fg(theme.tree.previous);

    let mut heading = vec![text_segment(0, format!(" {title}"), title_style)];
    if !context.is_empty() {
        heading.push(text_segment(1, format!("  {context}"), context_style));
    }
    let mut commands = Vec::new();
    for (index, shortcut) in shortcuts.iter().enumerate() {
        let key = index as u32 * 2;
        commands.push(text_segment(key, format!(" {}", shortcut.key), key_style));
        commands.push(text_segment(
            key + 1,
            format!(" {}  ", shortcut.label),
            action_style,
        ));
    }

    rsx! {
        Column {
            layout: Layout {
                basis: Basis::Length(2),
                shrink: 0,
                fill: Some(theme.status),
                ..Default::default()
            },
            ..,
            Row {
                key: 0u32,
                layout: Layout {
                    basis: Basis::Length(1),
                    shrink: 0,
                    fill: Some(theme.status),
                    ..Default::default()
                },
                ..,
                { heading }
            }
            Row {
                key: 1u32,
                layout: Layout {
                    basis: Basis::Length(1),
                    shrink: 0,
                    fill: Some(theme.status),
                    ..Default::default()
                },
                ..,
                { commands }
            }
        }
    }
}

fn text_segment(key: u32, text: String, style: Style) -> Node {
    let width = line_index::LineIndex::new(&text, 1).width().0 as u16;
    rsx! {
        Text {
            key: key,
            text: Rc::from(text),
            style: style,
            layout: Layout {
                basis: Basis::Length(width),
                shrink: 0,
                ..Default::default()
            },
            ..
        }
    }
}

#[cfg(test)]
mod tests {
    use loom::testing::Harness;
    use ui::Theme;
    use ui::components::{Context, Ui};
    use ui::ratatui::style::Modifier;

    use super::*;

    #[test]
    fn title_keys_and_actions_have_separate_emphasis() {
        let mut harness = Harness::new::<GalleryHeader>(
            GalleryHeaderProps {
                title: Rc::from("STORY"),
                context: Rc::from("side-by-side/replacement"),
                shortcuts: Rc::from([Shortcut {
                    key: "q",
                    label: "Quit",
                }]),
            },
            80,
            2,
        )
        .provide::<Ui>(Context {
            theme: Rc::new(Theme::DARK),
            ..Context::default()
        });

        assert_eq!(harness.style_at(1, 0).fg, Some(Theme::DARK.tree.heading));
        assert_eq!(harness.style_at(1, 1).fg, Some(Theme::DARK.tree.directory));
        assert_eq!(harness.style_at(3, 1).fg, Some(Theme::DARK.tree.previous));
        for (x, y) in [(1, 0), (1, 1), (3, 1)] {
            assert!(!harness.style_at(x, y).add_modifier.contains(Modifier::BOLD));
        }
    }
}
