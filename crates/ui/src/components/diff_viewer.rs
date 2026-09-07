//! Loads the selected file and chooses its view.

use std::collections::HashMap;
use std::rc::Rc;

use align::{Alignment, ViewLine};
use file_types::{DiffType, DiffVersion, File, Rev};
use loom::{
    Bubble, Column, ColumnProps, Layout, Listeners, Node, Ref, Scope, SetState, component, rsx,
    use_context, use_effect, use_ref, use_state,
};
use pipeline::diff::DiffContent;

use super::context::{Context, Ui};
use super::inline::{Inline, InlineProps};
use super::side_by_side::{SideBySide, SideBySideProps};
use super::single_file::{SingleFile, SingleFileProps};
use super::welcome::Welcome;

/// The screen position of one two-sided diff.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ViewState {
    pub first_view_line: Option<ViewLine>,
    pub first_cell: u32,
}

/// Screen positions shared by a diff's layouts.
#[derive(Debug, Default)]
pub struct ViewStateHistory {
    entries: HashMap<String, ViewState>,
}

impl ViewStateHistory {
    pub fn load(&self, key: &str) -> ViewState {
        self.entries.get(key).copied().unwrap_or_default()
    }

    pub fn save(&mut self, key: &str, state: ViewState) {
        self.entries.insert(key.to_owned(), state);
    }
}

pub(crate) fn first_view_line(
    alignment: &Alignment,
    layout: DiffType,
    top: u32,
) -> Option<ViewLine> {
    alignment.view_lines_from(layout, top).next()
}

pub(crate) fn find_view_line_index(
    alignment: &Alignment,
    layout: DiffType,
    first: ViewLine,
) -> Option<u32> {
    if let Some(line) = first.modified.line()
        && let Some(top) = alignment.view_line_at(layout, DiffVersion::Modified, line)
    {
        return Some(top);
    }
    first
        .original
        .line()
        .and_then(|line| alignment.view_line_at(layout, DiffVersion::Original, line))
}

fn use_diff_content(scope: &mut Scope, ctx: &Context) -> Option<Rc<DiffContent>> {
    let (content, set_content) = use_state(scope, || None::<Rc<DiffContent>>);
    let selected_file = ctx.file.as_ref().map(Rc::clone);
    let file_for_request = selected_file.as_ref().map(Rc::clone);
    let diff_service = ctx.diff_service.as_ref().map(Rc::clone);
    let watcher_service = ctx.watcher_service.as_ref().map(Rc::clone);

    use_effect(scope, selected_file, move || {
        let (Some(requested_file), Some(diff_service)) = (file_for_request, diff_service) else {
            set_content(&|_| None);
            return;
        };
        let requested_file_for_response = Rc::clone(&requested_file);
        diff_service
            .get(&requested_file)
            .subscribe(move |response| {
                if response.file != *requested_file_for_response {
                    return;
                }
                let matching_content = response
                    .content
                    .ok()
                    .filter(|content| content.file() == &*requested_file_for_response)
                    .map(Rc::new);
                set_content(&move |_| matching_content.clone());
            });
        let Some(watcher_service) = watcher_service else {
            return;
        };
        let diff_service_to_refresh = Rc::clone(&diff_service);
        let selected_file_to_refresh = Rc::clone(&requested_file);
        watcher_service.changes().subscribe(move |refresh| {
            if refresh_affects_file(refresh, &selected_file_to_refresh) {
                diff_service_to_refresh.refresh(&selected_file_to_refresh);
            }
        });
    });

    content
}

fn sync_active_view_state(
    history: Ref<ViewStateHistory>,
    active_state: Ref<ViewState>,
    active_key: Ref<Option<String>>,
    content: Option<&DiffContent>,
) {
    let current_key = match content {
        Some(DiffContent::Diff(diff)) => Some(diff.file.path().as_str().to_owned()),
        _ => None,
    };
    let previous_key = active_key.current().clone();
    if previous_key.as_ref() == current_key.as_ref() {
        return;
    }

    if let Some(previous_key) = previous_key {
        let state = *active_state.current();
        history.current().save(&previous_key, state);
    }
    let state = current_key
        .as_deref()
        .map(|key| history.current().load(key))
        .unwrap_or_default();
    *active_state.current() = state;
    *active_key.current() = current_key;
}

fn layout_listener(is_diff: bool, set_layout: SetState<DiffType>) -> Listeners {
    Listeners::new().on_key(move |key| {
        if is_diff && key == crokey::key!(t) {
            set_layout(&|layout| layout.other());
            Bubble::Stop
        } else {
            Bubble::Continue
        }
    })
}

fn render_content_view(
    content: Option<Rc<DiffContent>>,
    layout: DiffType,
    view_state: Ref<ViewState>,
) -> Node {
    match content {
        Some(content) => match content.as_ref() {
            DiffContent::Diff(_) => {
                let content_id = Rc::as_ptr(&content) as usize;
                match layout {
                    DiffType::SideBySide => rsx! {
                        SideBySide {
                            key: content_id,
                            content: Rc::clone(&content),
                            view_state: view_state,
                            auto_focus: true,
                        }
                    },
                    DiffType::Inline => rsx! {
                        Inline {
                            key: content_id,
                            content: Rc::clone(&content),
                            view_state: view_state,
                            auto_focus: true,
                        }
                    },
                    DiffType::Single => unreachable!("DiffViewer's diff layout cannot be Single"),
                }
            }
            DiffContent::SingleFile(_) => {
                rsx! {
                    SingleFile {
                        content: Rc::clone(&content),
                    }
                }
            }
        },
        None => rsx! { Welcome {} },
    }
}

#[component]
pub fn DiffViewer(scope: &mut Scope) -> Node {
    let ctx = use_context::<Ui>(scope);
    let content = use_diff_content(scope, &ctx);
    let (layout, set_layout) = use_state(scope, || DiffType::SideBySide);
    let view_states = use_ref(scope, ViewStateHistory::default);
    let active_view_state = use_ref(scope, ViewState::default);
    let active_file_key = use_ref(scope, || None::<String>);
    sync_active_view_state(
        view_states,
        active_view_state,
        active_file_key,
        content.as_deref(),
    );

    let is_diff = matches!(content.as_deref(), Some(DiffContent::Diff(_)));
    let toggle = layout_listener(is_diff, set_layout);
    let content_view = render_content_view(content, layout, active_view_state);

    rsx! {
        Column {
            listeners: toggle,
            layout: Layout { grow: 1, ..Default::default() },
            ..,
            { content_view }
        }
    }
}

fn refresh_affects_file(refresh: watcher::Refresh, file: &File) -> bool {
    [DiffVersion::Original, DiffVersion::Modified]
        .into_iter()
        .any(|version| {
            file.path_of_version(version).is_some()
                && match file.rev(version) {
                    Rev::Worktree => refresh.worktree,
                    Rev::Index | Rev::Conflict(_) => refresh.index,
                    Rev::Commit(_) => false,
                }
        })
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use file_types::{Oid, RepoPath, Revs};

    use super::*;

    fn file(revs: Revs) -> File {
        File::unchanged_path(RepoPath::new("selected.rs", Path::new("/repo")), revs)
    }

    #[test]
    fn only_changes_to_a_present_mutable_side_refresh() {
        let worktree = file(Revs::worktree_against(Oid::new("abc")));
        let staged = file(Revs::new(Rev::Commit(Oid::new("abc")), Rev::Index));
        let deleted = File::deleted(
            RepoPath::new("selected.rs", Path::new("/repo")),
            Revs::worktree_against(Oid::new("abc")),
        );

        assert!(refresh_affects_file(
            watcher::Refresh {
                worktree: true,
                ..watcher::Refresh::default()
            },
            &worktree
        ));
        assert!(refresh_affects_file(
            watcher::Refresh {
                index: true,
                ..watcher::Refresh::default()
            },
            &staged
        ));
        assert!(!refresh_affects_file(
            watcher::Refresh {
                worktree: true,
                ..watcher::Refresh::default()
            },
            &staged
        ));
        assert!(!refresh_affects_file(
            watcher::Refresh {
                worktree: true,
                ..watcher::Refresh::default()
            },
            &deleted
        ));
    }
}
