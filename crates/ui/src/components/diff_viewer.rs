//! Loads the selected file or directory and chooses its view.

// The opened files are spelled out in full rather than hidden behind a name.
#![allow(clippy::type_complexity)]

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use file_types::{DiffType, DiffVersion, File, Rev};
use loom::{
    Bubble, Column, ColumnProps, Layout, Listeners, Node, Ref, Scope, SetState, Text, TextProps,
    component, rsx, use_context, use_effect, use_memo, use_ref, use_state,
};
use pipeline::diff::DiffContent;
use syntax::Span;

use super::context::{Context, Ui};
use super::diff_cards::{DiffCards, DiffCardsProps};
use super::diff_viewer_container::{DiffViewerContainer, DiffViewerContainerProps};
use crate::keybindings::Action;
use crate::view::terminal_lines::TerminalLine;

/// The semantic screen position shared by a file's layouts.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ViewState {
    pub(crate) first_terminal_line: Option<TerminalLine>,
    pub first_cell: u32,
}

/// Screen positions saved independently for each file.
#[derive(Debug, Default)]
pub struct ViewStateHistory {
    entries: HashMap<String, ViewState>,
}

impl ViewStateHistory {
    pub fn load(&self, key: &str) -> ViewState {
        self.entries.get(key).cloned().unwrap_or_default()
    }

    pub fn save(&mut self, key: &str, state: ViewState) {
        self.entries.insert(key.to_owned(), state);
    }
}

/// The selected file's diff. Selecting another file, or a change on disk,
/// opens it again; the previous result is dropped.
fn use_opened_file(
    scope: &mut Scope,
    ctx: &Context,
) -> Rc<
    Option<(
        Arc<Mutex<Option<Result<Arc<DiffContent>, String>>>>,
        Arc<Mutex<[Vec<Vec<Span>>; 2]>>,
    )>,
> {
    let selected = ctx
        .file
        .as_ref()
        .filter(|_| ctx.directory.is_none())
        .map(Rc::clone);
    let (refreshes, set_refreshes) = use_state(scope, || 0u32);
    let watcher_service = ctx.watcher_service.as_ref().map(Rc::clone);
    let file_to_refresh = selected.as_ref().map(Rc::clone);
    use_effect(scope, selected.clone(), move || {
        let (Some(file), Some(watcher_service)) = (file_to_refresh, watcher_service) else {
            return;
        };
        watcher_service.changes().subscribe(move |refresh| {
            if refresh_affects_file(refresh, &file) {
                set_refreshes(&|refreshes| refreshes + 1);
            }
        });
    });
    let pipeline = ctx.diff_service.as_ref().map(Rc::clone);
    let file_to_open = selected.as_ref().map(Rc::clone);
    use_memo(scope, (selected, refreshes), move || {
        Some(pipeline?.open((*file_to_open?).clone()))
    })
}

/// The files of the selected directory that have been opened, in order.
/// The cards ask for the next one when their end comes into view.
struct DirectoryFiles {
    two_sided: Vec<File>,
    opened: RefCell<
        Vec<(
            Arc<Mutex<Option<Result<Arc<DiffContent>, String>>>>,
            Arc<Mutex<[Vec<Vec<Span>>; 2]>>,
        )>,
    >,
}

impl DirectoryFiles {
    fn open_next(&self, pipeline: &pipeline::Pipeline) {
        let mut opened = self.opened.borrow_mut();
        // One file at a time, in order.
        if opened
            .last()
            .is_some_and(|(diff, _)| diff.lock().unwrap().is_none())
        {
            return;
        }
        if let Some(file) = self.two_sided.get(opened.len()) {
            opened.push(pipeline.open(file.clone()));
        }
    }
}

fn use_directory_files(scope: &mut Scope, ctx: &Context) -> Rc<Option<DirectoryFiles>> {
    let directory = ctx.directory.as_ref().map(Rc::clone);
    let selected = directory.clone();
    use_memo(scope, selected, move || {
        let directory = directory?;
        Some(DirectoryFiles {
            two_sided: directory
                .files
                .iter()
                .filter(|file| file.is_one_sided().is_none())
                .cloned()
                .collect(),
            opened: RefCell::new(Vec::new()),
        })
    })
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
        let state = active_state.current().clone();
        history.current().save(&previous_key, state);
    }
    let state = current_key
        .as_deref()
        .map(|key| history.current().load(key))
        .unwrap_or_default();
    *active_state.current() = state;
    *active_key.current() = current_key;
}

fn layout_listener(
    scope: &mut Scope,
    is_diff: bool,
    set_view_layout: SetState<DiffType>,
    set_wrap: SetState<bool>,
) -> Listeners {
    let keybindings = use_context::<Ui>(scope).keybindings;
    Listeners::new().on_key(move |key| {
        if is_diff && keybindings.matches(Action::ToggleLayout, key) {
            set_view_layout(&|view_layout| view_layout.other());
            Bubble::Stop
        } else if keybindings.matches(Action::ToggleWrap, key) {
            set_wrap(&|wrap| !wrap);
            Bubble::Stop
        } else {
            Bubble::Continue
        }
    })
}

#[component]
pub fn DiffViewer(scope: &mut Scope) -> Node {
    let ctx = use_context::<Ui>(scope);
    let opened_file = use_opened_file(scope, &ctx);
    let opened_file = opened_file.as_ref().as_ref();
    let result = opened_file.map(|(diff, _)| diff.lock().unwrap().clone());
    let content = match &result {
        Some(Some(Ok(content))) => Some(Arc::clone(content)),
        _ => None,
    };
    let directory_files = use_directory_files(scope, &ctx);
    let (view_layout, set_view_layout) = use_state(scope, || match ctx.config.ui.layout {
        config::ViewLayout::SideBySide => DiffType::SideBySide,
        config::ViewLayout::Inline => DiffType::Inline,
    });
    let (wrap, set_wrap) = use_state(scope, || ctx.config.ui.wrap);
    let view_states = use_ref(scope, ViewStateHistory::default);
    let active_view_state = use_ref(scope, ViewState::default);
    let active_file_key = use_ref(scope, || None::<String>);
    sync_active_view_state(
        view_states,
        active_view_state,
        active_file_key,
        content.as_deref(),
    );

    let is_diff =
        ctx.directory.is_some() || matches!(content.as_deref(), Some(DiffContent::Diff(_)));
    let toggle = layout_listener(scope, is_diff, set_view_layout, set_wrap);

    let theme = *ctx.theme;
    let message = |text: String| {
        rsx! {
            Text {
                layout: Layout { grow: 1, fill: Some(theme.normal), ..Default::default() },
                text: text.into(),
                style: theme.normal,
                ..
            }
        }
    };
    let view = if let (Some(directory), Some(files), Some(pipeline)) = (
        ctx.directory.as_ref(),
        directory_files.as_ref().as_ref(),
        ctx.diff_service.as_ref(),
    ) {
        let opened = files.opened.borrow().clone();
        let more_files = opened.len() < files.two_sided.len();
        let directory_files = Rc::clone(&directory_files);
        let pipeline = Rc::clone(pipeline);
        rsx! {
            DiffCards {
                key: Rc::as_ptr(directory) as usize,
                files: Rc::new(opened),
                more_files: more_files,
                open_next: Rc::new(move || {
                    if let Some(directory_files) = directory_files.as_ref() {
                        directory_files.open_next(&pipeline);
                    }
                }) as Rc<dyn Fn()>,
                view_layout: view_layout,
                wrap: wrap,
            }
        }
    } else if let Some(result) = result.filter(|_| content.is_none()) {
        match result {
            Some(Err(error)) => message(error),
            _ => message("Loading file…".to_owned()),
        }
    } else {
        rsx! {
            DiffViewerContainer {
                content: content,
                colours: opened_file.map_or_else(Arc::default, |(_, colours)| Arc::clone(colours)),
                view_layout: view_layout,
                view_state: active_view_state,
                wrap: wrap,
                compact: false,
                auto_focus: true,
            }
        }
    };

    rsx! {
        Column {
            listeners: toggle,
            layout: Layout { grow: 1, ..Default::default() },
            ..,
            { view }
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
