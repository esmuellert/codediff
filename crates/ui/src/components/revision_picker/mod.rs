//! Revision picker with `/` search filtering, pinned `Working tree`, and
//! scroll/selection restoration on search exit.

mod build;
pub mod commit_graph;
mod commits;
mod header;
mod worktree;
mod worktree_summary;

use std::rc::Rc;

use crokey::crossterm::event::{KeyCode, KeyModifiers, MouseButton};
use crokey::{KeyCombination, OneToThree, key};
use file_types::Rev;
use loom::{
    Basis, Bubble, Column, ColumnProps, Divider, DividerProps, Layout, Listeners, Node as LoomNode,
    Scope, Scroll, ScrollHandle, ScrollOffset, ScrollProps, Text, TextProps, component, rsx,
    use_context, use_exit, use_layout_effect, use_measure, use_memo, use_ref, use_scroll,
    use_state,
};

use self::build::{DisplayRow, RevisionList, build_list};
use self::commit_graph::build::{SymbolSet, graph};
use self::commit_graph::entry::{Entry, EntryProps};
use self::commits::use_commits;
use self::header::header;
use self::worktree::{WorktreeRow, WorktreeRowProps, worktree_indent};
use self::worktree_summary::use_worktree_summary;
use super::context::Ui;
use super::revisions::{RevisionSide, apply_picked_revision, initial_for_side};
use crate::keybindings::Action;

const SCROLLOFF: u32 = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SavedBrowse {
    selected: usize,
    scroll_offset: ScrollOffset,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct PickerState {
    searching: bool,
    query: String,
    selected: usize,
    browse_anchor: Option<Rev>,
    saved_browse: Option<SavedBrowse>,
}

impl PickerState {
    fn active_index(&self, initial: Option<&Rev>, current_list: &RevisionList) -> usize {
        if self.query.trim().is_empty()
            && let Some(target) = self.browse_anchor.as_ref().or(initial)
            && let Some(index) = current_list.candidates.iter().position(|rev| rev == target)
        {
            return index;
        }
        clamp_index(self.selected, current_list.candidates.len())
    }

    fn open_search(&mut self, initial: Option<&Rev>, current_list: &RevisionList, top_line: u32) {
        let current = self.active_index(initial, current_list);
        self.selected = current;
        if self.browse_anchor.is_none() {
            self.browse_anchor = current_list.candidates.get(current).cloned();
        }
        if self.saved_browse.is_none() {
            self.saved_browse = Some(SavedBrowse {
                selected: current,
                scroll_offset: ScrollOffset { x: 0, y: top_line },
            });
        }
        self.searching = true;
    }

    fn close_search(&mut self, browse_list: RevisionList, scroll: ScrollHandle) {
        let restored = if let Some(saved) = self.saved_browse.take() {
            scroll.scroll_to(saved.scroll_offset);
            clamp_index(saved.selected, browse_list.candidates.len())
        } else {
            clamp_index(self.selected, browse_list.candidates.len())
        };
        let next_rev = browse_list.candidates.get(restored).cloned();
        self.searching = false;
        self.query.clear();
        self.selected = restored;
        self.browse_anchor = next_rev;
    }

    fn apply_query(
        &mut self,
        next_query: String,
        initial: Option<&Rev>,
        current_list: &RevisionList,
        next_list: RevisionList,
        top_line: u32,
        scroll: ScrollHandle,
    ) {
        let was_filtering = !self.query.trim().is_empty();
        let now_filtering = !next_query.trim().is_empty();

        if !was_filtering && now_filtering && self.saved_browse.is_none() {
            let current = self.active_index(initial, current_list);
            self.saved_browse = Some(SavedBrowse {
                selected: current,
                scroll_offset: ScrollOffset { x: 0, y: top_line },
            });
        }

        let next_selected = if !now_filtering {
            if let Some(saved) = self.saved_browse.take() {
                scroll.scroll_to(saved.scroll_offset);
                clamp_index(saved.selected, next_list.candidates.len())
            } else {
                clamp_index(self.selected, next_list.candidates.len())
            }
        } else {
            scroll.scroll_to(ScrollOffset::ZERO);
            0
        };

        let next_rev = next_list.candidates.get(next_selected).cloned();
        self.query = next_query;
        self.selected = next_selected;
        if !now_filtering {
            self.browse_anchor = next_rev;
        }
    }

    fn move_by(
        &mut self,
        delta: isize,
        initial: Option<&Rev>,
        current_list: &RevisionList,
        scroll: ScrollHandle,
    ) {
        let total = current_list.candidates.len();
        if total == 0 {
            return;
        }
        let prev = self.active_index(initial, current_list);
        let next = if delta < 0 {
            prev.saturating_sub(delta.unsigned_abs())
        } else {
            prev.saturating_add(delta as usize).min(total - 1)
        };
        if let Some(row_y) = current_list.row_for_candidate(next) {
            scroll.keep_y_visible(row_y as u32, SCROLLOFF);
        }
        self.selected = next;
        if self.query.trim().is_empty() {
            self.browse_anchor = current_list.candidates.get(next).cloned();
        }
    }

    fn jump_to(&mut self, to_end: bool, current_list: &RevisionList, scroll: ScrollHandle) {
        let total = current_list.candidates.len();
        if total == 0 {
            return;
        }
        let next = if to_end { total - 1 } else { 0 };
        if let Some(row_y) = current_list.row_for_candidate(next) {
            scroll.keep_y_visible(row_y as u32, 0);
        }
        self.selected = next;
        if self.query.trim().is_empty() {
            self.browse_anchor = current_list.candidates.get(next).cloned();
        }
    }
}

fn clamp_index(index: usize, len: usize) -> usize {
    if len == 0 { 0 } else { index.min(len - 1) }
}

#[component]
pub fn RevisionPicker(scope: &mut Scope) -> LoomNode {
    let ctx = use_context::<Ui>(scope);
    let theme = &ctx.theme;
    let base = theme.normal;
    let symbol_set = if ctx.branch_drawing {
        SymbolSet::BranchDrawing
    } else {
        SymbolSet::BoxDrawing
    };

    let commits = use_commits(scope, &ctx.repo, ctx.vcs.as_ref(), ctx.watcher.as_ref());
    let commits = &commits;
    let show_worktree = ctx.picking != Some(RevisionSide::Left);
    let loaded_summary = use_worktree_summary(
        scope,
        show_worktree,
        &ctx.repo,
        ctx.vcs.as_ref(),
        ctx.watcher.as_ref(),
    );
    let worktree_summary: Option<Rc<str>> =
        show_worktree.then(|| loaded_summary.unwrap_or_else(|| Rc::from("")));
    let initial = ctx
        .picking
        .and_then(|side| initial_for_side(side, &ctx.revision_range, commits));

    let (picker, set_picker) = use_state(scope, PickerState::default);
    let (initial_scrolled, set_initial_scrolled) = use_state(scope, || false);
    let last_side = use_ref(scope, || ctx.picking);
    if *last_side.current() != ctx.picking {
        *last_side.current() = ctx.picking;
        set_picker.set(PickerState::default());
        set_initial_scrolled.set(false);
    }

    let commits_for_graph = Rc::clone(commits);
    let built = use_memo(scope, Rc::clone(commits), move || graph(&commits_for_graph));

    let commits_for_list = Rc::clone(commits);
    let summary_for_list = worktree_summary.clone();
    let query_for_list = picker.query.clone();
    let list = use_memo(
        scope,
        (
            Rc::clone(commits),
            worktree_summary.clone(),
            picker.query.clone(),
        ),
        move || {
            build_list(
                &commits_for_list,
                summary_for_list.as_deref(),
                &query_for_list,
            )
        },
    );

    let filtering = !picker.query.trim().is_empty();
    let target = picker.browse_anchor.as_ref().or(initial.as_ref());
    let anchor_index = if !filtering {
        target.and_then(|rev| list.candidates.iter().position(|c| c == rev))
    } else {
        None
    };
    let active_selected =
        anchor_index.unwrap_or_else(|| clamp_index(picker.selected, list.candidates.len()));

    let selected_rev = list.candidates.get(active_selected).cloned();
    let selected_hash = match &selected_rev {
        Some(Rev::Commit(hash)) => Some(hash.as_str()),
        _ => None,
    };

    let row_count = list.rows.len() as u32;
    let (node_ref, size) = use_measure(scope);
    use_layout_effect(scope, (), move || {
        if let Some(handle) = *node_ref.current() {
            handle.focus();
        }
    });
    let (view, scroll) = use_scroll(scope, || ScrollOffset::ZERO);
    let top_line = view.clamped_offset().y;
    let visible_start = top_line.min(row_count.saturating_sub(1));
    let visible_count = u32::from(size.height).saturating_add(4);

    let has_commits = !commits.is_empty();
    let anchor_shifted = picker.browse_anchor.is_some()
        && anchor_index.is_some_and(|index| index != picker.selected);
    let list_for_initial = Rc::clone(&list);
    use_layout_effect(
        scope,
        (
            has_commits,
            initial_scrolled,
            anchor_shifted,
            size.height,
            row_count,
        ),
        move || {
            if anchor_shifted {
                set_picker.update(|state| {
                    if state.query.trim().is_empty() {
                        state.selected = active_selected;
                    }
                });
            }
            if has_commits && !initial_scrolled && size.height > 0 && row_count > 0 {
                if let Some(row_y) = list_for_initial.row_for_candidate(active_selected) {
                    scroll.keep_y_visible(row_y as u32, SCROLLOFF);
                }
                set_initial_scrolled.set(true);
            }
        },
    );

    let indent = worktree_indent(size.width, built.lane_count(), filtering);
    let summary_text = worktree_summary.clone().unwrap_or_else(|| Rc::from(""));

    let entries: Vec<LoomNode> = list
        .rows
        .iter()
        .skip(visible_start as usize)
        .take(visible_count as usize)
        .enumerate()
        .map(|(offset, row)| {
            let line = visible_start as usize + offset;
            match *row {
                DisplayRow::Worktree { candidate_index } => rsx! {
                    WorktreeRow {
                        key: line,
                        summary: Rc::clone(&summary_text),
                        indent,
                        selected: candidate_index == active_selected,
                    }
                },
                DisplayRow::Separator => rsx! {
                    Divider {
                        key: line,
                        symbol: "─",
                        style: theme.divider,
                        layout: Layout {
                            basis: Basis::Length(1),
                            shrink: 0,
                            ..Default::default()
                        },
                        ..
                    }
                },
                DisplayRow::Commit {
                    commit_index,
                    candidate_index,
                } => {
                    let row_selected = candidate_index == active_selected;
                    let cells = if filtering {
                        Rc::from([])
                    } else {
                        built.cells(commit_index, commits, selected_hash, symbol_set)
                    };
                    rsx! {
                        Entry {
                            key: line,
                            commit: commits[commit_index].clone(),
                            cells,
                            selected: row_selected,
                        }
                    }
                }
            }
        })
        .collect();

    let keybindings = Rc::clone(&ctx.keybindings);
    let exit = use_exit(scope);

    let commits_for_keys = Rc::clone(commits);
    let summary_for_keys = worktree_summary.clone();
    let list_for_keys = Rc::clone(&list);
    let rendered_query_for_keys = picker.query.clone();
    let picking = ctx.picking;
    let set_picking = ctx.set_picking;
    let set_revision_range = ctx.set_revision_range;
    let revision_range_for_keys = ctx.revision_range.clone();
    let commits_for_open = Rc::clone(commits);
    let initial_for_keys = initial.clone();
    let can_close = picking.is_some() && set_picking.is_some();

    let list_for_state = move |query: &str| -> Rc<RevisionList> {
        if query == rendered_query_for_keys {
            Rc::clone(&list_for_keys)
        } else {
            Rc::new(build_list(
                &commits_for_keys,
                summary_for_keys.as_deref(),
                query,
            ))
        }
    };

    let commits_for_update = Rc::clone(commits);
    let summary_for_update = worktree_summary.clone();
    let key_listeners = Listeners::new().on_key(move |pressed| {
        let mut bubble = Bubble::Continue;
        let mut opened: Option<Rev> = None;
        let mut should_close = false;
        let mut should_quit = false;

        set_picker.update(|state| {
            let current_list = list_for_state(&state.query);
            let init_ref = initial_for_keys.as_ref();
            if state.searching {
                if pressed == key!(esc) {
                    let browse_list =
                        build_list(&commits_for_update, summary_for_update.as_deref(), "");
                    state.close_search(browse_list, scroll);
                    bubble = Bubble::Stop;
                    return;
                }
                if pressed == key!(enter) {
                    let index = state.active_index(init_ref, &current_list);
                    opened = current_list.candidates.get(index).cloned();
                    bubble = Bubble::Stop;
                    return;
                }
                if pressed == key!(down) || pressed == key!(ctrl - n) {
                    state.move_by(1, init_ref, &current_list, scroll);
                    bubble = Bubble::Stop;
                    return;
                }
                if pressed == key!(up) || pressed == key!(ctrl - p) {
                    state.move_by(-1, init_ref, &current_list, scroll);
                    bubble = Bubble::Stop;
                    return;
                }
                if pressed == key!(backspace) {
                    let mut next = state.query.clone();
                    if next.pop().is_some() {
                        let next_list =
                            build_list(&commits_for_update, summary_for_update.as_deref(), &next);
                        state.apply_query(
                            next,
                            init_ref,
                            &current_list,
                            next_list,
                            top_line,
                            scroll,
                        );
                    }
                    bubble = Bubble::Stop;
                    return;
                }
                if let Some(ch) = plain_character(pressed) {
                    let mut next = state.query.clone();
                    next.push(ch);
                    let next_list =
                        build_list(&commits_for_update, summary_for_update.as_deref(), &next);
                    state.apply_query(next, init_ref, &current_list, next_list, top_line, scroll);
                    bubble = Bubble::Stop;
                }
                return;
            }

            match pressed {
                k if k == key!('/') => {
                    state.open_search(init_ref, &current_list, top_line);
                    bubble = Bubble::Stop;
                }
                k if k == key!(esc) => {
                    if !state.query.is_empty() {
                        let browse_list =
                            build_list(&commits_for_update, summary_for_update.as_deref(), "");
                        state.close_search(browse_list, scroll);
                        bubble = Bubble::Stop;
                    } else if can_close {
                        should_close = true;
                        bubble = Bubble::Stop;
                    }
                }
                k if keybindings.matches(Action::MoveDown, k) => {
                    state.move_by(1, init_ref, &current_list, scroll);
                    bubble = Bubble::Stop;
                }
                k if keybindings.matches(Action::MoveUp, k) => {
                    state.move_by(-1, init_ref, &current_list, scroll);
                    bubble = Bubble::Stop;
                }
                k if keybindings.matches(Action::Start, k) => {
                    state.jump_to(false, &current_list, scroll);
                    bubble = Bubble::Stop;
                }
                k if keybindings.matches(Action::End, k) => {
                    state.jump_to(true, &current_list, scroll);
                    bubble = Bubble::Stop;
                }
                k if keybindings.matches(Action::Open, k) => {
                    let index = state.active_index(init_ref, &current_list);
                    opened = current_list.candidates.get(index).cloned();
                    bubble = Bubble::Stop;
                }
                k if keybindings.matches(Action::Quit, k) => {
                    if can_close {
                        should_close = true;
                    } else {
                        should_quit = true;
                    }
                    bubble = Bubble::Stop;
                }
                k if keybindings.matches(Action::FocusNext, k) => {
                    loom::focus_next();
                    bubble = Bubble::Stop;
                }
                k if keybindings.matches(Action::FocusPrevious, k) => {
                    loom::focus_previous();
                    bubble = Bubble::Stop;
                }
                _ => {}
            }
        });

        if let Some(rev) = opened {
            if let (Some(side), Some(set_range)) = (picking, set_revision_range)
                && let Some(next_range) =
                    apply_picked_revision(side, rev, &revision_range_for_keys, &commits_for_open)
            {
                set_range.set(next_range);
            }
            if let Some(set_picking) = set_picking {
                set_picking.set(None);
            }
        }
        if should_close && let Some(set_picking) = set_picking {
            set_picking.set(None);
        }
        if should_quit {
            exit();
        }
        bubble
    });

    let list_for_mouse = Rc::clone(&list);
    let revision_range_for_mouse = ctx.revision_range.clone();
    let commits_for_mouse = Rc::clone(commits);
    let initial_for_mouse = initial.clone();
    let list_mouse_listeners = Listeners::new().on_mouse_down(move |mouse| {
        if mouse.button != Some(MouseButton::Left) {
            return Bubble::Continue;
        }
        let row_index = (top_line + u32::from(mouse.local.y)) as usize;
        let Some(candidate) = list_for_mouse.candidate_at_row(row_index) else {
            return Bubble::Stop;
        };
        let mut opened: Option<Rev> = None;
        set_picker.update(|state| {
            let prev = state.active_index(initial_for_mouse.as_ref(), &list_for_mouse);
            if candidate == prev {
                opened = list_for_mouse.candidates.get(candidate).cloned();
            } else {
                state.selected = candidate;
                if state.query.trim().is_empty() {
                    state.browse_anchor = list_for_mouse.candidates.get(candidate).cloned();
                }
            }
        });
        if let Some(rev) = opened {
            if let (Some(side), Some(set_range)) = (picking, set_revision_range)
                && let Some(next_range) =
                    apply_picked_revision(side, rev, &revision_range_for_mouse, &commits_for_mouse)
            {
                set_range.set(next_range);
            }
            if let Some(set_picking) = set_picking {
                set_picking.set(None);
            }
        }
        Bubble::Stop
    });

    let commits_for_header = Rc::clone(commits);
    let summary_for_header = worktree_summary.clone();
    let list_for_header = Rc::clone(&list);
    let initial_for_header = initial;
    let header_node = header(picker.searching, &picker.query, theme, move || {
        set_picker.update(|state| {
            if state.searching {
                let browse_list =
                    build_list(&commits_for_header, summary_for_header.as_deref(), "");
                state.close_search(browse_list, scroll);
            } else {
                state.open_search(initial_for_header.as_ref(), &list_for_header, top_line);
            }
        });
    });

    let body = if list.candidates.is_empty() {
        let empty_text = if filtering {
            " No revisions match."
        } else {
            " No commits"
        };
        rsx! {
            Column {
                key: 2u32,
                layout: Layout { grow: 1, fill: Some(base), ..Default::default() },
                ..,
                Text {
                    text: Rc::from(empty_text),
                    style: base.fg(theme.tree.count),
                    layout: Layout { basis: Basis::Length(1), shrink: 0, ..Default::default() },
                    ..
                }
            }
        }
    } else {
        rsx! {
            Scroll {
                key: 2u32,
                view,
                handle: Some(scroll),
                horizontal: false,
                vertical: true,
                wheel_step: 3,
                content_height: Some(row_count),
                content_offset: ScrollOffset { x: 0, y: visible_start },
                listeners: list_mouse_listeners,
                layout: Layout { grow: 1, fill: Some(base), ..Default::default() },
                ..,
                Column {
                    layout: Layout { fill: Some(base), ..Default::default() },
                    ..,
                    { entries }
                }
            }
        }
    };

    rsx! {
        Column {
            ref: Some(node_ref),
            focusable: true,
            auto_focus: true,
            listeners: key_listeners,
            layout: Layout { grow: 1, min_width: 8, fill: Some(base), ..Default::default() },
            ..,
            { header_node }
            { body }
        }
    }
}

fn plain_character(pressed: KeyCombination) -> Option<char> {
    let forbidden = KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SUPER;
    if pressed.modifiers.intersects(forbidden) {
        return None;
    }
    match pressed.codes {
        OneToThree::One(KeyCode::Char(character)) => Some(character),
        _ => None,
    }
}
