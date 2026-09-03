//! The explorer.

pub mod build;
mod entry;

use std::collections::HashSet;
use std::rc::Rc;

use crokey::key;
use file_types::File;
use loom::{
    Bubble, Column, ColumnProps, Layout, Listeners, Node as LoomNode, Scope, component, rsx,
    use_context, use_effect, use_exit, use_ref, use_state,
};

use self::build::{Node, directory_key, grouped_list, grouped_tree};
use self::entry::{Entry, EntryProps};
use super::context::Ui;
use crate::hooks::use_scroll::use_scroll;
use crate::services::version_control::VersionControlService;

#[component]
pub fn Explorer(scope: &mut Scope) -> LoomNode {
    let ctx = use_context::<Ui>(scope);
    let theme = &ctx.theme;
    let set_file = ctx.set_file;
    let (files, set_files) = use_state(scope, || Rc::new(Vec::<File>::new()));
    let repo = Rc::clone(&ctx.repo);
    let files_service = ctx.files_service.as_ref().map(Rc::clone);
    let watcher_service = ctx.watcher_service.as_ref().map(Rc::clone);
    let repo_for_effect = Rc::clone(&repo);
    use_effect(scope, repo, move || {
        let Some(files_service) = files_service else {
            return;
        };
        let requested_repo_for_response = Rc::clone(&repo_for_effect);
        files_service.get(&repo_for_effect).subscribe(
            move |response: pipeline::files::Response| {
                if response.repo.as_path() != requested_repo_for_response.as_ref() {
                    return;
                }
                let matching_files = Rc::new(response.files);
                if let Some(set_file) = set_file {
                    let files_for_selection = Rc::clone(&matching_files);
                    set_file(&move |selected| {
                        matching_selected_file(selected, &files_for_selection)
                    });
                }
                set_files(&move |_| Rc::clone(&matching_files));
            },
        );
        let Some(watcher_service) = watcher_service else {
            return;
        };
        let files_service_to_refresh = Rc::clone(&files_service);
        let repo_to_refresh = Rc::clone(&repo_for_effect);
        watcher_service
            .changes()
            .subscribe(move |refresh: watcher::Refresh| {
                if !refresh.is_empty() {
                    files_service_to_refresh.refresh(&repo_to_refresh);
                }
            });
    });

    let (view, handle) = use_scroll(scope, None);
    let (folded, set_folded) = use_state(scope, HashSet::<String>::new);
    let (tree_mode, set_tree_mode) = use_state(scope, || true);

    let base = theme.normal;

    let nodes = if tree_mode {
        grouped_tree(&files, &folded)
    } else {
        grouped_list(&files)
    };
    let nodes = Rc::new(nodes);
    let total = nodes.len() as u32;

    // When the file list changes, keep the cursor on the same item.
    let prev_files = use_ref(scope, || Rc::clone(&files));
    let saved_anchor = use_ref(scope, || None::<String>);
    let files_changed = !Rc::ptr_eq(&files, &*prev_files.current());
    if files_changed {
        if let Some(pos) = find_by_identity(saved_anchor.current().as_deref(), &nodes) {
            handle.set(pos as u32);
        }
        *prev_files.current() = Rc::clone(&files);
    } else {
        *saved_anchor.current() = nodes.get(view.cursor as usize).map(identity);
    }

    let cursor_node = nodes.get(view.cursor as usize).cloned();
    let nodes_click = Rc::clone(&nodes);
    let nodes_keys = Rc::clone(&nodes);
    let version_control_service = ctx.version_control_service.as_ref().map(Rc::clone);
    let exit = use_exit(scope);

    let listeners = Listeners::new()
        .on_key(move |k| match k {
            k if k == key!(j) || k == key!(down) => {
                handle.down(total);
                let next = (view.cursor + 1).min(total.saturating_sub(1));
                if let Some(Node::File { file, .. }) = nodes_keys.get(next as usize) {
                    open_file(file, set_file);
                }
                Bubble::Stop
            }
            k if k == key!(k) || k == key!(up) => {
                handle.up(total);
                let next = view.cursor.saturating_sub(1);
                if let Some(Node::File { file, .. }) = nodes_keys.get(next as usize) {
                    open_file(file, set_file);
                }
                Bubble::Stop
            }
            k if k == key!(enter) => {
                if let Some(ref node) = cursor_node {
                    activate_node(node, set_folded, set_file);
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
            k if k == key!(space) => {
                if let Some(ref node) = cursor_node {
                    toggle_stage(node, version_control_service.as_deref());
                }
                Bubble::Stop
            }
            k if k == key!(right) => {
                loom::focus_next();
                Bubble::Stop
            }
            _ => Bubble::Continue,
        })
        .on_wheel(move |delta| {
            handle.wheel(delta, total);
            Bubble::Stop
        })
        .on_mouse_down(move |mouse| {
            let line = handle.click(mouse.local.y as u32, total);
            if let Some(node) = nodes_click.get(line as usize) {
                activate_node(node, set_folded, set_file);
            }
            Bubble::Stop
        });

    let entries: Vec<LoomNode> = view
        .view_lines
        .clone()
        .filter_map(|line| {
            nodes.get(line as usize).map(|node| {
                rsx! {
                    Entry {
                        key: line,
                        node: node.clone(),
                        selected: line == view.cursor,
                    }
                }
            })
        })
        .collect();

    rsx! {
        Column {
            ref: Some(view.node_ref),
            focusable: true,
            auto_focus: true,
            listeners: listeners,
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

fn toggle_stage(node: &Node, version_control_service: Option<&VersionControlService>) {
    let Some(version_control_service) = version_control_service else {
        return;
    };
    match node {
        Node::Heading { .. } => {}
        Node::Directory { path, revs, .. } => {
            version_control_service.toggle_stage(path, revs);
        }
        Node::File { file, .. } => {
            version_control_service.toggle_stage(file.path(), &file.revs());
        }
    }
}

fn activate_node(
    node: &Node,
    set_folded: loom::SetState<HashSet<String>>,
    set_file: Option<loom::SetState<Option<Rc<File>>>>,
) {
    match node {
        Node::Heading { .. } => {}
        Node::Directory { path, revs, .. } => {
            let key = directory_key(path, revs);
            set_folded(&move |mut set| {
                if set.contains(&key) {
                    set.remove(&key);
                } else {
                    set.insert(key.clone());
                }
                set
            });
        }
        Node::File { file, .. } => open_file(file, set_file),
    }
}

/// Puts a file in the context, so the diff viewer shows it.
fn open_file(file: &File, set_file: Option<loom::SetState<Option<Rc<File>>>>) {
    if let Some(set) = set_file {
        let file = Rc::new(file.clone());
        set(&move |_| Some(Rc::clone(&file)));
    }
}

fn matching_selected_file(selected: Option<Rc<File>>, files: &[File]) -> Option<Rc<File>> {
    let selected = selected?;
    let matching = files.iter().find(|file| same_file_entry(file, &selected))?;
    if matching == selected.as_ref() {
        Some(selected)
    } else {
        Some(Rc::new(matching.clone()))
    }
}

fn same_file_entry(left: &File, right: &File) -> bool {
    left.path() == right.path() && left.revs().heading() == right.revs().heading()
}

pub fn find_by_identity(saved: Option<&str>, nodes: &[Node]) -> Option<usize> {
    let saved = saved?;
    nodes.iter().position(|n| identity(n) == saved)
}

pub fn identity(node: &Node) -> String {
    match node {
        Node::Heading { name, .. } => name.to_string(),
        Node::Directory { path, revs, .. } => directory_key(path, revs),
        Node::File { file, .. } => file.path().as_str().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use file_types::{Oid, RepoPath, Revs};

    use super::*;

    #[test]
    fn an_unchanged_selection_keeps_its_rc() {
        let file = File::unchanged_path(
            RepoPath::new("selected.rs", Path::new("/repo")),
            Revs::worktree_against(Oid::new("abc")),
        );
        let selected = Rc::new(file.clone());

        let matching = matching_selected_file(Some(Rc::clone(&selected)), &[file]).unwrap();

        assert!(Rc::ptr_eq(&matching, &selected));
    }
}
