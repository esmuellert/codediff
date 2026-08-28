//! The explorer.

mod build;
mod entry;

use std::rc::Rc;

use file_types::File;
use loom::{
    Column, ColumnProps, Layout, Node as LoomNode, Scope, component, rsx, use_context,
    use_layout_effect,
};

use self::entry::{Entry, EntryProps};
use self::build::tree;
use super::context::Ui;

#[component]
pub fn Explorer(scope: &mut Scope, on_open: Rc<dyn Fn(File)>) -> LoomNode {
    let ctx = use_context::<Ui>(scope);
    let theme = &ctx.theme;
    let view_lines = &ctx.list_view_lines;
    let cursor = ctx.list_cursor;
    let files = &ctx.files;
    let _ = on_open;

    let base = theme.normal;

    let files: &[File] = files;
    let nodes = tree(files);

    let rows = nodes.len() as u32;
    let set_list_rows = ctx.set_list_rows;
    let set_list_viewport = ctx.set_list_viewport;
    use_layout_effect(scope, rows, move || {
        if let Some(set) = set_list_rows {
            set(&move |_| rows);
        }
        if let Some(set) = set_list_viewport {
            set(&move |mut viewport: crate::components::Viewport| {
                viewport.place(cursor.min(rows.saturating_sub(1)), rows);
                viewport
            });
        }
    });

    let entries: Vec<LoomNode> = view_lines
        .clone()
        .filter_map(|line| {
            nodes.get(line as usize).map(|node| {
                let selected = line == cursor;
                rsx! {
                    Entry {
                        key: line,
                        node: Rc::new(node.clone()),
                        selected: selected,
                    }
                }
            })
        })
        .collect();

    rsx! {
        Column {
            layout: Layout { grow: 1, min_width: 8, fill: Some(base), ..Default::default() },
            ..,
            { entries }
        }
    }
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
