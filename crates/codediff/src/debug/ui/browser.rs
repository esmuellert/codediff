//! The searchable catalog shown by `codediff debug ui` with no story ID.

use std::rc::Rc;

use anyhow::{Result, bail};
use loom::crokey::crossterm::event::{KeyCode, KeyModifiers};
use loom::crokey::{KeyCombination, OneToThree, key};
use loom::{
    Bubble, Column, ColumnProps, Layout, Listeners, Node, Scope, component, rsx, use_context,
    use_effect, use_exit, use_state,
};
use ui::Theme;
use ui::components::{Context as UiContext, Ui, UiProps};
use ui::hooks::use_scroll::use_scroll;
#[cfg(test)]
use ui::ratatui::style::Modifier;

use super::catalog;
use super::catalog_rows::{
    CatalogRow, first_story_line, heading_row, next_story_line, previous_story_line, rows,
    story_row,
};
use super::chrome::{GalleryBar, GalleryBarProps, Shortcut};

#[component]
pub(super) fn BrowserApp(
    _scope: &mut Scope,
    initial_story: usize,
    open: Rc<dyn Fn(usize)>,
) -> Node {
    rsx! {
        Ui {
            value: UiContext {
                theme: Rc::new(Theme::DARK),
                ..UiContext::default()
            },
            Browser {
                initial_story: *initial_story,
                open: Rc::clone(open),
            }
        }
    }
}

pub(super) fn snapshot(width: u16, height: u16) -> Result<Vec<String>> {
    if width == 0 || height < 3 {
        bail!("the UI catalog needs a non-zero width and at least three rows");
    }
    let mut harness = loom::testing::Harness::new::<BrowserApp>(
        BrowserAppProps {
            initial_story: 0,
            open: Rc::new(|_| {}),
        },
        width,
        height,
    );
    for _ in 0..4 {
        harness.force_draw();
    }
    Ok(harness.screen())
}

#[component]
fn Browser(scope: &mut Scope, initial_story: usize, open: Rc<dyn Fn(usize)>) -> Node {
    let theme = use_context::<Ui>(scope).theme;
    let exit = use_exit(scope);
    let (query, set_query) = use_state(scope, String::new);
    let (searching, set_searching) = use_state(scope, || false);
    let initial_rows = rows("");
    let initial_line = initial_rows
        .iter()
        .position(|row| matches!(row, CatalogRow::Story { index, .. } if *index == *initial_story))
        .unwrap_or_else(|| first_story_line(&initial_rows));
    let nodes = Rc::new(rows(&query));
    let (selected_line, set_selected_line) = use_state(scope, || initial_line as u32);
    let total = nodes.len() as u32;
    let (view, scroll) = use_scroll(scope, None, total);
    let initial_target = initial_line as u32;
    let viewport_rows = view.view_lines.len() as u32;
    use_effect(scope, (*initial_story, viewport_rows), move || {
        scroll.keep_line_visible(initial_target, 2);
    });
    let selected_story = nodes.get(selected_line as usize).and_then(|row| match row {
        CatalogRow::Story { index, .. } => Some(*index),
        CatalogRow::Heading(_) => None,
    });

    let nodes_for_keys = Rc::clone(&nodes);
    let open_key = Rc::clone(open);
    let has_query = !query.is_empty();
    let listeners = Listeners::new().on_key(move |pressed| {
        if searching {
            if pressed == key!(esc) {
                set_searching(&|_| false);
                return Bubble::Stop;
            }
            if pressed == key!(enter) {
                set_searching(&|_| false);
                if let Some(index) = selected_story {
                    open_key(index);
                }
                return Bubble::Stop;
            }
            if pressed == key!(backspace) {
                set_query(&|mut query| {
                    query.pop();
                    query
                });
                set_selected_line(&|_| 1);
                scroll.keep_line_visible(1, 0);
                return Bubble::Stop;
            }
            if let Some(character) = plain_character(pressed) {
                set_query(&move |mut query| {
                    query.push(character);
                    query
                });
                set_selected_line(&|_| 1);
                scroll.keep_line_visible(1, 0);
                return Bubble::Stop;
            }
            return Bubble::Continue;
        }

        if pressed == key!(q) {
            exit();
            Bubble::Stop
        } else if pressed == key!(esc) && has_query {
            set_query(&|_| String::new());
            set_selected_line(&|_| 1);
            scroll.keep_line_visible(1, 0);
            Bubble::Stop
        } else if pressed == key!('/') {
            set_searching(&|_| true);
            Bubble::Stop
        } else if pressed == key!(j) || pressed == key!(down) {
            let nodes = Rc::clone(&nodes_for_keys);
            set_selected_line(&move |line| {
                let next = next_story_line(&nodes, line as usize).unwrap_or(line as usize) as u32;
                scroll.keep_line_visible(next, 2);
                next
            });
            Bubble::Stop
        } else if pressed == key!(k) || pressed == key!(up) {
            let nodes = Rc::clone(&nodes_for_keys);
            set_selected_line(&move |line| {
                let next =
                    previous_story_line(&nodes, line as usize).unwrap_or(line as usize) as u32;
                scroll.keep_line_visible(next, 2);
                next
            });
            Bubble::Stop
        } else if pressed == key!(enter) {
            if let Some(index) = selected_story {
                open_key(index);
            }
            Bubble::Stop
        } else {
            Bubble::Continue
        }
    });

    let matching = nodes
        .iter()
        .filter(|row| matches!(row, CatalogRow::Story { .. }))
        .count();
    let (bar_title, bar_context, shortcuts) = if searching {
        (
            "FILTER",
            if query.is_empty() {
                "type a story name".to_owned()
            } else {
                query.clone()
            },
            vec![
                Shortcut {
                    key: "Enter",
                    action: "Open",
                },
                Shortcut {
                    key: "Esc",
                    action: "Stop filtering",
                },
                Shortcut {
                    key: "Backspace",
                    action: "Delete",
                },
            ],
        )
    } else if !query.is_empty() {
        (
            "STORIES",
            format!("{matching} matches · {query}"),
            vec![
                Shortcut {
                    key: "j/k",
                    action: "Select",
                },
                Shortcut {
                    key: "Enter",
                    action: "Open",
                },
                Shortcut {
                    key: "/",
                    action: "Edit filter",
                },
                Shortcut {
                    key: "Esc",
                    action: "Clear filter",
                },
                Shortcut {
                    key: "q",
                    action: "Quit",
                },
            ],
        )
    } else {
        (
            "STORIES",
            catalog::len().to_string(),
            vec![
                Shortcut {
                    key: "j/k",
                    action: "Select",
                },
                Shortcut {
                    key: "Enter",
                    action: "Open",
                },
                Shortcut {
                    key: "/",
                    action: "Filter",
                },
                Shortcut {
                    key: "q",
                    action: "Quit",
                },
            ],
        )
    };
    let visible: Vec<Node> = view
        .view_lines
        .clone()
        .filter_map(|line| {
            nodes.get(line as usize).map(|row| match row {
                CatalogRow::Heading(label) => heading_row(line, label, *theme),
                CatalogRow::Story { definition, .. } => {
                    story_row(line, definition, line == selected_line, *theme)
                }
            })
        })
        .collect();

    rsx! {
        Column {
            focusable: true,
            auto_focus: true,
            listeners: listeners,
            layout: Layout { grow: 1, fill: Some(theme.normal), ..Default::default() },
            ..,
            GalleryBar {
                key: 0u32,
                title: Rc::from(bar_title),
                context: Rc::from(bar_context),
                shortcuts: Rc::from(shortcuts),
            }
            Column {
                key: 1u32,
                ref: Some(view.node_ref),
                layout: Layout { grow: 1, ..Default::default() },
                ..,
                { visible }
            }
        }
    }
}

fn plain_character(key: KeyCombination) -> Option<char> {
    let forbidden = KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER;
    if key.modifiers.intersects(forbidden) {
        return None;
    }
    match key.codes {
        OneToThree::One(KeyCode::Char(character)) => Some(character),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn story_ids_and_descriptions_have_distinct_colours() {
        let mut harness = loom::testing::Harness::new::<BrowserApp>(
            BrowserAppProps {
                initial_story: 0,
                open: Rc::new(|_| {}),
            },
            100,
            24,
        );
        for _ in 0..4 {
            harness.force_draw();
        }

        assert_eq!(harness.style_at(3, 5).fg, Some(Theme::DARK.tree.directory));
        assert_eq!(harness.style_at(40, 5).fg, Some(Theme::DARK.tree.previous));
        assert_eq!(
            harness.style_at(3, 3).bg,
            Theme::DARK.normal.patch(Theme::DARK.cursor_line).bg
        );
        assert_eq!(harness.style_at(3, 3).fg, Some(Theme::DARK.tree.heading));
        for (x, y) in [(2, 2), (3, 3), (3, 5), (40, 5)] {
            assert!(!harness.style_at(x, y).add_modifier.contains(Modifier::BOLD));
        }
    }

    #[test]
    fn returning_to_the_catalog_keeps_a_late_story_visible() {
        let mut harness = loom::testing::Harness::new::<BrowserApp>(
            BrowserAppProps {
                initial_story: catalog::len() - 1,
                open: Rc::new(|_| {}),
            },
            100,
            8,
        );
        for _ in 0..4 {
            harness.force_draw();
        }

        assert!(
            harness
                .screen()
                .iter()
                .any(|row| row.contains("single-file/large-syntax"))
        );
    }
}
