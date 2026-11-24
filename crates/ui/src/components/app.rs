//! Application root component.

use std::path::Path;
use std::rc::Rc;

use file_types::File;
use loom::{
    Bubble, Column, ColumnProps, Layout, Listeners, Node, Row, RowProps, Scope, component,
    crokey::key, rsx, use_exit, use_memo, use_state,
};

use super::border::{Border, BorderProps};
use super::context::{Context, DirectorySelection, Ui, UiProps};
use super::diff_viewer::DiffViewer;
use super::explorer::Explorer;
use super::revision_header::RevisionHeader;
use super::revision_picker::RevisionPicker;
use super::revisions::{RevisionSide, swap_revision_range};
use crate::RevisionRange;
use crate::hooks::use_commits::use_commits;
use crate::keybindings::{Action, KeyMap};
use crate::services::{DiffService, SyntaxService, VcsService, WatcherService};
use crate::theme::{Theme, cached_branch_drawing};

fn configured_theme(name: &str) -> Theme {
    if name == "auto" {
        return Theme::from_environment();
    }
    Theme::named(name).unwrap_or_else(Theme::from_environment)
}

#[allow(clippy::too_many_arguments)]
#[component]
pub fn App(
    scope: &mut Scope,
    cwd: Rc<Path>,
    revision_range: RevisionRange,
    config: Rc<config::Config>,
    keybindings: Rc<KeyMap>,
    vcs: Rc<VcsService>,
    diff: Rc<DiffService>,
    syntax: Rc<SyntaxService>,
    watcher: Rc<WatcherService>,
) -> Node {
    let initial_range = revision_range.clone();
    let (revision_range, set_revision_range) = use_state(scope, || initial_range);
    let (file, set_file) = use_state(scope, || None::<Rc<File>>);
    let (directory, set_directory) = use_state(scope, || None::<Rc<DirectorySelection>>);
    let (picking, set_picking) = use_state(scope, || None::<RevisionSide>);

    let theme_name = config.ui.theme.clone();
    let theme = use_memo(scope, theme_name.clone(), move || {
        configured_theme(&theme_name)
    });
    let branch_drawing = *use_memo(scope, (), cached_branch_drawing);
    let commits = use_commits(scope, cwd, vcs, watcher);

    let exit = use_exit(scope);
    let explorer_width = config.ui.explorer_width;
    let keybindings_for_keys = Rc::clone(keybindings);
    let range_for_keys = revision_range.clone();

    let keys = Listeners::new().on_key(move |k| {
        if picking.is_some() && (k == key!(esc) || keybindings_for_keys.matches(Action::Quit, k)) {
            set_picking.set(None);
            return Bubble::Stop;
        }
        if keybindings_for_keys.matches(Action::Quit, k) {
            exit();
            return Bubble::Stop;
        }
        if keybindings_for_keys.matches(Action::PickLeft, k) {
            set_picking.update(|side| {
                *side = (*side != Some(RevisionSide::Left)).then_some(RevisionSide::Left);
            });
            return Bubble::Stop;
        }
        if keybindings_for_keys.matches(Action::PickRight, k) {
            set_picking.update(|side| {
                *side = (*side != Some(RevisionSide::Right)).then_some(RevisionSide::Right);
            });
            return Bubble::Stop;
        }
        if keybindings_for_keys.matches(Action::SwapRevisions, k) {
            if let Some(swapped) = swap_revision_range(&range_for_keys) {
                set_revision_range.set(swapped);
            }
            return Bubble::Stop;
        }
        Bubble::Continue
    });

    let right_pane = match picking {
        Some(side) => rsx! { RevisionPicker { key: side as u32 } },
        None => rsx! { DiffViewer {} },
    };

    rsx! {
        Ui {
            value: Context {
                config: Rc::clone(config),
                keybindings: Rc::clone(keybindings),
                theme,
                branch_drawing,
                repo: Rc::clone(cwd),
                revision_range,
                set_revision_range: Some(set_revision_range),
                picking,
                set_picking: Some(set_picking),
                commits,
                file: file.as_ref().map(Rc::clone),
                set_file: Some(set_file),
                directory: directory.as_ref().map(Rc::clone),
                set_directory: Some(set_directory),
                vcs: Some(Rc::clone(vcs)),
                diff: Some(Rc::clone(diff)),
                syntax: Some(Rc::clone(syntax)),
                watcher: Some(Rc::clone(watcher)),
            },
            Column {
                listeners: keys,
                layout: Layout { grow: 1, ..Default::default() },
                ..,
                RevisionHeader {}
                Row {
                    layout: Layout { grow: 1, ..Default::default() },
                    ..,
                    Border {
                        layout: Layout { basis: loom::Basis::Length(explorer_width), shrink: 1, ..Default::default() },
                        Explorer {}
                    }
                    Border {
                        layout: Layout { grow: 1, ..Default::default() },
                        { right_pane }
                    }
                }
            }
        }
    }
}
