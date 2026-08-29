//! The explorer.

pub mod build;
mod entry;

use std::collections::HashSet;
use std::rc::Rc;

use crokey::key;
use file_types::File;
use loom::{
    Bubble, Column, ColumnProps, Layout, Listeners, Node as LoomNode, Scope, component, rsx,
    use_context, use_exit, use_ref, use_state,
};

use self::entry::{Entry, EntryProps};
use self::build::{grouped_list, grouped_tree, Node};
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
    let (tree_mode, set_tree_mode) = use_state(scope, || true);

    let base = theme.normal;

    let files: &[File] = files;
    let nodes = if tree_mode {
        grouped_tree(files, &folded)
    } else {
        grouped_list(files)
    };
    let nodes = Rc::new(nodes);
    let total = nodes.len() as u32;

    // When the file list changes, keep the cursor on the same item.
    let prev_files = use_ref(scope, || Rc::clone(&ctx.files));
    let saved_anchor = use_ref(scope, || None::<String>);
    let files_changed = !Rc::ptr_eq(&ctx.files, &*prev_files.current());
    if files_changed {
        if let Some(pos) = find_by_identity(saved_anchor.current().as_deref(), &nodes) {
            if let Some(set) = set_cursor {
                set(&move |_| pos as u32);
            }
        }
        *prev_files.current() = Rc::clone(&ctx.files);
    } else {
        *saved_anchor.current() = nodes.get(cursor as usize).map(|n| identity(n));
    }

    // What the cursor is on, so Enter can decide what to do.
    let cursor_node = nodes.get(cursor as usize).cloned();

    let on_open = Rc::clone(on_open);
    let on_open_click = Rc::clone(&on_open);
    let nodes_click = Rc::clone(&nodes);
    let view_start = view_lines.start;
    let exit = use_exit(scope);
    let listeners = Listeners::new()
        .on_key(move |k| {
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
                    if let Some(ref node) = cursor_node {
                        activate_node(node, set_folded, &on_open);
                    }
                    Bubble::Stop
                }
                k if k == key!(q) => {
                    exit();
                    Bubble::Stop
                }
                k if k == key!(i) => {
                    set_tree_mode(&|mode| !mode);
                    Bubble::Stop
                }
                _ => Bubble::Continue,
            }
        })
        .on_wheel(move |delta| {
            if let Some(set) = set_cursor {
                let step = (delta.abs() * 3) as u32;
                if delta > 0 {
                    set(&move |c| c.saturating_add(step).min(total.saturating_sub(1)));
                } else {
                    set(&move |c| c.saturating_sub(step));
                }
            }
            Bubble::Stop
        })
        .on_mouse_down(move |mouse| {
            let line = view_start + mouse.local.y as u32;
            if line < total {
                if let Some(set) = set_cursor {
                    set(&move |_| line);
                }
                if let Some(node) = nodes_click.get(line as usize) {
                    activate_node(node, set_folded, &on_open_click);
                }
            }
            Bubble::Stop
        });

    let entries: Vec<LoomNode> = view_lines
        .clone()
        .filter_map(|line| {
            nodes.get(line as usize).map(|node| {
                rsx! {
                    Entry {
                        key: line,
                        node: node.clone(),
                        selected: line == cursor,
                    }
                }
            })
        })
        .collect();

    rsx! {
        Column {
            listeners: listeners,
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

fn activate_node(
    node: &Node,
    set_folded: loom::SetState<HashSet<String>>,
    on_open: &Rc<dyn Fn(File)>,
) {
    match node {
        Node::Heading { .. } => {}
        Node::Directory { path, .. } => {
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
        Node::File { file, .. } => {
            on_open(file.clone());
        }
    }
}

pub fn find_by_identity(saved: Option<&str>, nodes: &[Node]) -> Option<usize> {
    let saved = saved?;
    nodes.iter().position(|n| identity(n) == saved)
}

pub fn identity(node: &Node) -> String {
    match node {
        Node::Heading { name, .. } => name.to_string(),
        Node::Directory { path, .. } => path.clone(),
        Node::File { file, .. } => file.path().as_str().to_string(),
    }
}
