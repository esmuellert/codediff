//! Loads the selected file or directory and chooses its view.

mod card;
mod cards;
mod code_text;
mod compact;
mod container;
mod filler;
mod fold;
mod gutter;
mod header;
mod inline;
mod navigation;
mod side_by_side;
mod single_file;
mod terminal_lines;
mod welcome;

pub use card::{DiffCard, DiffCardProps};
pub use cards::{DiffCards, DiffCardsProps};
pub use code_text::{CodeText, CodeTextProps};
pub use container::{DiffViewerContainer, DiffViewerContainerProps};
pub use filler::{Filler, FillerProps};
pub use gutter::{Gutter, GutterProps};
pub use header::{DiffHeader, DiffHeaderProps, DiffViewMenu, DiffViewMenuProps};
pub use navigation::use_diff_viewer_navigation;
pub use terminal_lines::{TerminalLine, terminal_line_pairs};

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

use diff::DiffContent;
use file_types::{DiffVersion, File, Rev, ViewerMode};
use loom::{
    Bubble, Column, ColumnProps, Layout, Listeners, Node, Ref, Scope, SetState, Stack, StackProps,
    Text, TextProps, component, rsx, use_context, use_effect, use_memo, use_ref, use_state,
};

use super::context::{Context, Ui, directory_key};
use crate::keybindings::Action;
use crate::services::{DiffService, Resource};

/// The semantic screen position shared by a file's layouts.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ScrollPosition {
    pub(crate) first_terminal_line: Option<TerminalLine>,
    pub first_cell: u32,
}

/// Loads the selected file and keeps it up to date.
///
/// Returns the file's diff resource. `DiffService` fills it in once the file
/// has been read and diffed.
fn use_selected_file(scope: &mut Scope, ctx: &Context) -> Rc<Option<Resource<Arc<DiffContent>>>> {
    // A selected directory is shown as cards instead.
    let selected = if ctx.directory.is_some() {
        None
    } else {
        ctx.file.clone()
    };

    // The selected file as it was last listed. A refresh reads this one.
    let selected_file = use_ref(scope, || None::<Rc<File>>);
    selected_file.current().clone_from(&selected);

    // A file is loaded once. A new listing of the same file only brings new
    // line counts, so files are compared by path and revisions.
    let path_and_revs = selected
        .as_deref()
        .map(|file| (file.path().clone(), file.revs()));
    let resource = use_memo(scope, path_and_revs.clone(), {
        let diff = ctx.diff.clone();
        move || match (diff, selected) {
            (Some(diff), Some(file)) => Some(diff.load((*file).clone())),
            _ => None,
        }
    });

    // When the file changes on disk, load it again into the same resource.
    // It keeps showing the file until the new result is ready.
    use_effect(scope, path_and_revs, {
        let diff = ctx.diff.clone();
        let watcher = ctx.watcher.clone();
        let resource = Rc::clone(&resource);
        move || {
            let Some(resource) = resource.as_ref().clone() else {
                return;
            };
            let (Some(diff), Some(watcher)) = (diff, watcher) else {
                return;
            };
            watcher.subscribe(move |refresh| {
                let Some(file) = selected_file.current().clone() else {
                    return;
                };
                if refresh_affects_file(refresh, &file) {
                    diff.refresh((*file).clone(), &resource);
                }
            });
        }
    });

    resource
}

/// The files of the selected directory that have been loaded, in order.
/// The cards ask for the next one when their end comes into view.
struct DirectoryFiles {
    files: Vec<File>,
    opened: RefCell<Vec<Resource<Arc<DiffContent>>>>,
}

impl DirectoryFiles {
    fn open_next(&self, diff: &DiffService) {
        let mut opened = self.opened.borrow_mut();
        // One file at a time, in order.
        if opened.last().is_some_and(|resource| !resource.is_ready()) {
            return;
        }
        if let Some(file) = self.files.get(opened.len()) {
            opened.push(diff.load(file.clone()));
        }
    }
}

fn use_directory_files(scope: &mut Scope, ctx: &Context) -> Rc<Option<DirectoryFiles>> {
    let directory = ctx.directory.clone();
    use_memo(scope, directory.clone(), move || {
        let directory = directory?;
        Some(DirectoryFiles {
            files: directory.files.as_ref().clone(),
            opened: RefCell::new(Vec::new()),
        })
    })
}

fn sync_active_scroll_position(
    positions: Ref<HashMap<String, ScrollPosition>>,
    active_position: Ref<ScrollPosition>,
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
        let position = active_position.current().clone();
        positions.current().insert(previous_key, position);
    }
    let position = current_key
        .as_deref()
        .and_then(|key| positions.current().get(key).cloned())
        .unwrap_or_default();
    *active_position.current() = position;
    *active_key.current() = current_key;
}

fn viewer_mode_listener(
    scope: &mut Scope,
    is_diff: bool,
    menu_open: bool,
    set_menu_open: SetState<bool>,
    set_viewer_mode: SetState<ViewerMode>,
    set_wrap: SetState<bool>,
) -> Listeners {
    let keybindings = use_context::<Ui>(scope).keybindings;
    Listeners::new().on_key(move |key| {
        if menu_open && key == loom::crokey::key!(esc) {
            set_menu_open.set(false);
            Bubble::Stop
        } else if is_diff && keybindings.matches(Action::ToggleViewerMode, key) {
            set_viewer_mode(&|viewer_mode| viewer_mode.other());
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
    let selected_resource = use_selected_file(scope, &ctx);
    // No file, a file not read yet, its diff, or why it could not be read.
    let loaded = selected_resource.as_ref().as_ref().map(Resource::get);
    let content = match &loaded {
        Some(Some(Ok(content))) => Some(Arc::clone(content)),
        _ => None,
    };
    let content_key = content
        .as_ref()
        .map(|content| Arc::as_ptr(content) as usize);
    let syntax = ctx.syntax.clone();
    let content_for_colours = content.clone();
    let colours = use_memo(scope, content_key, move || {
        match (syntax, content_for_colours) {
            (Some(syntax), Some(content)) => syntax.highlight(content),
            _ => Arc::new(Mutex::new([Vec::new(), Vec::new()])),
        }
    });
    let colours = Arc::clone(&colours);
    let directory_files = use_directory_files(scope, &ctx);
    let (viewer_mode, set_viewer_mode) = use_state(scope, || match ctx.config.ui.viewer_mode {
        config::ViewerMode::SideBySide => ViewerMode::SideBySide,
        config::ViewerMode::Inline => ViewerMode::Inline,
    });
    let (wrap, set_wrap) = use_state(scope, || ctx.config.ui.wrap);
    let (menu_open, set_menu_open) = use_state(scope, || false);
    let has_selection = ctx.directory.is_some() || ctx.file.is_some();
    if !has_selection && menu_open {
        set_menu_open.set(false);
    }
    let scroll_positions = use_ref(scope, HashMap::<String, ScrollPosition>::new);
    let active_scroll_position = use_ref(scope, ScrollPosition::default);
    let active_file_key = use_ref(scope, || None::<String>);
    sync_active_scroll_position(
        scroll_positions,
        active_scroll_position,
        active_file_key,
        content.as_deref(),
    );

    let is_diff =
        ctx.directory.is_some() || matches!(content.as_deref(), Some(DiffContent::Diff(_)));
    let menu_visible = has_selection && menu_open;
    let toggle = viewer_mode_listener(
        scope,
        is_diff,
        menu_visible,
        set_menu_open,
        set_viewer_mode,
        set_wrap,
    );

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
    let view = if let (Some(directory), Some(files), Some(diff)) = (
        ctx.directory.as_ref(),
        directory_files.as_ref().as_ref(),
        ctx.diff.as_ref(),
    ) {
        let opened = files.opened.borrow().clone();
        let more_files = opened.len() < files.files.len();
        let open_next = {
            let directory_files = Rc::clone(&directory_files);
            let diff = Rc::clone(diff);
            Rc::new(move || {
                if let Some(files) = directory_files.as_ref() {
                    files.open_next(&diff);
                }
            }) as Rc<dyn Fn()>
        };
        rsx! {
            DiffCards {
                // The same directory listed again keeps its cards.
                key: directory_key(&directory.path, &directory.revs),
                files: Rc::new(opened),
                more_files,
                open_next,
                viewer_mode,
                wrap,
            }
        }
    } else if content.is_none()
        && let Some(loaded) = loaded
    {
        match loaded {
            Some(Err(error)) => message(error),
            _ => message("Loading file…".to_owned()),
        }
    } else {
        rsx! {
            DiffViewerContainer {
                content,
                colours,
                viewer_mode,
                scroll_position: active_scroll_position,
                wrap,
                compact: false,
                auto_focus: true,
            }
        }
    };

    let menu = if menu_visible {
        rsx! {
            DiffViewMenu {
                viewer_mode,
                set_viewer_mode,
                wrap,
                set_wrap,
                set_menu_open,
            }
        }
    } else {
        Node::Empty
    };

    rsx! {
        Stack {
            listeners: toggle,
            layout: Layout { grow: 1, ..Default::default() },
            ..,
            Column {
                key: 0u32,
                layout: Layout { grow: 1, ..Default::default() },
                ..,
                DiffHeader {
                    menu_open: menu_visible,
                    set_menu_open,
                }
                { view }
            }
            { menu }
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

    use file_types::{CommitHash, RepoPath, Revs};

    use super::*;

    fn file(revs: Revs) -> File {
        File::unchanged_path(RepoPath::new("selected.rs", Path::new("/repo")), revs)
    }

    #[test]
    fn only_changes_to_a_present_mutable_side_refresh() {
        let worktree = file(Revs::worktree_against(CommitHash::new("abc")));
        let staged = file(Revs::new(Rev::Commit(CommitHash::new("abc")), Rev::Index));
        let deleted = File::deleted(
            RepoPath::new("selected.rs", Path::new("/repo")),
            Revs::worktree_against(CommitHash::new("abc")),
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
