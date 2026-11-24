//! Lazily loads commit history when `RevisionPicker` is mounted.

use std::path::Path;
use std::rc::Rc;

use loom::{Scope, use_effect, use_memo, use_state};

use crate::Commit;
use crate::services::{Resource, VcsService, WatcherService};

const COMMIT_LIMIT: usize = 500;

pub(super) fn use_commits(
    scope: &mut Scope,
    repo: &Rc<Path>,
    vcs: Option<&Rc<VcsService>>,
    watcher: Option<&Rc<WatcherService>>,
) -> Rc<Vec<Commit>> {
    let (commits, set_commits) = use_state(scope, || Rc::new(Vec::<Commit>::new()));
    let (refreshes, set_refreshes) = use_state(scope, || 0u32);

    let watcher_for_effect = watcher.map(Rc::clone);
    let vcs_for_watcher = vcs.map(Rc::clone);
    use_effect(scope, Rc::clone(repo), move || {
        let Some(watcher) = watcher_for_effect else {
            return;
        };
        watcher.subscribe(move |refresh: watcher::Refresh| {
            if refresh.head || refresh.refs {
                if let Some(vcs) = vcs_for_watcher.as_ref() {
                    vcs.invalidate_commits();
                }
                set_refreshes.update(|n| *n += 1);
            }
        });
    });

    let repo_for_commits = Rc::clone(repo);
    let vcs_for_commits = vcs.map(Rc::clone);
    let loaded_commits = use_memo(scope, (Rc::clone(repo), refreshes), move || {
        vcs_for_commits.map(|vcs| vcs.list_commits(&repo_for_commits, COMMIT_LIMIT))
    });

    // Synchronous Frame-0 read when `VcsService` already holds a prewarmed first page.
    let mut effective_commits = Rc::clone(&commits);
    if let Some(Ok(listed)) = loaded_commits
        .as_ref()
        .as_ref()
        .and_then(|res| res.is_ready().then(|| res.take()).flatten())
        && *effective_commits != listed
    {
        let next = Rc::new(listed);
        effective_commits = Rc::clone(&next);
        set_commits.set(next);
    }

    let commits_ready = loaded_commits
        .as_ref()
        .as_ref()
        .is_some_and(Resource::is_ready);
    let commits_for_effect = Rc::clone(&loaded_commits);
    use_effect(
        scope,
        (Rc::clone(repo), refreshes, commits_ready),
        move || {
            let Some(Ok(listed)) = commits_for_effect
                .as_ref()
                .as_ref()
                .and_then(Resource::take)
            else {
                return;
            };
            set_commits.update(|current| {
                if **current != listed {
                    *current = Rc::new(listed);
                }
            });
        },
    );

    effective_commits
}
