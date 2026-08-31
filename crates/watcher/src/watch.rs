//! The watcher thread: raw notify + bounded debounce → one Refresh per batch.
//!
//! Uses `notify::RecommendedWatcher` directly (pure epoll, zero idle CPU).
//! Results are sent via an Emitter — no forwarding thread needed.

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::time::{Duration, Instant};

use channel::Emitter;
use notify::RecommendedWatcher;

use crate::Refresh;
use crate::filter::{self, Context};
use crate::git_dirs;
use crate::ignore_rules;
use crate::scope::{self, WatchScope};

/// Flushes an ordinary burst after this long without another event.
const QUIET_PERIOD: Duration = Duration::from_millis(50);

/// Prevents continuous events from postponing a refresh indefinitely.
const MAX_BATCH_DURATION: Duration = Duration::from_millis(250);

/// Bounds memory while absorbing normal kernel event bursts.
const EVENT_QUEUE_CAPACITY: usize = 1_024;

/// Summarizes a burst without retaining every raw event.
#[derive(Default)]
struct Batch {
    refresh: Refresh,
    reload_ignore_rules: bool,
    recompute_scope: bool,
    processed_event_count: usize,
    events_lost: bool,
}

impl Batch {
    fn add_event(
        &mut self,
        event: &notify::Event,
        filter_context: &Context,
        watch_scope: &WatchScope,
    ) {
        self.processed_event_count += 1;
        if event.need_rescan() {
            self.invalidate_all();
            return;
        }

        self.refresh = self
            .refresh
            .union(filter::refresh_for_event(event, filter_context));
        let reload_ignore_rules = ignore_rules::requires_reload(
            event,
            &filter_context.repo_root,
            &filter_context.common_git_dir,
        );
        self.reload_ignore_rules |= reload_ignore_rules;
        // Rule changes can add or remove visible worktree files by themselves.
        self.refresh.worktree |= reload_ignore_rules;
        self.recompute_scope |= reload_ignore_rules || watch_scope.directory_tree_changed(event);
    }

    fn invalidate_all(&mut self) {
        // A dropped event may have affected any refresh bit or watch root.
        self.refresh = Refresh {
            worktree: true,
            index: true,
            head: true,
            refs: true,
        };
        self.reload_ignore_rules = true;
        self.recompute_scope = true;
        self.events_lost = true;
    }
}

fn try_queue_event(
    sender: &mpsc::SyncSender<notify::Event>,
    queue_overflow_flag: &AtomicBool,
    event: notify::Event,
) {
    match sender.try_send(event) {
        Ok(()) => {}
        Err(mpsc::TrySendError::Full(_)) => queue_overflow_flag.store(true, Ordering::Relaxed),
        Err(mpsc::TrySendError::Disconnected(_)) => {}
    }
}

/// A live subscription. Dropping it stops the underlying notify watcher.
pub struct Subscription {
    _watcher: Arc<Mutex<RecommendedWatcher>>,
}

/// Subscribes to repository changes and sends them through `emitter`.
pub fn subscribe(repo_root: &Path, emitter: Emitter<Refresh>) -> anyhow::Result<Subscription> {
    let repo_root = repo_root.canonicalize()?;
    let worktree_git_dir = git_dirs::worktree_git_dir(&repo_root);
    let common_git_dir = git_dirs::common_git_dir(&worktree_git_dir);
    let (event_sender, event_receiver) = mpsc::sync_channel::<notify::Event>(EVENT_QUEUE_CAPACITY);
    let queue_overflow_flag = Arc::new(AtomicBool::new(false));
    let callback_overflow_flag = Arc::clone(&queue_overflow_flag);

    let mut watcher = notify::recommended_watcher(move |result| match result {
        Ok(event) => try_queue_event(&event_sender, &callback_overflow_flag, event),
        Err(error) => tracing::warn!(?error, "watcher error"),
    })?;
    let desired_scope = scope::compute(&repo_root, &worktree_git_dir, &common_git_dir);
    let watch_count = desired_scope.len();
    let mut watch_scope = WatchScope::install(&mut watcher, desired_scope);
    let watcher = Arc::new(Mutex::new(watcher));
    let worker_watcher = Arc::downgrade(&watcher);

    let mut filter_context = Context {
        repo_root: repo_root.clone(),
        worktree_git_dir: worktree_git_dir.clone(),
        common_git_dir: common_git_dir.clone(),
        ignorer: ignore_rules::build_matcher(&repo_root, &common_git_dir),
    };

    thread::Builder::new()
        .name("watcher-events".to_owned())
        .spawn(move || {
            while let Ok(first_event) = event_receiver.recv() {
                let batch_deadline = Instant::now() + MAX_BATCH_DURATION;
                let mut batch = Batch::default();
                batch.add_event(&first_event, &filter_context, &watch_scope);

                let queue_disconnected = loop {
                    let remaining_batch_time =
                        batch_deadline.saturating_duration_since(Instant::now());
                    if remaining_batch_time.is_zero() {
                        break false;
                    }
                    match event_receiver.recv_timeout(QUIET_PERIOD.min(remaining_batch_time)) {
                        Ok(event) => {
                            batch.add_event(&event, &filter_context, &watch_scope);
                        }
                        Err(mpsc::RecvTimeoutError::Timeout) => break false,
                        Err(mpsc::RecvTimeoutError::Disconnected) => break true,
                    }
                };

                if queue_overflow_flag.swap(false, Ordering::Relaxed) {
                    tracing::warn!("filesystem event queue overflowed; refreshing all state");
                    batch.invalidate_all();
                }
                if batch.reload_ignore_rules {
                    filter_context.ignorer =
                        ignore_rules::build_matcher(&repo_root, &common_git_dir);
                }
                if batch.recompute_scope {
                    let next_scope = scope::compute(&repo_root, &worktree_git_dir, &common_git_dir);
                    let Some(watcher) = worker_watcher.upgrade() else {
                        break;
                    };
                    let Ok(mut watcher) = watcher.lock() else {
                        break;
                    };
                    watch_scope.update(&mut *watcher, next_scope);
                }

                if !batch.refresh.is_empty() {
                    tracing::info!(
                        refresh = %batch.refresh,
                        processed_events = batch.processed_event_count,
                        events_lost = batch.events_lost,
                        "refresh triggered"
                    );
                    if !emitter.send(batch.refresh) {
                        break;
                    }
                }
                if queue_disconnected {
                    break;
                }
            }
        })
        .expect("the watcher event thread starts");

    tracing::info!(count = watch_count, "watcher started");
    Ok(Subscription { _watcher: watcher })
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn event_queue_overflow_invalidates_all_derived_state() {
        let (sender, _receiver) = mpsc::sync_channel(1);
        let overflow_flag = AtomicBool::new(false);
        sender
            .try_send(notify::Event {
                kind: notify::EventKind::Any,
                paths: vec![PathBuf::from("/repo/first.txt")],
                attrs: Default::default(),
            })
            .unwrap();

        try_queue_event(
            &sender,
            &overflow_flag,
            notify::Event {
                kind: notify::EventKind::Any,
                paths: vec![PathBuf::from("/repo/dropped.txt")],
                attrs: Default::default(),
            },
        );

        let mut batch = Batch::default();
        if overflow_flag.swap(false, Ordering::Relaxed) {
            batch.invalidate_all();
        }
        assert_eq!(
            batch.refresh,
            Refresh {
                worktree: true,
                index: true,
                head: true,
                refs: true,
            }
        );
        assert!(batch.reload_ignore_rules && batch.recompute_scope && batch.events_lost);
    }

    #[test]
    fn backend_rescan_invalidates_all_derived_state() {
        let event =
            notify::Event::new(notify::EventKind::Other).set_flag(notify::event::Flag::Rescan);
        let mut batch = Batch::default();
        let (ignorer, _) = ignore::gitignore::Gitignore::new("/repo/.gitignore");
        let filter_context = Context {
            repo_root: PathBuf::from("/repo"),
            worktree_git_dir: PathBuf::from("/repo/.git"),
            common_git_dir: PathBuf::from("/repo/.git"),
            ignorer,
        };

        batch.add_event(&event, &filter_context, &WatchScope::default());

        assert_eq!(
            batch.refresh,
            Refresh {
                worktree: true,
                index: true,
                head: true,
                refs: true,
            }
        );
        assert!(batch.reload_ignore_rules && batch.recompute_scope && batch.events_lost);
    }
}
