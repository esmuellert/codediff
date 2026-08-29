//! The explorer.

pub mod build;
mod entry;

use std::collections::HashSet;
use std::rc::Rc;

use crokey::key;
use file_types::File;
use loom::{
    Bubble, Column, ColumnProps, Layout, Listeners, Node as LoomNode, Scope, component, rsx,
    use_context, use_exit, use_state,
};

use self::entry::{Body, Entry, EntryProps, Indent, Status};
use self::build::{grouped_tree, Node};
use super::context::Ui;

/// Rows kept between the cursor and the edge while scrolling.
const SCROLLOFF: u32 = 3;

#[component]
pub fn Explorer(scope: &mut Scope, on_open: Rc<dyn Fn(File)>) -> LoomNode {
    let ctx = use_context::<Ui>(scope);
    let theme = &ctx.theme;
    let view_lines = &ctx.view_lines;
    let cursor = ctx.cursor;
    let set_cursor = ctx.set_cursor;
    let files = &ctx.files;

    let (folded, set_folded) = use_state(scope, HashSet::<String>::new);

    let base = theme.normal;

    let files: &[File] = files;
    let nodes = grouped_tree(files, &folded);
    let total = nodes.len() as u32;

    // What the cursor is on, so Enter can decide what to do.
    let cursor_node = nodes.get(cursor as usize).cloned();

    let on_open = Rc::clone(on_open);
    let exit = use_exit(scope);
    let keys = Listeners::new().on_key(move |k| {
        match k {
            k if k == key!(j) || k == key!(down) => {
                if let Some(set) = set_cursor {
                    set(&|c| c.saturating_add(1).min(total.saturating_sub(1)));
                }
                Bubble::Stop
            }
            k if k == key!(k) || k == key!(up) => {
                if let Some(set) = set_cursor {
                    set(&|c| c.saturating_sub(1));
                }
                Bubble::Stop
            }
            k if k == key!(enter) => {
                match cursor_node {
                    Some(Node::Heading { .. }) => {}
                    Some(Node::Directory { ref path, .. }) => {
                        let path = path.clone();
                        set_folded(&move |mut set| {
                            if set.contains(&path) {
                                set.remove(&path);
                            } else {
                                set.insert(path.clone());
                            }
                            set
                        });
                    }
                    Some(Node::File { ref file, .. }) => {
                        on_open(file.clone());
                    }
                    None => {}
                }
                Bubble::Stop
            }
            k if k == key!(q) => {
                exit();
                Bubble::Stop
            }
            _ => Bubble::Continue,
        }
    });

    let entries: Vec<LoomNode> = view_lines
        .clone()
        .filter_map(|line| {
            nodes.get(line as usize).map(|node| {
                let selected = line == cursor;
                let (indent, body, status) = match node {
                    Node::Heading { name, count, added, removed } => {
                        let mut suffix: Vec<(Rc<str>, ratatui::style::Color)> = Vec::new();
                        if *added == 0 && *removed == 0 {
                            suffix.push((Rc::from(format!(" ({count})").as_str()), theme.tree.count));
                        } else {
                            suffix.push((Rc::from(format!(" ({count} · ").as_str()), theme.tree.count));
                            if *added > 0 {
                                suffix.push((Rc::from(format!("+{added}").as_str()), theme.change.gained));
                            }
                            if *added > 0 && *removed > 0 {
                                suffix.push((Rc::from(" "), theme.tree.count));
                            }
                            if *removed > 0 {
                                suffix.push((Rc::from(format!("-{removed}").as_str()), theme.change.lost));
                            }
                            suffix.push((Rc::from(")"), theme.tree.count));
                        }
                        (
                            Indent { markers: "".into() },
                            Body {
                                icon: None,
                                text: Rc::from(*name),
                                text_color: theme.tree.heading,
                                bold: false,
                                suffix,
                            },
                            None,
                        )
                    }
                    Node::Directory { indent, name, open, .. } => {
                        let ic = crate::theme::icon::folder(*open);
                        (
                            Indent { markers: indent.as_str().into() },
                            Body {
                                icon: Some(ic),
                                text: name.as_str().into(),
                                text_color: theme.tree.directory,
                                bold: false,
                                suffix: Vec::new(),
                            },
                            None,
                        )
                    }
                    Node::File { indent, name, file } => {
                        let ic = crate::theme::icon::file(name);
                        let change = file.get_change_type();
                        let stats = file.get_stats().filter(|s| !s.is_empty());
                        (
                            Indent { markers: indent.as_str().into() },
                            Body {
                                icon: Some(ic),
                                text: name.as_str().into(),
                                text_color: theme.tree.name,
                                bold: false,
                                suffix: file.previous_path()
                                    .map(|p| vec![(Rc::from(format!(" ← {p}").as_str()), theme.tree.previous)])
                                    .unwrap_or_default(),
                            },
                            Some(Status {
                                added: stats.map_or(0, |s| s.added),
                                removed: stats.map_or(0, |s| s.removed),
                                symbol: letter(change),
                                symbol_color: theme.change.of(change),
                                added_color: theme.change.gained,
                                removed_color: theme.change.lost,
                            }),
                        )
                    }
                };
                rsx! {
                    Entry {
                        key: line,
                        indent: indent,
                        body: body,
                        status: status,
                        selected: selected,
                    }
                }
            })
        })
        .collect();

    rsx! {
        Column {
            listeners: keys,
            layout: Layout { grow: 1, min_width: 8, fill: Some(base), ..Default::default() },
            ..,
            { entries }
        }
    }
}

/// Computes the scroll top given a cursor, total rows, viewport height, and
/// the previous top. Keeps SCROLLOFF rows between the cursor and the edges.
pub fn scroll_top(cursor: u32, total: u32, height: u32, prev_top: u32) -> u32 {
    if height == 0 {
        return 0;
    }
    let last_top = total.saturating_sub(height);
    let margin = SCROLLOFF.min(height.saturating_sub(1) / 2);

    let mut top = prev_top;
    if cursor < top + margin {
        top = cursor.saturating_sub(margin);
    }
    if cursor + margin >= top + height {
        top = (cursor + margin + 1).saturating_sub(height);
    }
    top.min(last_top)
}

pub fn letter(change: file_types::ChangeType) -> &'static str {
    match change {
        file_types::ChangeType::Added => "A",
        file_types::ChangeType::Deleted => "D",
        file_types::ChangeType::Modified => "M",
        file_types::ChangeType::Moved => "R",
        file_types::ChangeType::Untracked => "??",
        file_types::ChangeType::Conflicted => "!",
    }
}
