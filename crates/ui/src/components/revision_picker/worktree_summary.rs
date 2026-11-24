//! Lazily loads the worktree changed-file summary when `RevisionPicker` needs it.

use std::collections::HashSet;
use std::path::Path;
use std::rc::Rc;

use loom::{Scope, use_effect, use_memo, use_state};

use crate::RevisionRange;
use crate::services::{Resource, VcsService, WatcherService};

fn format_worktree_summary(count: usize) -> String {
    match count {
        0 => "clean".to_owned(),
        1 => "changed · 1 file".to_owned(),
        n => format!("changed · {n} files"),
    }
}

pub(super) fn use_worktree_summary(
    scope: &mut Scope,
    enabled: bool,
    repo: &Rc<Path>,
    vcs: Option<&Rc<VcsService>>,
    watcher: Option<&Rc<WatcherService>>,
) -> Option<Rc<str>> {
    let (summary, set_summary) = use_state(scope, || None::<Rc<str>>);
    let (refreshes, set_refreshes) = use_state(scope, || 0u32);

    let watcher_for_effect = watcher.map(Rc::clone);
    use_effect(scope, (enabled, Rc::clone(repo)), move || {
        if !enabled {
            return;
        }
        let Some(watcher) = watcher_for_effect else {
            return;
        };
        watcher.subscribe(move |refresh: watcher::Refresh| {
            if !refresh.is_empty() {
                set_refreshes.update(|n| *n += 1);
            }
        });
    });

    let repo_for_files = Rc::clone(repo);
    let vcs_for_files = vcs.map(Rc::clone);
    let loaded_files = use_memo(scope, (enabled, Rc::clone(repo), refreshes), move || {
        if !enabled {
            return None;
        }
        vcs_for_files.map(|vcs| vcs.list_files(&repo_for_files, RevisionRange::Worktree))
    });
    let files_ready = loaded_files
        .as_ref()
        .as_ref()
        .is_some_and(Resource::is_ready);

    let files_for_effect = Rc::clone(&loaded_files);
    use_effect(
        scope,
        (enabled, Rc::clone(repo), refreshes, files_ready),
        move || {
            let Some(Ok(listed)) = files_for_effect.as_ref().as_ref().and_then(Resource::take)
            else {
                return;
            };
            let unique: HashSet<&str> = listed.iter().map(|file| file.path().as_str()).collect();
            let next = format_worktree_summary(unique.len());
            set_summary.update(|current| {
                if current.as_deref() != Some(next.as_str()) {
                    *current = Some(Rc::from(next));
                }
            });
        },
    );

    summary
}
