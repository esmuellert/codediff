//! Virtualized commit graph list.

pub mod build;
mod entry;

use std::rc::Rc;

use loom::{
    Basis, Bubble, Column, ColumnProps, Layout, Listeners, Node as LoomNode, Scope, Scroll,
    ScrollOffset, ScrollProps, Text, TextProps, component, rsx, use_context, use_effect, use_exit,
    use_measure, use_memo, use_ref, use_scroll, use_state,
};

use self::build::{SymbolSet, graph};
use self::entry::{Entry, EntryProps};
use super::context::Ui;
use crate::Commit;
use crate::keybindings::Action;

const SCROLLOFF: u32 = 3;

#[component]
pub fn CommitGraph(
    scope: &mut Scope,
    commits: Rc<Vec<Commit>>,
    on_select: Option<Rc<dyn Fn(&Commit)>>,
    on_open: Option<Rc<dyn Fn(&Commit)>>,
) -> LoomNode {
    let ctx = use_context::<Ui>(scope);
    let theme = &ctx.theme;
    let base = theme.normal;
    let symbol_set = if ctx.branch_drawing {
        SymbolSet::BranchDrawing
    } else {
        SymbolSet::BoxDrawing
    };

    let commits_for_graph = Rc::clone(commits);
    let built = use_memo(scope, Rc::clone(commits), move || graph(&commits_for_graph));

    let (selected_line, set_selected_line) = use_state(scope, || 0u32);
    let commit_count = commits.len() as u32;
    let (node_ref, size) = use_measure(scope);
    let (view, scroll) = use_scroll(scope, || ScrollOffset::ZERO);
    let top_line = view.clamped_offset().y;
    let visible_start = top_line.min(commit_count.saturating_sub(1));
    let visible_count = u32::from(size.height).saturating_add(4);

    // Preserve selection on the same commit OID when `commits` reloads or grows.
    let prev_commits = use_ref(scope, || Rc::clone(commits));
    let saved_oid = use_ref(scope, || None::<String>);
    let active_line = if !Rc::ptr_eq(commits, &*prev_commits.current()) {
        saved_oid
            .current()
            .as_deref()
            .and_then(|oid| commits.iter().position(|commit| commit.oid.as_str() == oid))
            .map(|pos| pos as u32)
            .unwrap_or_else(|| selected_line.min(commit_count.saturating_sub(1)))
    } else {
        selected_line.min(commit_count.saturating_sub(1))
    };
    *saved_oid.current() = commits
        .get(active_line as usize)
        .map(|commit| commit.oid.as_str().to_owned());

    let commits_for_anchor = Rc::clone(commits);
    use_effect(scope, Rc::clone(commits), move || {
        if !Rc::ptr_eq(&commits_for_anchor, &*prev_commits.current()) {
            *prev_commits.current() = Rc::clone(&commits_for_anchor);
            if active_line != selected_line {
                set_selected_line.set(active_line);
            }
        }
    });

    if commits.is_empty() {
        return rsx! {
            Column {
                ref: Some(node_ref),
                focusable: true,
                auto_focus: true,
                layout: Layout { grow: 1, min_width: 8, fill: Some(base), ..Default::default() },
                ..,
                Text {
                    text: Rc::from("  No commits"),
                    style: base.fg(theme.tree.count),
                    layout: Layout { basis: Basis::Length(1), shrink: 0, ..Default::default() },
                    ..
                }
            }
        };
    }

    let selected_oid = commits
        .get(active_line as usize)
        .map(|commit| commit.oid.as_str());

    let entries: Vec<LoomNode> = commits
        .iter()
        .skip(visible_start as usize)
        .take(visible_count as usize)
        .enumerate()
        .map(|(offset, commit)| {
            let line = visible_start as usize + offset;
            let cells = built.cells(line, commits, selected_oid, symbol_set);
            rsx! {
                Entry {
                    key: line,
                    commit: commit.clone(),
                    cells,
                    selected: line as u32 == active_line,
                }
            }
        })
        .collect();

    let commits_ref = use_ref(scope, || Rc::clone(commits));
    *commits_ref.current() = Rc::clone(commits);
    let on_select_ref = use_ref(scope, || None::<Rc<dyn Fn(&Commit)>>);
    *on_select_ref.current() = on_select.as_ref().map(Rc::clone);
    let on_open_ref = use_ref(scope, || None::<Rc<dyn Fn(&Commit)>>);
    *on_open_ref.current() = on_open.as_ref().map(Rc::clone);

    let keybindings = Rc::clone(&ctx.keybindings);
    let exit = use_exit(scope);

    let listeners = Listeners::new()
        .on_key(move |k| match k {
            k if keybindings.matches(Action::MoveDown, k) => {
                set_selected_line.update(|line| {
                    let prev = *line;
                    let next = prev.saturating_add(1).min(commit_count.saturating_sub(1));
                    scroll.keep_y_visible(next, SCROLLOFF);
                    if next != prev
                        && let Some(callback) = on_select_ref.current().as_ref()
                        && let Some(commit) = commits_ref.current().get(next as usize)
                    {
                        callback(commit);
                    }
                    *line = next;
                });
                Bubble::Stop
            }
            k if keybindings.matches(Action::MoveUp, k) => {
                set_selected_line.update(|line| {
                    let prev = *line;
                    let next = prev.saturating_sub(1);
                    scroll.keep_y_visible(next, SCROLLOFF);
                    if next != prev
                        && let Some(callback) = on_select_ref.current().as_ref()
                        && let Some(commit) = commits_ref.current().get(next as usize)
                    {
                        callback(commit);
                    }
                    *line = next;
                });
                Bubble::Stop
            }
            k if keybindings.matches(Action::Start, k) => {
                set_selected_line.update(|line| {
                    let prev = *line;
                    let next = 0;
                    scroll.keep_y_visible(next, 0);
                    if next != prev
                        && let Some(callback) = on_select_ref.current().as_ref()
                        && let Some(commit) = commits_ref.current().get(next as usize)
                    {
                        callback(commit);
                    }
                    *line = next;
                });
                Bubble::Stop
            }
            k if keybindings.matches(Action::End, k) => {
                set_selected_line.update(|line| {
                    let prev = *line;
                    let next = commit_count.saturating_sub(1);
                    scroll.keep_y_visible(next, 0);
                    if next != prev
                        && let Some(callback) = on_select_ref.current().as_ref()
                        && let Some(commit) = commits_ref.current().get(next as usize)
                    {
                        callback(commit);
                    }
                    *line = next;
                });
                Bubble::Stop
            }
            k if keybindings.matches(Action::Open, k) => {
                if let Some(callback) = on_open_ref.current().as_ref()
                    && let Some(commit) = commits_ref.current().get(active_line as usize)
                {
                    callback(commit);
                }
                Bubble::Stop
            }
            k if keybindings.matches(Action::Quit, k) => {
                exit();
                Bubble::Stop
            }
            k if keybindings.matches(Action::FocusNext, k) => {
                loom::focus_next();
                Bubble::Stop
            }
            k if keybindings.matches(Action::FocusPrevious, k) => {
                loom::focus_previous();
                Bubble::Stop
            }
            _ => Bubble::Continue,
        })
        .on_mouse_down(move |mouse| {
            let next = (top_line + u32::from(mouse.local.y)).min(commit_count.saturating_sub(1));
            let changed = next != active_line;
            set_selected_line.set(next);
            if changed
                && let Some(callback) = on_select_ref.current().as_ref()
                && let Some(commit) = commits_ref.current().get(next as usize)
            {
                callback(commit);
            }
            Bubble::Stop
        });

    rsx! {
        Column {
            ref: Some(node_ref),
            focusable: true,
            auto_focus: true,
            listeners,
            layout: Layout { grow: 1, min_width: 8, fill: Some(base), ..Default::default() },
            ..,
            Scroll {
                view,
                handle: Some(scroll),
                horizontal: false,
                vertical: true,
                wheel_step: 3,
                content_height: Some(commit_count),
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
