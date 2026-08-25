//! The file list.

pub mod model;
mod utils;

use std::rc::Rc;

use file_types::{ChangeType, File, Stats};
use loom::{
    Bubble, Column, ColumnProps, Layout, Listeners, Node, Scope, component, rsx, use_context,
    use_layout_effect, use_memo, use_ref, use_state, use_sync_external_store,
};
use ratatui::style::{Modifier, Style};

use self::model::{Content, Explorer as Model, FoldState, NodeId, Tree, ViewMode};
use super::context::{FileListStoreCtx, ObservedCtx, Ui};
use super::entry::{Body, Entry, EntryProps, Indent, Run, Status, priority};
use crate::input::{Action, BufferAction, KeymapType, Match, ViewAction, keymap};
use crate::theme::{Theme, icon};

/// The file list.
///
/// The model decides what the rows are; this decides what they look like.
/// The files come from the list store, and how they are arranged is this
/// component's own state, so the rows are worked out here rather than handed
/// over.
///
/// The pane above holds the cursor and the scroll, because a key that moves
/// them means the same thing in a diff and the status line counts them
/// either way. So a rebuilt list and a press both say where the reader now
/// is through the setter `App` left in `Observed`.
///
/// One prop, because only the session can reach the worker that reads a file.
#[component]
pub fn Explorer(scope: &mut Scope, on_open: Rc<dyn Fn(File)>) -> Node {
    let ctx = use_context::<Ui>(scope);
    let theme = &ctx.theme;
    // Where the repository is. Nothing on a row shows it yet.
    let _repo = &ctx.repo;
    let view_lines = &ctx.view_lines;
    let cursor = ctx.cursor;
    // The context carries the *focused* pane's file, so no file is the reader
    // being in the list — which is exactly when the list's own keys are its
    // own.
    let focused = ctx.file.is_none();
    let observed = use_context::<ObservedCtx>(scope);
    let store = use_context::<FileListStoreCtx>(scope);
    // The list worker fills the store; this subscribes rather than being
    // handed what it produced.
    let files = use_sync_external_store(scope, &store);

    // How the reader has arranged the list: whether it is nested or flat, and
    // which rows are folded. The rows themselves follow from these and the
    // store, so they are worked out rather than kept and changed in place.
    let (mode, set_mode) = use_state(scope, ViewMode::default);
    let (folds, set_folds) = use_state(scope, FoldState::default);
    let model = use_memo(scope, (files.clone(), mode, folds.clone()), || {
        Model::arranged(files.to_vec(), mode, &folds)
    });
    // The list the last render worked out, or nothing before there was one.
    // A rebuilt list renumbers every row, so the file the reader was on is
    // named in the old one and looked up in the new (D54).
    let previous = use_ref(scope, || None::<Rc<Model>>);
    let before = previous.current().clone();
    *previous.current() = Some(Rc::clone(&model));

    // The pane is told what it is now looking at, once per rebuild rather
    // than once per frame.
    //
    // Only by the list that was drawn: a screen too small for two panes
    // draws one of them on its own, and that one is another list of the same
    // files. The one left out has no rectangle, and a list nobody can see
    // has nothing to say about where the reader is.
    let node = use_ref(scope, || None::<loom::NodeHandle>);
    let arranged = Rc::clone(&model);
    let report = Rc::clone(&observed);
    use_layout_effect(scope, (files.clone(), mode, folds.clone()), move || {
        let Some(node) = *node.current() else { return };
        if node.area().is_empty() {
            return;
        }
        let landing = match &before {
            Some(before) if before.view_lines() > 0 => arranged.line_after(before, cursor),
            // No list before this one, so the pane's cursor is all there is
            // to go on. Where it starts is a heading, which opens nothing,
            // so the reader is put on the first row that does (D48).
            _ if arranged.file(cursor).is_some() => cursor,
            _ => arranged.first_file(),
        };
        report.place_in_list(arranged.view_lines(), landing);
    });

    // Folding a row. Answers whether there was anything to fold, so the
    // gesture that both folds and opens can tell which it did.
    let folding = Rc::clone(&model);
    let fold: Rc<dyn Fn(u32) -> bool> = Rc::new(move |line: u32| match folding.folded(line) {
        Some(next) => {
            set_folds(&move |_| next.clone());
            true
        }
        None => false,
    });

    // Opening a row, whether the gesture was a key or a click. A heading and
    // a directory have nothing to open, so they fold instead.
    let open_row: Rc<dyn Fn(u32)> = {
        let folding = Rc::clone(&fold);
        let held = Rc::clone(&model);
        let request = Rc::clone(on_open);
        Rc::new(move |line: u32| {
            if folding(line) {
                return;
            }
            if let Some(file) = held.file(line).cloned() {
                request(file);
            }
        })
    };

    // The list's own keys. Looked up rather than resolved: these are single
    // keys, so nothing here has to remember what came before — and the key
    // goes on up to the pane, which resolves it and clears whatever a key
    // clears.
    let flipping = Rc::clone(&model);
    let folding = Rc::clone(&fold);
    let opening = Rc::clone(&open_row);
    let keys = move |key: crokey::KeyCombination| {
        if !focused {
            return Bubble::Continue;
        }
        match keymap::lookup(KeymapType::Explorer, &[key.normalized()]) {
            Match::Exact(Action::Buffer(BufferAction::Toggle)) => {
                folding(cursor);
            }
            Match::Exact(Action::Buffer(BufferAction::ToggleViewMode)) => {
                // A change of shape renumbers every row, so the folds that
                // survive it are the ones that name a group rather than a
                // node in the list that is being thrown away.
                let next = flipping.fold_state().headings_only();
                set_mode(&|mode: ViewMode| mode.other());
                set_folds(&move |_| next.clone());
            }
            Match::Exact(Action::View(ViewAction::Open)) => opening(cursor),
            _ => {}
        }
        Bubble::Continue
    };

    // A press picks the row it landed on: the pane is told to put the cursor
    // there, and the row is opened or folded. The press goes on up, so that
    // the pane it landed in also takes the keys back.
    let top = view_lines.start;
    let rows_now = model.view_lines();
    let press = Rc::clone(&observed);
    let opening = Rc::clone(&open_row);
    let listeners = Listeners::new()
        .on_key(keys)
        .on_mouse_down(move |mouse| {
            let line = top + u32::from(mouse.local.y);
            if line < rows_now {
                press.place_in_list(rows_now, line);
                opening(line);
            }
            Bubble::Continue
        });

    let base = theme.normal;
    let rows: Vec<Node> = view_lines
        .clone()
        .filter_map(|line| model.content(line).map(|content| (line, content)))
        .map(|(line, content)| {
            let selected = line == cursor;
            let background = if selected { base.patch(theme.cursor_line) } else { base };

            let indent = Indent {
                lines: Rc::from(match model.nested_at(line) {
                    Some((tree, id)) => indent_of(tree, id),
                    // A heading has no tree node to describe.
                    None => String::new(),
                }),
                style: base.fg(theme.tree.marker),
            };

            let (body, status) = match &content {
                Content::Heading { name, files, stats } => {
                    heading(name, *files, *stats, theme, background)
                }
                Content::Directory { name, open } => directory(name, *open, theme, background),
                Content::File { name, file } => file_row(name, file, theme, background),
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
            ref: Some(node),
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
    // A file that gained and lost nothing says nothing.
    if let Some(stats) = file.get_stats().filter(|s| !s.is_empty()) {
        push_stats(&mut status, stats, theme, background, priority::COUNTS);
        // The space between the counts and the letter, which goes with them
        // The space goes with the counts.
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
