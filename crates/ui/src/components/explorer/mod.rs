//! The explorer.

pub mod build;
mod entry;

use std::collections::HashSet;
use std::rc::Rc;

use file_types::{File, RepoPath, Revs};
use loom::{
    Bubble, Column, ColumnProps, Layout, Listeners, Node as LoomNode, Scope, Scroll, ScrollOffset,
    ScrollProps, Text, TextProps, component, rsx, use_context, use_effect, use_exit, use_measure,
    use_memo, use_ref, use_scroll, use_state,
};

use self::build::{Node, directory_key, grouped_list, grouped_tree};
use self::entry::{Entry, EntryProps};
use super::context::{DirectorySelection, Ui};
use crate::keybindings::Action;
use crate::services::version_control::VersionControlService;

const SCROLLOFF: u32 = 3;

#[component]
pub fn Explorer(scope: &mut Scope) -> LoomNode {
    let ctx = use_context::<Ui>(scope);
    let theme = &ctx.theme;
    let set_file = ctx.set_file;
    let set_directory = ctx.set_directory;
    let (files, set_files) = use_state(scope, || Rc::new(Vec::<File>::new()));
    let repo = Rc::clone(&ctx.repo);
    let pipeline = ctx.diff_service.as_ref().map(Rc::clone);
    let watcher_service = ctx.watcher_service.as_ref().map(Rc::clone);
    // A change on disk lists the files again; the previous list is dropped.
    let (refreshes, set_refreshes) = use_state(scope, || 0u32);
    use_effect(scope, Rc::clone(&repo), move || {
        let Some(watcher_service) = watcher_service else {
            return;
        };
        watcher_service
            .changes()
            .subscribe(move |refresh: watcher::Refresh| {
                if !refresh.is_empty() {
                    set_refreshes(&|refreshes| refreshes + 1);
                }
            });
    });
    let repo_to_list = Rc::clone(&repo);
    let listing = use_memo(scope, (Rc::clone(&repo), refreshes), move || {
        pipeline.map(|pipeline| pipeline.list_files(&repo_to_list))
    });
    let (listed, list_error) = match listing
        .as_ref()
        .as_ref()
        .map(|listing| listing.lock().unwrap())
    {
        Some(result) => match result.as_ref() {
            Some(Ok(_)) => (true, None),
            Some(Err(error)) => (false, Some(error.clone())),
            None => (false, None),
        },
        None => (false, None),
    };
    use_effect(scope, (repo, refreshes, listed), move || {
        let Some(Some(Ok(listed))) = listing
            .as_ref()
            .as_ref()
            .map(|listing| listing.lock().unwrap().clone())
        else {
            return;
        };
        let matching_files = Rc::new(listed);
        if let Some(set_file) = set_file {
            let files_for_selection = Rc::clone(&matching_files);
            set_file(&move |selected| matching_selected_file(selected, &files_for_selection));
        }
        if let Some(set_directory) = set_directory {
            let files_for_selection = Rc::clone(&matching_files);
            set_directory(&move |selected| {
                matching_selected_directory(selected.as_ref(), &files_for_selection)
            });
        }
        set_files(&move |_| Rc::clone(&matching_files));
    });

    let (selected_line, set_selected_line) = use_state(scope, || 0u32);
    let (folded, set_folded) = use_state(scope, HashSet::<String>::new);
    let (tree_mode, set_tree_mode) = use_state(scope, || {
        matches!(ctx.config.ui.explorer_mode, config::ExplorerMode::Tree)
    });

    let base = theme.normal;

    let nodes = if tree_mode {
        grouped_tree(&files, &folded)
    } else {
        grouped_list(&files)
    };
    let nodes = Rc::new(nodes);
    let catalog_line_count = nodes.len() as u32;
    let (node_ref, size) = use_measure(scope);
    let (view, scroll) = use_scroll(scope, || ScrollOffset::ZERO);
    let visible_start = view
        .clamped_offset()
        .y
        .min(catalog_line_count.saturating_sub(1));
    let visible_count = u32::from(size.height).saturating_add(4);

    // When the file list changes, keep the selection on the same item.
    let prev_files = use_ref(scope, || Rc::clone(&files));
    let saved_anchor = use_ref(scope, || None::<String>);
    let files_changed = !Rc::ptr_eq(&files, &*prev_files.current());
    if files_changed {
        if let Some(pos) = find_by_identity(saved_anchor.current().as_deref(), &nodes) {
            set_selected_line(&move |_| pos as u32);
        }
        *prev_files.current() = Rc::clone(&files);
    } else {
        *saved_anchor.current() = nodes.get(selected_line as usize).map(identity);
    }

    let selected_node = nodes.get(selected_line as usize).cloned();
    let nodes_click = Rc::clone(&nodes);
    let nodes_keys = Rc::clone(&nodes);
    let files_for_keys = Rc::clone(&files);
    let files_for_mouse = Rc::clone(&files);
    let keybindings = Rc::clone(&ctx.keybindings);
    let version_control_service = ctx.version_control_service.as_ref().map(Rc::clone);
    let exit = use_exit(scope);
    let scroll_for_keys = scroll.clone();
    let view_for_mouse = view.clone();

    let listeners = Listeners::new()
        .on_key(move |k| match k {
            k if keybindings.matches(Action::MoveDown, k) => {
                let nodes = Rc::clone(&nodes_keys);
                let files = Rc::clone(&files_for_keys);
                let scroll = scroll_for_keys.clone();
                set_selected_line(&move |selected_line| {
                    let next = selected_line
                        .saturating_add(1)
                        .min(catalog_line_count.saturating_sub(1));
                    scroll.keep_y_visible(next, SCROLLOFF);
                    if let Some(node) = nodes.get(next as usize) {
                        select_node(node, &files, set_file, set_directory);
                    }
                    next
                });
                Bubble::Stop
            }
            k if keybindings.matches(Action::MoveUp, k) => {
                let nodes = Rc::clone(&nodes_keys);
                let files = Rc::clone(&files_for_keys);
                let scroll = scroll_for_keys.clone();
                set_selected_line(&move |selected_line| {
                    let next = selected_line.saturating_sub(1);
                    scroll.keep_y_visible(next, SCROLLOFF);
                    if let Some(node) = nodes.get(next as usize) {
                        select_node(node, &files, set_file, set_directory);
                    }
                    next
                });
                Bubble::Stop
            }
            k if keybindings.matches(Action::Open, k) => {
                if let Some(ref node) = selected_node {
                    activate_node(node, set_folded, set_file, set_directory);
                }
                Bubble::Stop
            }
            k if keybindings.matches(Action::Quit, k) => {
                exit();
                Bubble::Stop
            }
            k if keybindings.matches(Action::ToggleExplorerMode, k) => {
                set_tree_mode(&|mode| !mode);
                Bubble::Stop
            }
            k if keybindings.matches(Action::Stage, k) => {
                if let Some(ref node) = selected_node {
                    toggle_stage(node, version_control_service.as_deref());
                }
                Bubble::Stop
            }
            k if keybindings.matches(Action::FocusNext, k) => {
                loom::focus_next();
                Bubble::Stop
            }
            _ => Bubble::Continue,
        })
        .on_mouse_down(move |mouse| {
            let line = (view_for_mouse.clamped_offset().y + u32::from(mouse.local.y))
                .min(catalog_line_count.saturating_sub(1));
            let already_selected = line == selected_line;
            set_selected_line(&move |_| line);
            if let Some(node) = nodes_click.get(line as usize) {
                if already_selected && matches!(node, Node::Directory { .. }) {
                    activate_node(node, set_folded, set_file, set_directory);
                } else {
                    select_node(node, &files_for_mouse, set_file, set_directory);
                }
            }
            Bubble::Stop
        });

    let entries: Vec<LoomNode> = nodes
        .iter()
        .skip(visible_start as usize)
        .take(visible_count as usize)
        .enumerate()
        .map(|(offset, node)| {
            let line = visible_start as usize + offset;
            rsx! {
                Entry {
                    key: line,
                    node: node.clone(),
                    selected: line as u32 == selected_line,
                }
            }
        })
        .collect();

    let list_error = list_error.map_or(LoomNode::Empty, |error| {
        rsx! {
            Text {
                text: format!("Cannot list files: {error}").into(),
                style: theme.warning,
                layout: Layout { basis: loom::Basis::Length(1), shrink: 0, ..Default::default() },
                ..
            }
        }
    });

    rsx! {
        Column {
            ref: Some(node_ref),
            focusable: true,
            auto_focus: true,
            listeners: listeners,
            layout: Layout { grow: 1, min_width: 8, fill: Some(base), ..Default::default() },
            ..,
            { list_error }
            Scroll {
                view: view,
                handle: Some(scroll.clone()),
                horizontal: false,
                vertical: true,
                wheel_step: 3,
                content_height: Some(catalog_line_count),
                content_offset: ScrollOffset { x: 0, y: visible_start },
                layout: Layout { grow: 1, fill: Some(base), ..Default::default() },
                ..,
                Column {
                    layout: Layout { fill: Some(base), ..Default::default() },
                    ..,
                    { entries }
                }
            }
        }
    }
}

pub fn letter(change: file_types::ChangeType) -> &'static str {
    match change {
        file_types::ChangeType::Added => "A",
        file_types::ChangeType::Deleted => "D",
        file_types::ChangeType::Modified => "M",
        file_types::ChangeType::Moved => "R",
        file_types::ChangeType::Untracked => "A",
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
    set_directory: Option<loom::SetState<Option<Rc<DirectorySelection>>>>,
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
        Node::File { file, .. } => {
            if let Some(set_directory) = set_directory {
                set_directory(&|_| None);
            }
            open_file(file, set_file);
        }
    }
}

fn select_node(
    node: &Node,
    files: &[File],
    set_file: Option<loom::SetState<Option<Rc<File>>>>,
    set_directory: Option<loom::SetState<Option<Rc<DirectorySelection>>>>,
) {
    match node {
        Node::Heading { .. } => {
            if let Some(set_directory) = set_directory {
                set_directory(&|_| None);
            }
        }
        Node::Directory { path, revs, .. } => {
            if let Some(set_file) = set_file {
                set_file(&|_| None);
            }
            if let Some(set_directory) = set_directory {
                let directory = Rc::new(DirectorySelection {
                    path: path.clone(),
                    revs: revs.clone(),
                    files: Rc::new(files_under_directory(path, revs, files)),
                });
                set_directory(&move |_| Some(Rc::clone(&directory)));
            }
        }
        Node::File { file, .. } => {
            if let Some(set_directory) = set_directory {
                set_directory(&|_| None);
            }
            open_file(file, set_file);
        }
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

fn files_under_directory(path: &RepoPath, revs: &Revs, files: &[File]) -> Vec<File> {
    let prefix = format!("{}/", path.as_str());
    files
        .iter()
        .filter(|file| file.revs() == revs.clone() && file.path().as_str().starts_with(&prefix))
        .cloned()
        .collect()
}

fn matching_selected_directory(
    selected: Option<&Rc<DirectorySelection>>,
    files: &[File],
) -> Option<Rc<DirectorySelection>> {
    let selected = selected?;
    let matching_files = files_under_directory(&selected.path, &selected.revs, files);
    if matching_files.is_empty() {
        return None;
    }
    Some(Rc::new(DirectorySelection {
        path: selected.path.clone(),
        revs: selected.revs.clone(),
        files: Rc::new(matching_files),
    }))
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
