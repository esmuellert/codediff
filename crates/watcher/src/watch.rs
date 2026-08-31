//! The watcher thread: raw notify + manual debounce → one Refresh per window.
//!
//! Uses `notify::RecommendedWatcher` directly (pure epoll, zero idle CPU).
//! Results are sent via an Emitter — no forwarding thread needed.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, mpsc};
use std::thread;
use std::time::Duration;

use channel::Emitter;
use ignore::WalkBuilder;
use ignore::gitignore::GitignoreBuilder;
use notify::RecommendedWatcher;

use crate::Refresh;
use crate::filter::{self, Context};
use crate::scope::{self, WatchScope};

/// How long to wait after the first event before flushing the batch.
const DEBOUNCE: Duration = Duration::from_millis(50);

/// A live subscription. Dropping it stops the underlying notify watcher.
pub struct Subscription {
    _watcher: Arc<Mutex<RecommendedWatcher>>,
}

/// Subscribes to repository changes and sends them through `emitter`.
pub fn subscribe(repo_root: &Path, emitter: Emitter<Refresh>) -> anyhow::Result<Subscription> {
    let repo_root = repo_root.canonicalize()?;
    let git_dir = private_git_dir(&repo_root);
    let common_dir = common_git_dir(&git_dir);
    let (tx_raw, rx_raw) = mpsc::channel::<notify::Event>();

    let mut watcher =
        notify::recommended_watcher(move |result: notify::Result<notify::Event>| match result {
            Ok(event) => {
                let _ = tx_raw.send(event);
            }
            Err(e) => tracing::warn!(?e, "watcher error"),
        })?;
    let desired_scope = scope::compute(&repo_root, &git_dir, &common_dir);
    let watch_count = desired_scope.len();
    let mut watch_scope = WatchScope::install(&mut watcher, desired_scope);
    let watcher = Arc::new(Mutex::new(watcher));
    let worker_watcher = Arc::downgrade(&watcher);

    let mut ctx = Context {
        repo_root: repo_root.clone(),
        git_dir: git_dir.clone(),
        common_dir: common_dir.clone(),
        ignorer: build_ignorer(&repo_root, &common_dir),
    };

    thread::Builder::new()
        .name("watcher-debounce".to_owned())
        .spawn(move || {
            while let Ok(first) = rx_raw.recv() {
                let mut batch = vec![first];
                while let Ok(event) = rx_raw.recv_timeout(DEBOUNCE) {
                    batch.push(event);
                }

                let rules_changed = ignore_rules_changed(&batch, &repo_root, &common_dir);
                let directories_changed = watch_scope.directory_tree_changed(&batch);
                if rules_changed {
                    ctx.ignorer = build_ignorer(&repo_root, &common_dir);
                }
                if rules_changed || directories_changed {
                    let next_scope = scope::compute(&repo_root, &git_dir, &common_dir);
                    let Some(watcher) = worker_watcher.upgrade() else {
                        break;
                    };
                    let Ok(mut watcher) = watcher.lock() else {
                        break;
                    };
                    watch_scope.update(&mut *watcher, next_scope);
                }

                let refresh = filter::get_refresh(&batch, &ctx);
                if !refresh.is_empty() {
                    tracing::info!(%refresh, events = batch.len(), "refresh triggered");
                    if !emitter.send(refresh) {
                        break;
                    }
                }
            }
        })
        .expect("the watcher-debounce thread starts");

    tracing::info!(count = watch_count, "watcher started");
    Ok(Subscription { _watcher: watcher })
}

fn ignore_rules_changed(events: &[notify::Event], repo_root: &Path, common_dir: &Path) -> bool {
    let exclude_path = common_dir.join("info/exclude");
    events.iter().flat_map(|event| &event.paths).any(|path| {
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
    fn nested_gitignore_is_an_ignore_rule() {
        let event = notify::Event {
            kind: notify::EventKind::Any,
            paths: vec![PathBuf::from("/repo/nested/.gitignore")],
            attrs: Default::default(),
        };
        assert!(ignore_rules_changed(
            &[event],
            Path::new("/repo"),
            Path::new("/repo/.git")
        ));
    }

    #[test]
    fn an_ordinary_file_is_not_an_ignore_rule() {
        let event = notify::Event {
            kind: notify::EventKind::Any,
            paths: vec![PathBuf::from("/repo/file.txt")],
            attrs: Default::default(),
        };
        assert!(!ignore_rules_changed(
            &[event],
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
