//! Lazily loads commit history in 500-commit batches when `RevisionPicker` is mounted.

use std::cell::Cell;
use std::path::Path;
use std::rc::Rc;

use loom::{Scope, use_effect, use_memo, use_state};

use crate::Commit;
use crate::services::{Resource, VcsService, WatcherService};

pub(super) const BATCH_SIZE: usize = 500;
pub(super) const PREFETCH_MARGIN: usize = 100;

struct BatchRequest {
    generation: u32,
    skip: usize,
    resource: Resource<Vec<Commit>>,
}

impl PartialEq for BatchRequest {
    fn eq(&self, other: &Self) -> bool {
        self.generation == other.generation && self.skip == other.skip
    }
}

#[allow(clippy::type_complexity)]
pub(super) fn use_commits(
    scope: &mut Scope,
    repo: &Rc<Path>,
    vcs: Option<&Rc<VcsService>>,
    watcher: Option<&Rc<WatcherService>>,
) -> (Rc<Vec<Commit>>, Option<Rc<dyn Fn()>>) {
    let (commits, set_commits) = use_state(scope, || Rc::new(Vec::<Commit>::new()));
    let (has_more, set_has_more) = use_state(scope, || false);
    let (refreshes, set_refreshes) = use_state(scope, || 0u32);
    let (extra_batch, set_extra_batch) = use_state(scope, || None::<Rc<BatchRequest>>);

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

    let repo_for_first = Rc::clone(repo);
    let vcs_for_first = vcs.map(Rc::clone);
    let first_batch = use_memo(scope, (Rc::clone(repo), refreshes), move || {
        vcs_for_first.map(|vcs| {
            (
                vcs.list_commits(&repo_for_first, 0, BATCH_SIZE + 1),
                Cell::new(false),
            )
        })
    });

    // Synchronous Frame-0 read when `VcsService` already holds a prewarmed first page.
    let mut effective_commits = Rc::clone(&commits);
    let mut effective_has_more = has_more;
    if let Some(result) = first_batch.as_ref().as_ref().and_then(|(res, consumed)| {
        let taken = res.is_ready().then(|| res.take()).flatten();
        if taken.is_some() {
            consumed.set(true);
        }
        taken
    }) {
        match result {
            Ok(mut listed) => {
                let more = listed.len() > BATCH_SIZE;
                if more {
                    listed.truncate(BATCH_SIZE);
                }
                if effective_has_more != more {
                    effective_has_more = more;
                    set_has_more.set(more);
                }
                if *effective_commits != listed {
                    let next = Rc::new(listed);
                    effective_commits = Rc::clone(&next);
                    set_commits.set(next);
                }
            }
            Err(_) => {
                if effective_has_more {
                    effective_has_more = false;
                    set_has_more.set(false);
                }
            }
        }
    }

    let first_ready = first_batch
        .as_ref()
        .as_ref()
        .is_some_and(|(res, _)| res.is_ready());

    // Clear any stale in-flight pagination batch when repo or refreshes advances.
    use_effect(scope, (Rc::clone(repo), refreshes), move || {
        set_extra_batch.update(|pending| {
            if pending
                .as_ref()
                .is_some_and(|req| req.generation != refreshes)
            {
                *pending = None;
            }
        });
    });

    let first_for_effect = Rc::clone(&first_batch);
    use_effect(
        scope,
        (Rc::clone(repo), refreshes, first_ready),
        move || {
            let Some((res, consumed)) = first_for_effect.as_ref().as_ref() else {
                return;
            };
            let Some(result) = res.take() else {
                return;
            };
            consumed.set(true);
            let Ok(mut listed) = result else {
                set_has_more.set(false);
                return;
            };
            let more = listed.len() > BATCH_SIZE;
            if more {
                listed.truncate(BATCH_SIZE);
            }
            set_has_more.set(more);
            set_commits.update(|current| {
                if **current != listed {
                    *current = Rc::new(listed);
                }
            });
        },
    );

    let active_extra = extra_batch
        .as_ref()
        .filter(|req| req.generation == refreshes)
        .map(Rc::clone);
    let extra_ready = active_extra
        .as_ref()
        .is_some_and(|req| req.resource.is_ready());
    let extra_key = active_extra.as_ref().map(|req| (req.generation, req.skip));

    let extra_for_effect = active_extra.clone();
    use_effect(scope, (extra_key, extra_ready), move || {
        let Some(req) = extra_for_effect else {
            return;
        };
        let Some(result) = req.resource.take() else {
            return;
        };
        set_extra_batch.set(None);
        let Ok(mut listed) = result else {
            set_has_more.set(false);
            return;
        };
        let more = listed.len() > BATCH_SIZE;
        if more {
            listed.truncate(BATCH_SIZE);
        }
        set_has_more.set(more);
        if !listed.is_empty() {
            set_commits.update(|current| {
                if current.len() == req.skip {
                    let mut combined = (**current).clone();
                    combined.extend(listed);
                    *current = Rc::new(combined);
                }
            });
        }
    });

    let first_pending = first_batch
        .as_ref()
        .as_ref()
        .is_some_and(|(_, consumed)| !consumed.get());
    let loading = first_pending || active_extra.is_some();

    let load_more = if effective_has_more && !loading {
        vcs.map(|vcs| {
            let repo_for_more = Rc::clone(repo);
            let vcs_for_more = Rc::clone(vcs);
            let current_len = effective_commits.len();
            Rc::new(move || {
                set_extra_batch.update(|pending| {
                    if pending
                        .as_ref()
                        .is_some_and(|req| req.generation == refreshes)
                    {
                        return;
                    }
                    let resource =
                        vcs_for_more.list_commits(&repo_for_more, current_len, BATCH_SIZE + 1);
                    *pending = Some(Rc::new(BatchRequest {
                        generation: refreshes,
                        skip: current_len,
                        resource,
                    }));
                });
            }) as Rc<dyn Fn()>
        })
    } else {
        None
    };

    (effective_commits, load_more)
}
