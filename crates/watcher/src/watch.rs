//! The watcher thread: raw notify + bounded debounce → one Refresh per batch.
//!
//! Uses `notify::RecommendedWatcher` directly (pure epoll, zero idle CPU).
//! Results are sent via an Emitter — no forwarding thread needed.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::time::{Duration, Instant};

use channel::Emitter;
use ignore::WalkBuilder;
use ignore::gitignore::GitignoreBuilder;
use notify::RecommendedWatcher;

use crate::Refresh;
use crate::filter::{self, Context};
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
        let reload_ignore_rules = event_requires_ignore_reload(
            event,
            &filter_context.repo_root,
            &filter_context.common_dir,
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
    let git_dir = private_git_dir(&repo_root);
    let common_dir = common_git_dir(&git_dir);
    let (event_sender, event_receiver) = mpsc::sync_channel::<notify::Event>(EVENT_QUEUE_CAPACITY);
    let queue_overflow_flag = Arc::new(AtomicBool::new(false));
    let callback_overflow_flag = Arc::clone(&queue_overflow_flag);

    let mut watcher = notify::recommended_watcher(move |result| match result {
        Ok(event) => try_queue_event(&event_sender, &callback_overflow_flag, event),
        Err(error) => tracing::warn!(?error, "watcher error"),
    })?;
    let desired_scope = scope::compute(&repo_root, &git_dir, &common_dir);
    let watch_count = desired_scope.len();
    let mut watch_scope = WatchScope::install(&mut watcher, desired_scope);
    let watcher = Arc::new(Mutex::new(watcher));
    let worker_watcher = Arc::downgrade(&watcher);

    let mut filter_context = Context {
        repo_root: repo_root.clone(),
        git_dir: git_dir.clone(),
        common_dir: common_dir.clone(),
        ignorer: build_ignorer(&repo_root, &common_dir),
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
                    filter_context.ignorer = build_ignorer(&repo_root, &common_dir);
                }
                if batch.recompute_scope {
                    let next_scope = scope::compute(&repo_root, &git_dir, &common_dir);
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

fn event_requires_ignore_reload(
    event: &notify::Event,
    repo_root: &Path,
    common_dir: &Path,
) -> bool {
    if matches!(event.kind, notify::EventKind::Access(_)) {
        return false;
    }
    let exclude_path = common_dir.join("info/exclude");
    event.paths.iter().any(|path| {
        path.strip_prefix(repo_root).is_ok_and(|relative| {
            relative
                .file_name()
                .is_some_and(|name| name == ".gitignore")
        }) || path == &exclude_path
    })
}

fn build_ignorer(repo_root: &Path, common_dir: &Path) -> ignore::gitignore::Gitignore {
    let mut builder = GitignoreBuilder::new(repo_root);
    let root_rules = repo_root.join(".gitignore");
    if root_rules.exists() {
        let _ = builder.add(&root_rules);
    }

    for entry in WalkBuilder::new(repo_root)
        .hidden(false)
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true)
        .parents(true)
        .filter_entry(|entry| entry.file_name() != ".git")
        .build()
        .filter_map(Result::ok)
    {
        let path = entry.path();
        if path != root_rules && path.file_name().is_some_and(|name| name == ".gitignore") {
            let _ = builder.add(path);
        }
    }

    let exclude_path = common_dir.join("info/exclude");
    if exclude_path.exists() {
        let _ = builder.add(&exclude_path);
    }
    builder.build().unwrap_or_else(|_| {
        let (gi, _) = ignore::gitignore::Gitignore::new(root_rules);
        gi
    })
}

/// The git dir belonging to this worktree alone, holding its `index` and `HEAD`.
///
/// A linked worktree has a `.git` *file* reading `gitdir: <path>`, and that is
/// the directory it names. Everywhere else `.git` is the directory itself.
fn private_git_dir(repo_root: &Path) -> PathBuf {
    let dot_git = repo_root.join(".git");
    if !dot_git.is_file() {
        return dot_git;
    }
    let Ok(text) = fs::read_to_string(&dot_git) else {
        tracing::warn!(path = ?dot_git, "cannot read the .git file");
        return dot_git;
    };
    let Some(named) = text.lines().find_map(|l| l.strip_prefix("gitdir: ")) else {
        tracing::warn!(path = ?dot_git, "the .git file names no gitdir");
        return dot_git;
    };
    resolve_against(repo_root, named.trim())
}

/// The git dir shared with every other worktree, holding `refs/` and `packed-refs`.
///
/// A linked worktree's private dir has a `commondir` file naming it, usually
/// relatively. A plain repository has no such file, and shares nothing.
fn common_git_dir(git_dir: &Path) -> PathBuf {
    let Ok(text) = fs::read_to_string(git_dir.join("commondir")) else {
        return git_dir.to_owned();
    };
    let named = text.trim();
    if named.is_empty() {
        return git_dir.to_owned();
    }
    resolve_against(git_dir, named)
}

/// Reads `named` as a path, taking `base` as its start when it is relative.
fn resolve_against(base: &Path, named: &str) -> PathBuf {
    let path = Path::new(named);
    let joined = if path.is_absolute() {
        path.to_owned()
    } else {
        base.join(path)
    };
    // `../..` from a worktree's git dir only names the common dir once resolved.
    joined.canonicalize().unwrap_or(joined)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A plain repository: `.git/` with a `refs/`, and no worktree files.
    fn plain_repo() -> tempfile::TempDir {
        let tmp = tempfile::tempdir().unwrap();
        fs::create_dir_all(tmp.path().join(".git/refs/heads")).unwrap();
        tmp
    }

    /// A repository with one linked worktree, as `git worktree add` leaves it.
    /// Answers (main root, worktree root).
    fn linked_worktree(gitdir_line: impl Fn(&Path) -> String) -> (tempfile::TempDir, PathBuf) {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().canonicalize().unwrap();
        let main = root.join("main");
        let private = main.join(".git/worktrees/wt");
        fs::create_dir_all(main.join(".git/refs/heads")).unwrap();
        fs::create_dir_all(&private).unwrap();
        fs::write(private.join("commondir"), "../..\n").unwrap();

        let wt = root.join("wt");
        fs::create_dir_all(&wt).unwrap();
        fs::write(wt.join(".git"), gitdir_line(&private)).unwrap();
        (tmp, wt)
    }

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
            git_dir: PathBuf::from("/repo/.git"),
            common_dir: PathBuf::from("/repo/.git"),
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

    #[test]
    fn nested_gitignore_change_requires_reload() {
        let event = notify::Event {
            kind: notify::EventKind::Any,
            paths: vec![PathBuf::from("/repo/nested/.gitignore")],
            attrs: Default::default(),
        };
        assert!(event_requires_ignore_reload(
            &event,
            Path::new("/repo"),
            Path::new("/repo/.git")
        ));
    }

    #[test]
    fn reading_gitignore_does_not_require_reload() {
        let event = notify::Event {
            kind: notify::EventKind::Access(notify::event::AccessKind::Open(
                notify::event::AccessMode::Any,
            )),
            paths: vec![PathBuf::from("/repo/.gitignore")],
            attrs: Default::default(),
        };
        assert!(!event_requires_ignore_reload(
            &event,
            Path::new("/repo"),
            Path::new("/repo/.git")
        ));
    }

    #[test]
    fn ordinary_file_change_does_not_require_ignore_reload() {
        let event = notify::Event {
            kind: notify::EventKind::Any,
            paths: vec![PathBuf::from("/repo/file.txt")],
            attrs: Default::default(),
        };
        assert!(!event_requires_ignore_reload(
            &event,
            Path::new("/repo"),
            Path::new("/repo/.git")
        ));
    }

    #[test]
    fn plain_repo_git_dir_is_dot_git() {
        let tmp = plain_repo();
        assert_eq!(private_git_dir(tmp.path()), tmp.path().join(".git"));
    }

    #[test]
    fn plain_repo_common_dir_is_the_git_dir() {
        let tmp = plain_repo();
        let git_dir = private_git_dir(tmp.path());
        assert_eq!(common_git_dir(&git_dir), git_dir);
    }

    #[test]
    fn worktree_git_dir_comes_from_the_dot_git_file() {
        let (_tmp, wt) = linked_worktree(|private| format!("gitdir: {}\n", private.display()));
        assert_eq!(
            private_git_dir(&wt),
            wt.parent().unwrap().join("main/.git/worktrees/wt")
        );
    }

    #[test]
    fn worktree_git_dir_may_be_named_relatively() {
        let (_tmp, wt) = linked_worktree(|_| "gitdir: ../main/.git/worktrees/wt\n".to_owned());
        assert_eq!(
            private_git_dir(&wt),
            wt.parent().unwrap().join("main/.git/worktrees/wt")
        );
    }

    #[test]
    fn worktree_common_dir_comes_from_commondir() {
        let (_tmp, wt) = linked_worktree(|private| format!("gitdir: {}\n", private.display()));
        let git_dir = private_git_dir(&wt);
        assert_eq!(
            common_git_dir(&git_dir),
            wt.parent().unwrap().join("main/.git")
        );
    }

    #[test]
    fn unreadable_dot_git_file_falls_back_to_dot_git() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join(".git"), "nothing git would write\n").unwrap();
        assert_eq!(private_git_dir(tmp.path()), tmp.path().join(".git"));
    }
}
