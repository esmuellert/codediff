//! The watcher thread: notify + debouncer + filter → one Refresh per window.

use std::path::Path;
use std::sync::mpsc::{self, Receiver};
use std::time::Duration;

use ignore::gitignore::GitignoreBuilder;
use notify::RecommendedWatcher;
use notify_debouncer_full::{DebounceEventResult, Debouncer, RecommendedCache, new_debouncer};

use crate::Refresh;
use crate::filter::{self, Context};
use crate::scope;

/// A running watcher. Dropping it stops the background thread.
pub struct Watcher {
    _debouncer: Debouncer<RecommendedWatcher, RecommendedCache>,
}

/// Starts watching the repo. Returns the handle and a receiver for refresh
/// events.
pub fn start(repo_root: &Path) -> anyhow::Result<(Watcher, Receiver<Refresh>)> {
    let repo_root = repo_root.canonicalize()?;
    let git_dir = repo_root.join(".git");

    let (tx, rx) = mpsc::channel::<Refresh>();

    // Build the gitignore matcher.
    let ignorer = build_ignorer(&repo_root);

    let ctx = Context {
        repo_root: repo_root.clone(),
        git_dir: git_dir.clone(),
        ignorer,
    };

    // The debouncer's callback runs on notify's internal thread.
    let debouncer_tx = tx;
    let mut debouncer = new_debouncer(
        Duration::from_millis(50),
        None,
        move |result: DebounceEventResult| {
            let events = match result {
                Ok(events) => events,
                Err(errors) => {
                    for e in &errors {
                        tracing::warn!(?e, "watcher error");
                    }
                    return;
                }
            };

            let notify_events: Vec<notify::Event> = events.into_iter().map(|de| de.event).collect();
            let refresh = filter::get_refresh(&notify_events, &ctx);

            if !refresh.is_empty() {
                tracing::info!(%refresh, "refresh triggered");
                let _ = debouncer_tx.send(refresh);
            }
        },
    )?;

    // Register all watch roots.
    let watch_roots = scope::get_scope(&repo_root, &git_dir);
    for root in &watch_roots {
        if let Err(e) = debouncer.watch(&root.path, root.mode) {
            tracing::warn!(path = ?root.path, ?e, "failed to watch directory");
        }
    }
    tracing::info!(count = watch_roots.len(), "watcher started");

    Ok((
        Watcher {
            _debouncer: debouncer,
        },
        rx,
    ))
}

fn build_ignorer(repo_root: &Path) -> ignore::gitignore::Gitignore {
    let mut builder = GitignoreBuilder::new(repo_root);
    let gitignore_path = repo_root.join(".gitignore");
    if gitignore_path.exists() {
        let _ = builder.add(&gitignore_path);
    }
    let exclude_path = repo_root.join(".git/info/exclude");
    if exclude_path.exists() {
        let _ = builder.add(&exclude_path);
    }
    builder.build().unwrap_or_else(|_| {
        let (gi, _) = ignore::gitignore::Gitignore::new(repo_root.join(".gitignore"));
        gi
    })
}
