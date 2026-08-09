//! The watcher thread: raw notify + manual debounce → one Refresh per window.
//!
//! Uses `notify::RecommendedWatcher` directly (pure epoll, zero idle CPU)
//! instead of `notify-debouncer-full` (which polls at ~80Hz).

use std::path::Path;
use std::sync::mpsc::{self, Receiver};
use std::thread;
use std::time::Duration;

use ignore::gitignore::GitignoreBuilder;
use notify::{RecommendedWatcher, Watcher as NotifyWatcher};

use crate::Refresh;
use crate::filter::{self, Context};
use crate::scope;

/// How long to wait after the first event before flushing the batch.
const DEBOUNCE: Duration = Duration::from_millis(50);

/// A running watcher. Dropping it stops the watcher (notify cleans up on drop).
pub struct Watcher {
    _watcher: RecommendedWatcher,
}

/// Starts watching the repo. Returns the handle and a receiver for refresh
/// events.
pub fn start(repo_root: &Path) -> anyhow::Result<(Watcher, Receiver<Refresh>)> {
    let repo_root = repo_root.canonicalize()?;
    let git_dir = repo_root.join(".git");

    let (tx_refresh, rx_refresh) = mpsc::channel::<Refresh>();

    // Internal channel: raw notify events → debounce thread.
    let (tx_raw, rx_raw) = mpsc::channel::<notify::Event>();

    // Build the gitignore matcher.
    let ignorer = build_ignorer(&repo_root);

    let ctx = Context {
        repo_root: repo_root.clone(),
        git_dir: git_dir.clone(),
        ignorer,
    };

    // Debounce thread: blocks until an event arrives, drains for 50ms, then
    // classifies and sends one Refresh. Zero CPU while idle.
    thread::Builder::new()
        .name("watcher-debounce".to_owned())
        .spawn(move || {
            while let Ok(first) = rx_raw.recv() {
                let mut batch = vec![first];
                // Drain everything that arrives within DEBOUNCE.
                while let Ok(ev) = rx_raw.recv_timeout(DEBOUNCE) {
                    batch.push(ev);
                }
                let refresh = filter::get_refresh(&batch, &ctx);
                if !refresh.is_empty() {
                    tracing::info!(%refresh, events = batch.len(), "refresh triggered");
                    if tx_refresh.send(refresh).is_err() {
                        break;
                    }
                }
            }
        })
        .expect("the watcher-debounce thread starts");

    // Raw watcher: sends events to the debounce thread.
    let mut watcher =
        notify::recommended_watcher(move |result: notify::Result<notify::Event>| match result {
            Ok(event) => {
                let _ = tx_raw.send(event);
            }
            Err(e) => {
                tracing::warn!(?e, "watcher error");
            }
        })?;

    // Register all watch roots.
    let watch_roots = scope::get_scope(&repo_root, &git_dir);
    for root in &watch_roots {
        if let Err(e) = watcher.watch(&root.path, root.mode) {
            tracing::warn!(path = ?root.path, ?e, "failed to watch directory");
        }
    }
    tracing::info!(count = watch_roots.len(), "watcher started");

    Ok((Watcher { _watcher: watcher }, rx_refresh))
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
