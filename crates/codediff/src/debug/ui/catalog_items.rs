//! Filtered catalog items and their visual hierarchy.

use std::rc::Rc;

use loom::{Basis, Layout, Node, Row, RowProps, Text, TextProps, rsx};
use ui::Theme;

use super::catalog;
use super::definition::StoryDefinition;

#[derive(Clone)]
pub(super) enum CatalogItem {
    Heading(&'static str),
    Story {
        index: usize,
        definition: &'static StoryDefinition,
    },
}

pub(super) fn filtered_items(query: &str) -> Vec<CatalogItem> {
    let query = query.to_lowercase();
    let mut items = Vec::new();
    let mut index = 0usize;
    for group in catalog::GROUPS {
        let mut matching = Vec::new();
        for definition in group.stories {
            let matches = query.is_empty()
                || definition.id.to_lowercase().contains(&query)
                || definition.description.to_lowercase().contains(&query);
            if matches {
                matching.push(CatalogItem::Story { index, definition });
            }
            index += 1;
        }
        if !matching.is_empty() {
            items.push(CatalogItem::Heading(group.label));
            items.extend(matching);
        }
    }
    items
}

pub(super) fn first_story_line(items: &[CatalogItem]) -> usize {
    items
        .iter()
        .position(|item| matches!(item, CatalogItem::Story { .. }))
        .unwrap_or(0)
}

pub(super) fn next_story_line(items: &[CatalogItem], current: usize) -> Option<usize> {
    ((current + 1)..items.len()).find(|&line| matches!(items[line], CatalogItem::Story { .. }))
}

pub(super) fn previous_story_line(items: &[CatalogItem], current: usize) -> Option<usize> {
    (0..current)
        .rev()
        .find(|&line| matches!(items[line], CatalogItem::Story { .. }))
}

pub(super) fn heading_item(line: u32, label: &str, theme: Theme) -> Node {
    let style = theme.normal.fg(theme.tree.heading);
    rsx! {
        Row {
            key: line,
            layout: Layout {
                basis: Basis::Length(1),
                shrink: 0,
                fill: Some(theme.normal),
                ..Default::default()
            },
            ..,
            Text {
                text: Rc::from(format!("  {label}")),
                style: style,
                layout: Layout { grow: 1, ..Default::default() },
                ..
            }
        }
    }
}

pub(super) fn story_item(
    line: u32,
    definition: &StoryDefinition,
    selected: bool,
    theme: Theme,
) -> Node {
    let base = if selected {
        theme.normal.patch(theme.cursor_line)
    } else {
        theme.normal
    };
    let marker = base.fg(if selected {
        theme.line_number_current.fg.unwrap_or(theme.tree.heading)
    } else {
        theme.tree.marker
    });
    let id = base.fg(if selected {
        theme.tree.heading
    } else {
        theme.tree.directory
    });
    let description_style = base.fg(if selected {
        theme.tree.name
    } else {
        theme.tree.previous
    });

    rsx! {
        Row {
            key: line,
            layout: Layout {
                basis: Basis::Length(1),
                shrink: 0,
                fill: Some(base),
                ..Default::default()
            },
            ..,
            Text {
                key: 0u32,
                text: if selected { Rc::from(" › ") } else { Rc::from("   ") },
                style: marker,
                layout: Layout { basis: Basis::Length(3), shrink: 0, ..Default::default() },
                ..
            }
            Text {
                key: 1u32,
                text: Rc::from(definition.id),
                style: id,
                layout: Layout { basis: Basis::Length(34), shrink: 0, ..Default::default() },
                ..
            }
            Text {
                key: 2u32,
                text: Rc::from(definition.description),
                style: description_style,
                layout: Layout { grow: 1, ..Default::default() },
                ..
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filtering_keeps_group_heading_and_global_story_index() {
        let filtered = filtered_items("edge-matrix");

        assert!(matches!(filtered[0], CatalogItem::Heading("Side by side")));
        assert!(matches!(
            filtered[1],
            CatalogItem::Story {
                index: 16,
                definition
            } if definition.id == "side-by-side/edge-matrix"
        ));
    }

    #[test]
    fn navigation_skips_group_headings() {
        let items = filtered_items("");
        assert_eq!(first_story_line(&items), 1);
        assert_eq!(next_story_line(&items, 1), Some(3));
        assert_eq!(previous_story_line(&items, 3), Some(1));
    }
}
