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
    CatalogRow, filtered_rows, first_story_line, heading_row, next_story_line, previous_story_line,
    story_row,
};
use super::gallery_header::{GalleryHeader, GalleryHeaderProps, Shortcut};

#[component]
pub(super) fn CatalogRoot(
    _scope: &mut Scope,
    initial_story_index: usize,
    open_story: Rc<dyn Fn(usize)>,
) -> Node {
    rsx! {
        Ui {
            value: UiContext {
                theme: Rc::new(Theme::DARK),
                ..UiContext::default()
            },
            CatalogView {
                initial_story_index: *initial_story_index,
                open_story: Rc::clone(open_story),
            }
        }
    }
}

pub(super) fn snapshot(width: u16, height: u16) -> Result<Vec<String>> {
    if width == 0 || height < 3 {
        bail!("the UI catalog needs a non-zero width and at least three rows");
    }
    let mut harness = loom::testing::Harness::new::<CatalogRoot>(
        CatalogRootProps {
            initial_story_index: 0,
            open_story: Rc::new(|_| {}),
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
fn CatalogView(
    scope: &mut Scope,
    initial_story_index: usize,
    open_story: Rc<dyn Fn(usize)>,
) -> Node {
    let theme = use_context::<Ui>(scope).theme;
    let exit = use_exit(scope);
    let (query, set_query) = use_state(scope, String::new);
    let (searching, set_searching) = use_state(scope, || false);
    let initial_rows = filtered_rows("");
    let initial_line = initial_rows
        .iter()
        .position(
            |row| matches!(row, CatalogRow::Story { index, .. } if *index == *initial_story_index),
        )
        .unwrap_or_else(|| first_story_line(&initial_rows));
    let catalog_rows = Rc::new(filtered_rows(&query));
    let (selected_line, set_selected_line) = use_state(scope, || initial_line as u32);
    let total = catalog_rows.len() as u32;
    let (view, scroll) = use_scroll(scope, None, total);
    let initial_target = initial_line as u32;
    let viewport_rows = view.view_lines.len() as u32;
    use_effect(scope, (*initial_story_index, viewport_rows), move || {
        scroll.keep_line_visible(initial_target, 2);
    });
    let selected_story_index = catalog_rows
        .get(selected_line as usize)
        .and_then(|row| match row {
            CatalogRow::Story { index, .. } => Some(*index),
            CatalogRow::Heading(_) => None,
        });

    let rows_for_keys = Rc::clone(&catalog_rows);
    let open_selected_story = Rc::clone(open_story);
    let has_query = !query.is_empty();
    let listeners = Listeners::new().on_key(move |pressed| {
        if searching {
            if pressed == key!(esc) {
                set_searching(&|_| false);
                return Bubble::Stop;
            }
            if pressed == key!(enter) {
                set_searching(&|_| false);
                if let Some(index) = selected_story_index {
                    open_selected_story(index);
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
            let rows = Rc::clone(&rows_for_keys);
            set_selected_line(&move |line| {
                let next = next_story_line(&rows, line as usize).unwrap_or(line as usize) as u32;
                scroll.keep_line_visible(next, 2);
                next
            });
            Bubble::Stop
        } else if pressed == key!(k) || pressed == key!(up) {
            let rows = Rc::clone(&rows_for_keys);
            set_selected_line(&move |line| {
                let next =
                    previous_story_line(&rows, line as usize).unwrap_or(line as usize) as u32;
                scroll.keep_line_visible(next, 2);
                next
            });
            Bubble::Stop
        } else if pressed == key!(enter) {
            if let Some(index) = selected_story_index {
                open_selected_story(index);
            }
            Bubble::Stop
        } else {
            Bubble::Continue
        }
    });

    let matching_story_count = catalog_rows
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
                    label: "Open",
                },
                Shortcut {
                    key: "Esc",
                    label: "Stop filtering",
                },
                Shortcut {
                    key: "Backspace",
                    label: "Delete",
                },
            ],
        )
    } else if !query.is_empty() {
        (
            "STORIES",
            format!("{matching_story_count} matches · {query}"),
            vec![
                Shortcut {
                    key: "j/k",
                    label: "Select",
                },
                Shortcut {
                    key: "Enter",
                    label: "Open",
                },
                Shortcut {
                    key: "/",
                    label: "Edit filter",
                },
                Shortcut {
                    key: "Esc",
                    label: "Clear filter",
                },
                Shortcut {
                    key: "q",
                    label: "Quit",
                },
            ],
        )
    } else {
        (
            "STORIES",
            catalog::story_count().to_string(),
            vec![
                Shortcut {
                    key: "j/k",
                    label: "Select",
                },
                Shortcut {
                    key: "Enter",
                    label: "Open",
                },
                Shortcut {
                    key: "/",
                    label: "Filter",
                },
                Shortcut {
                    key: "q",
                    label: "Quit",
                },
            ],
        )
    };
    let visible: Vec<Node> = view
        .view_lines
        .clone()
        .filter_map(|line| {
            catalog_rows.get(line as usize).map(|row| match row {
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
            GalleryHeader {
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
        let mut harness = loom::testing::Harness::new::<CatalogRoot>(
            CatalogRootProps {
                initial_story_index: 0,
                open_story: Rc::new(|_| {}),
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
        let mut harness = loom::testing::Harness::new::<CatalogRoot>(
            CatalogRootProps {
                initial_story_index: catalog::story_count() - 1,
                open_story: Rc::new(|_| {}),
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
                .any(|row| row.contains("single-file/long-syntax-file"))
        );
    }
}
