//! The file list.

use std::rc::Rc;

use crokey::key;
use file_types::{ChangeType, File, Stats};
use loom::{
    Bubble, Column, ColumnProps, Layout, Listeners, Node, Scope, component, rsx, use_context,
    use_memo, use_state, use_sync_external_store,
};
use ratatui::style::{Modifier, Style};

use super::context::{CursorContext, FileListStoreContext, ThemeContext, ViewLinesContext};
use super::entry::{Body, Entry, EntryProps, Indent, Run, Status, priority};
use crate::state::buffer::explorer::{Explorer as Model, NodeId, Tree, ViewLine, ViewMode};
use crate::theme::{Theme, icon};

/// The file list.
///
/// The model decides what the rows are; this decides what they look like.
/// Nothing hands the model in: the list worker fills the store, this
/// subscribes to it, and the groups and folds follow from what it read.
#[component]
pub fn Explorer(scope: &mut Scope, on_open: Rc<dyn Fn(File)>) -> Node {
    let theme = use_context::<ThemeContext>(scope);
    let view_lines = use_context::<ViewLinesContext>(scope);
    let cursor = use_context::<CursorContext>(scope);
    let store = use_context::<FileListStoreContext>(scope);
    let files = use_sync_external_store(scope, &store);

    // Nested or flat. The reader's choice, and nothing above this has any use
    // for it.
    let (tree_mode, set_tree_mode) = use_state(scope, || true);

    // What the files mean: groups, directories, folds. Rebuilt when the list
    // changes, and when the arrangement does — the mode decides what the
    // lines *are*, not just how they are drawn.
    let model = use_memo(scope, (files.clone(), tree_mode), || {
        let mut model = Model::new(files.to_vec());
        if !tree_mode {
            model.set_mode(ViewMode::List);
        }
        model
    });

    let opening = Rc::clone(&model);
    let on_open = Rc::clone(on_open);
    let listeners = Listeners::new().on_key(move |press| {
        if press == key!(t) {
            set_tree_mode(&|nested: bool| !nested);
            Bubble::Stop
        } else if press == key!(enter) {
            // A heading and a directory are not files, so there is nothing to
            // open on one.
            if let Some(file) = opening.file(cursor) {
                on_open(file.clone());
            }
            Bubble::Stop
        } else {
            Bubble::Continue
        }
    });

    let base = theme.normal;
    let rows: Vec<Node> = view_lines
        .clone()
        .filter_map(|line| model.view_line(line).map(|content| (line, content)))
        .map(|(line, content)| {
            let selected = line == cursor;
            let background = if selected { base.patch(theme.cursor_line) } else { base };

            let indent = Indent {
                lines: Rc::from(match model.nested_at(line) {
                    Some((tree, id)) => indent_of(tree, id),
                    // A heading is what the arrangement hangs from, not a line
                    // in it.
                    None => String::new(),
                }),
                style: base.fg(theme.tree.marker),
            };

            let (body, status) = match &content {
                ViewLine::Heading { name, files, stats } => {
                    heading(name, *files, *stats, &theme, background)
                }
                ViewLine::Directory { name, open } => directory(name, *open, &theme, background),
                ViewLine::File { name, file } => file_row(name, file, &theme, background),
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
        .collect();

    rsx! {
        Column {
            // How wide the list is, is the pane's business.
            layout: Layout { grow: 1, min_width: 8, fill: Some(base), ..Default::default() },
            listeners: listeners,
            ..,
            { rows }
        }
    }
}

/// The columns before a line's name.
///
/// A guide at a given depth means an ancestor at that depth has more children
/// after it; blank space where that ancestor was the last, so nothing runs
/// down beside nothing.
fn indent_of(tree: &Tree, id: NodeId) -> String {
    let node = tree.node(id);
    let mut levels = vec![if node.is_last { "└ " } else { "├ " }];
    let mut above = node.parent;
    while let Some(parent) = above {
        let parent = tree.node(parent);
        levels.push(if parent.is_last { "  " } else { "│ " });
        above = parent.parent;
    }
    levels.into_iter().rev().collect()
}

/// A heading: its name, how many files it holds, and their total.
///
/// Bold is applied here rather than stored in the theme: a heading is bold in
/// every theme, so it is structural.
fn heading(
    name: &str,
    files: usize,
    stats: Stats,
    theme: &Theme,
    background: Style,
) -> (Body, Option<Status>) {
    let count = background.fg(theme.tree.count);
    let mut runs = vec![Run::fixed(
        name,
        background.fg(theme.tree.heading).add_modifier(Modifier::BOLD),
    )];

    if stats.is_empty() {
        runs.push(Run::droppable(format!(" ({files})"), count, priority::FILES));
    } else {
        runs.push(Run::droppable(format!(" ({files} · "), count, priority::FILES));
        push_stats(&mut runs, stats, theme, background, priority::FILES);
        runs.push(Run::droppable(")", count, priority::FILES));
    }

    (Body { icon: None, runs: runs.into() }, None)
}

/// A directory: its icon and name, and nothing at the right-hand edge.
fn directory(name: &str, open: bool, theme: &Theme, background: Style) -> (Body, Option<Status>) {
    (
        Body {
            icon: Some(icon::folder(open)),
            runs: vec![Run::fixed(name, background.fg(theme.tree.directory))].into(),
        },
        None,
    )
}

/// A file: its icon, name, where it came from, what it gained, and what
/// happened.
fn file_row(name: &str, file: &File, theme: &Theme, background: Style) -> (Body, Option<Status>) {
    let mut runs = vec![Run::fixed(name, background.fg(theme.tree.name))];
    if let Some(previous) = file.previous_path() {
        runs.push(Run::droppable(
            format!(" ← {previous}"),
            background.fg(theme.tree.previous),
            priority::MOVED,
        ));
    }

    let mut status = Vec::new();
    // A file that gained and lost nothing says nothing, rather than `+0 -0` in
    // a column the eye is scanning.
    if let Some(stats) = file.get_stats().filter(|s| !s.is_empty()) {
        push_stats(&mut status, stats, theme, background, priority::COUNTS);
        // The space between the counts and the letter, which goes with them
        // rather than staying behind as a lone column.
        status.push(Run::droppable(" ", background.fg(theme.tree.name), priority::COUNTS));
    }

    let change = file.get_change_type();
    status.push(Run::fixed(
        letter(change),
        background.fg(theme.change.of(change)).add_modifier(Modifier::BOLD),
    ));

    // The name carries the whole path in the flat arrangement, and the lookup
    // drops the directories, so one call answers both.
    (
        Body { icon: Some(icon::file(name)), runs: runs.into() },
        Some(Status { runs: status.into() }),
    )
}

/// The `+4 -3` pair, with a side left out when it is zero.
fn push_stats(
    runs: &mut Vec<Run>,
    stats: Stats,
    theme: &Theme,
    background: Style,
    priority: u8,
) {
    if stats.added > 0 {
        runs.push(Run::droppable(
            format!("+{}", stats.added),
            background.fg(theme.change.gained),
            priority,
        ));
    }
    if stats.removed > 0 {
        let separator = if stats.added > 0 { " " } else { "" };
        runs.push(Run::droppable(
            format!("{separator}-{}", stats.removed),
            background.fg(theme.change.lost),
            priority,
        ));
    }
}

/// Git's letter for what happened.
///
/// A copy arrives as `Moved` and shows `R`, and a type change as `Modified`
/// and shows `M`. What a reviewer does about either is read the new content,
/// which is what those letters already promise.
pub fn letter(change: ChangeType) -> &'static str {
    match change {
        ChangeType::Added => "A",
        ChangeType::Deleted => "D",
        ChangeType::Modified => "M",
        ChangeType::Moved => "R",
        ChangeType::Untracked => "??",
        ChangeType::Conflicted => "!",
    }
}
