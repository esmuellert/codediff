//! Loads repository commits via `VcsService` and refreshes them when
//! `WatcherService` reports repository changes.

use std::path::Path;
use std::rc::Rc;

use loom::{Scope, use_effect, use_memo, use_state};

use crate::Commit;
use crate::services::{VcsService, WatcherService};

const COMMIT_LIMIT: usize = 500;

pub fn use_commits(
    scope: &mut Scope,
    repo: &Rc<Path>,
    vcs: &Rc<VcsService>,
    watcher: &Rc<WatcherService>,
) -> Rc<Vec<Commit>> {
    let (commits, set_commits) = use_state(scope, || Rc::new(Vec::<Commit>::new()));
    let (refreshes, set_refreshes) = use_state(scope, || 0u32);

    let watcher_for_effect = Rc::clone(watcher);
    use_effect(scope, Rc::clone(repo), move || {
        watcher_for_effect
            .subscribe()
            .subscribe(move |refresh: watcher::Refresh| {
                if refresh.head || refresh.refs {
                    set_refreshes.update(|n| *n += 1);
                }
            });
    });

    let repo_for_commits = Rc::clone(repo);
    let vcs_for_commits = Rc::clone(vcs);
    let commit_slot = use_memo(scope, (Rc::clone(repo), refreshes), move || {
        vcs_for_commits.list_commits(&repo_for_commits, COMMIT_LIMIT)
    });
    let commits_ready = commit_slot.lock().unwrap().is_some();

    let commit_slot_for_effect = Rc::clone(&commit_slot);
    use_effect(
        scope,
        (Rc::clone(repo), refreshes, commits_ready),
        move || {
            let Some(Ok(listed)) = commit_slot_for_effect.lock().unwrap().take() else {
                return;
            };
            set_commits.update(|current| {
                if **current != listed {
                    *current = Rc::new(listed);
                }
            });
        },
    );

    commits
}
