//! Revision picker with `/` search filtering, pinned `Working tree`, and
//! scroll/selection restoration on search exit.

mod build;
pub mod commit_graph;
mod header;
mod worktree;

use std::rc::Rc;

use crokey::crossterm::event::{KeyCode, KeyModifiers, MouseButton};
use crokey::{KeyCombination, OneToThree, key};
use file_types::Rev;
use loom::{
    Basis, Bubble, Column, ColumnProps, Divider, DividerProps, Layout, Listeners, Node as LoomNode,
    Scope, Scroll, ScrollHandle, ScrollOffset, ScrollProps, Text, TextProps, component, rsx,
    use_context, use_exit, use_layout_effect, use_measure, use_memo, use_scroll, use_state,
};

use self::build::{DisplayRow, RevisionList, build_list};
use self::commit_graph::build::{SymbolSet, graph};
use self::commit_graph::entry::{Entry, EntryProps};
use self::header::header;
use self::worktree::{WorktreeRow, WorktreeRowProps, worktree_indent};
use super::context::Ui;
use crate::Commit;
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
    pending_initial: Option<Rev>,
}

impl PickerState {
    fn from_initial(
        initial: Option<&Rev>,
        commits: &[Commit],
        worktree_summary: Option<&str>,
    ) -> Self {
        let list = build_list(commits, worktree_summary, "");
        let Some(target) = initial else {
            return Self {
                browse_anchor: list.candidates.first().cloned(),
                ..Self::default()
            };
        };
        if let Some(index) = list.candidates.iter().position(|rev| rev == target) {
            Self {
                selected: index,
                browse_anchor: Some(target.clone()),
                ..Self::default()
            }
        } else {
            Self {
                browse_anchor: list.candidates.first().cloned(),
                pending_initial: Some(target.clone()),
                ..Self::default()
            }
        }
    }

    fn open_search(&mut self, active_selected: usize, top_line: u32) {
        self.pending_initial = None;
        if self.saved_browse.is_none() {
            self.saved_browse = Some(SavedBrowse {
                selected: active_selected,
                scroll_offset: ScrollOffset { x: 0, y: top_line },
            });
        }
        self.searching = true;
    }

    fn close_search(
        &mut self,
        current_list: &RevisionList,
        browse_list: RevisionList,
        scroll: ScrollHandle,
        on_select: Option<&Rc<dyn Fn(Rev)>>,
    ) {
        let prev_rev = current_list.candidates.get(self.selected).cloned();
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
        self.browse_anchor = next_rev.clone();
        notify_selected(on_select, prev_rev.as_ref(), next_rev.as_ref());
    }

    fn apply_query(
        &mut self,
        next_query: String,
        current_list: &RevisionList,
        next_list: RevisionList,
        top_line: u32,
        scroll: ScrollHandle,
        on_select: Option<&Rc<dyn Fn(Rev)>>,
    ) {
        self.pending_initial = None;
        let was_filtering = !self.query.trim().is_empty();
        let now_filtering = !next_query.trim().is_empty();
        let prev_rev = current_list.candidates.get(self.selected).cloned();

        if !was_filtering && now_filtering && self.saved_browse.is_none() {
            self.saved_browse = Some(SavedBrowse {
                selected: clamp_index(self.selected, current_list.candidates.len()),
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
            self.browse_anchor = next_rev.clone();
        }
        notify_selected(on_select, prev_rev.as_ref(), next_rev.as_ref());
    }

    fn move_by(
        &mut self,
        delta: isize,
        current_list: &RevisionList,
        scroll: ScrollHandle,
        on_select: Option<&Rc<dyn Fn(Rev)>>,
    ) {
        self.pending_initial = None;
        let total = current_list.candidates.len();
        if total == 0 {
            return;
        }
        let prev = self.selected.min(total - 1);
        let next = if delta < 0 {
            prev.saturating_sub(delta.unsigned_abs())
        } else {
            prev.saturating_add(delta as usize).min(total - 1)
        };
        if let Some(row_y) = current_list.row_for_candidate(next) {
            scroll.keep_y_visible(row_y as u32, SCROLLOFF);
        }
        self.selected = next;
        let next_rev = current_list.candidates.get(next);
        if self.query.trim().is_empty() {
            self.browse_anchor = next_rev.cloned();
        }
        notify_selected(on_select, current_list.candidates.get(prev), next_rev);
    }

    fn jump_to(
        &mut self,
        to_end: bool,
        current_list: &RevisionList,
        scroll: ScrollHandle,
        on_select: Option<&Rc<dyn Fn(Rev)>>,
    ) {
        self.pending_initial = None;
        let total = current_list.candidates.len();
        if total == 0 {
            return;
        }
        let prev = self.selected.min(total - 1);
        let next = if to_end { total - 1 } else { 0 };
        if let Some(row_y) = current_list.row_for_candidate(next) {
            scroll.keep_y_visible(row_y as u32, 0);
        }
        self.selected = next;
        let next_rev = current_list.candidates.get(next);
        if self.query.trim().is_empty() {
            self.browse_anchor = next_rev.cloned();
        }
        notify_selected(on_select, current_list.candidates.get(prev), next_rev);
    }
}

fn clamp_index(index: usize, len: usize) -> usize {
    if len == 0 { 0 } else { index.min(len - 1) }
}

fn notify_selected(on_select: Option<&Rc<dyn Fn(Rev)>>, prev: Option<&Rev>, next: Option<&Rev>) {
    if prev != next
        && let Some(next_rev) = next
        && let Some(callback) = on_select
    {
        callback(next_rev.clone());
    }
}

#[component]
pub fn RevisionPicker(
    scope: &mut Scope,
    commits: Rc<Vec<Commit>>,
    worktree_summary: Option<Rc<str>>,
    initial: Option<Rev>,
    on_select: Option<Rc<dyn Fn(Rev)>>,
    on_open: Option<Rc<dyn Fn(Rev)>>,
    on_close: Option<Rc<dyn Fn()>>,
) -> LoomNode {
    let ctx = use_context::<Ui>(scope);
    let theme = &ctx.theme;
    let base = theme.normal;
    let symbol_set = if ctx.branch_drawing {
        SymbolSet::BranchDrawing
    } else {
        SymbolSet::BoxDrawing
    };

    let initial_for_state = initial.clone();
    let commits_for_init = Rc::clone(commits);
    let summary_for_init = worktree_summary.clone();
    let (picker, set_picker) = use_state(scope, move || {
        PickerState::from_initial(
            initial_for_state.as_ref(),
            &commits_for_init,
            summary_for_init.as_deref(),
        )
    });
    let (initial_scrolled, set_initial_scrolled) = use_state(scope, || initial.is_none());

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
    let anchor_index = if !filtering && picker.pending_initial.is_none() {
        picker
            .browse_anchor
            .as_ref()
            .and_then(|anchor| list.candidates.iter().position(|rev| rev == anchor))
    } else {
        None
    };
    let active_selected =
        anchor_index.unwrap_or_else(|| clamp_index(picker.selected, list.candidates.len()));

    let selected_rev = list.candidates.get(active_selected).cloned();
    let selected_oid = match &selected_rev {
        Some(Rev::Commit(oid)) => Some(oid.as_str()),
        _ => None,
    };

    let row_count = list.rows.len() as u32;
    let (node_ref, size) = use_measure(scope);
    let (view, scroll) = use_scroll(scope, || ScrollOffset::ZERO);
    let top_line = view.clamped_offset().y;
    let visible_start = top_line.min(row_count.saturating_sub(1));
    let visible_count = u32::from(size.height).saturating_add(4);

    let resolved_async_initial = picker
        .pending_initial
        .as_ref()
        .and_then(|target| list.candidates.iter().position(|rev| rev == target));
    let needs_mount_scroll = !initial_scrolled && picker.pending_initial.is_none();
    let anchor_shifted = anchor_index.is_some_and(|index| index != picker.selected);
    let list_for_initial = Rc::clone(&list);
    use_layout_effect(
        scope,
        (
            resolved_async_initial,
            needs_mount_scroll,
            anchor_shifted,
            size.height,
            row_count,
        ),
        move || {
            if let Some(index) = resolved_async_initial {
                let target_rev = list_for_initial.candidates.get(index).cloned();
                set_picker.update(|state| {
                    if state.pending_initial.is_some() && !state.searching && state.query.is_empty()
                    {
                        state.selected = index;
                        state.browse_anchor = target_rev;
                        state.pending_initial = None;
                        if let Some(row_y) = list_for_initial.row_for_candidate(index) {
                            scroll.keep_y_visible(row_y as u32, SCROLLOFF);
                        }
                    }
                });
                set_initial_scrolled.set(true);
            } else if anchor_shifted {
                set_picker.update(|state| {
                    if state.pending_initial.is_none() && state.query.trim().is_empty() {
                        state.selected = active_selected;
                    }
                });
            } else if needs_mount_scroll && size.height > 0 && row_count > 0 {
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
                        built.cells(commit_index, commits, selected_oid, symbol_set)
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
    let on_select_for_keys = on_select.as_ref().map(Rc::clone);
    let on_open_for_keys = on_open.as_ref().map(Rc::clone);
    let on_close_for_keys = on_close.as_ref().map(Rc::clone);

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
            if state.searching {
                if pressed == key!(esc) {
                    let browse_list =
                        build_list(&commits_for_update, summary_for_update.as_deref(), "");
                    state.close_search(
                        &current_list,
                        browse_list,
                        scroll,
                        on_select_for_keys.as_ref(),
                    );
                    bubble = Bubble::Stop;
                    return;
                }
                if pressed == key!(enter) {
                    let index = clamp_index(state.selected, current_list.candidates.len());
                    opened = current_list.candidates.get(index).cloned();
                    bubble = Bubble::Stop;
                    return;
                }
                if pressed == key!(down) || pressed == key!(ctrl - n) {
                    state.move_by(1, &current_list, scroll, on_select_for_keys.as_ref());
                    bubble = Bubble::Stop;
                    return;
                }
                if pressed == key!(up) || pressed == key!(ctrl - p) {
                    state.move_by(-1, &current_list, scroll, on_select_for_keys.as_ref());
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
                            &current_list,
                            next_list,
                            top_line,
                            scroll,
                            on_select_for_keys.as_ref(),
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
                    state.apply_query(
                        next,
                        &current_list,
                        next_list,
                        top_line,
                        scroll,
                        on_select_for_keys.as_ref(),
                    );
                    bubble = Bubble::Stop;
                }
                return;
            }

            match pressed {
                k if k == key!('/') => {
                    let current = clamp_index(state.selected, current_list.candidates.len());
                    state.open_search(current, top_line);
                    bubble = Bubble::Stop;
                }
                k if k == key!(esc) => {
                    if !state.query.is_empty() {
                        let browse_list =
                            build_list(&commits_for_update, summary_for_update.as_deref(), "");
                        state.close_search(
                            &current_list,
                            browse_list,
                            scroll,
                            on_select_for_keys.as_ref(),
                        );
                        bubble = Bubble::Stop;
                    } else if on_close_for_keys.is_some() {
                        should_close = true;
                        bubble = Bubble::Stop;
                    }
                }
                k if keybindings.matches(Action::MoveDown, k) => {
                    state.move_by(1, &current_list, scroll, on_select_for_keys.as_ref());
                    bubble = Bubble::Stop;
                }
                k if keybindings.matches(Action::MoveUp, k) => {
                    state.move_by(-1, &current_list, scroll, on_select_for_keys.as_ref());
                    bubble = Bubble::Stop;
                }
                k if keybindings.matches(Action::Start, k) => {
                    state.jump_to(false, &current_list, scroll, on_select_for_keys.as_ref());
                    bubble = Bubble::Stop;
                }
                k if keybindings.matches(Action::End, k) => {
                    state.jump_to(true, &current_list, scroll, on_select_for_keys.as_ref());
                    bubble = Bubble::Stop;
                }
                k if keybindings.matches(Action::Open, k) => {
                    let index = clamp_index(state.selected, current_list.candidates.len());
                    opened = current_list.candidates.get(index).cloned();
                    bubble = Bubble::Stop;
                }
                k if keybindings.matches(Action::Quit, k) => {
                    if on_close_for_keys.is_some() {
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

        if let Some(rev) = opened
            && let Some(callback) = on_open_for_keys.as_ref()
        {
            callback(rev);
        }
        if should_close && let Some(callback) = on_close_for_keys.as_ref() {
            callback();
        }
        if should_quit {
            exit();
        }
        bubble
    });

    let list_for_mouse = Rc::clone(&list);
    let on_select_for_mouse = on_select.as_ref().map(Rc::clone);
    let list_mouse_listeners = Listeners::new().on_mouse_down(move |mouse| {
        if mouse.button != Some(MouseButton::Left) {
            return Bubble::Continue;
        }
        let row_index = (top_line + u32::from(mouse.local.y)) as usize;
        let Some(candidate) = list_for_mouse.candidate_at_row(row_index) else {
            return Bubble::Stop;
        };
        set_picker.update(|state| {
            state.pending_initial = None;
            let prev = clamp_index(state.selected, list_for_mouse.candidates.len());
            state.selected = candidate;
            let next_rev = list_for_mouse.candidates.get(candidate);
            if state.query.trim().is_empty() {
                state.browse_anchor = next_rev.cloned();
            }
            notify_selected(
                on_select_for_mouse.as_ref(),
                list_for_mouse.candidates.get(prev),
                next_rev,
            );
        });
        Bubble::Stop
    });

    let commits_for_header = Rc::clone(commits);
    let summary_for_header = worktree_summary.clone();
    let list_for_header = Rc::clone(&list);
    let on_select_for_header = on_select.as_ref().map(Rc::clone);
    let header_node = header(picker.searching, &picker.query, theme, move || {
        set_picker.update(|state| {
            if state.searching {
                let browse_list =
                    build_list(&commits_for_header, summary_for_header.as_deref(), "");
                state.close_search(
                    &list_for_header,
                    browse_list,
                    scroll,
                    on_select_for_header.as_ref(),
                );
            } else {
                let current = clamp_index(state.selected, list_for_header.candidates.len());
                state.open_search(current, top_line);
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
