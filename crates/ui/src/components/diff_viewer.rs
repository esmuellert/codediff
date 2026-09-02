//! Loads the selected file and chooses its view.

use std::rc::Rc;

use file_types::{DiffVersion, File, Rev};
use loom::{Node, Scope, component, rsx, use_context, use_effect, use_state};
use pipeline::diff::DiffContent;

use super::context::Ui;
use super::side_by_side::{SideBySide, SideBySideProps};
use super::single_file::{SingleFile, SingleFileProps};
use super::welcome::Welcome;

#[component]
pub fn DiffViewer(scope: &mut Scope) -> Node {
    let ctx = use_context::<Ui>(scope);
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

    match content {
        Some(content) => match content.as_ref() {
            DiffContent::Diff(_) => rsx! { SideBySide { content: Rc::clone(&content) } },
            DiffContent::SingleFile(_) => {
                rsx! { SingleFile { content: Rc::clone(&content) } }
            }
        },
        None => rsx! { Welcome {} },
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
