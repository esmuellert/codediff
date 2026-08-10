//! The watcher thread: raw notify + manual debounce → one Refresh per window.
//!
//! Uses `notify::RecommendedWatcher` directly (pure epoll, zero idle CPU).
//! Results are sent via an Emitter — no forwarding thread needed.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use channel::Emitter;
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

/// Starts watching the repo. Sends Refresh via the emitter when changes occur.
pub fn start(repo_root: &Path, emitter: Emitter<Refresh>) -> anyhow::Result<Watcher> {
    let repo_root = repo_root.canonicalize()?;
    let git_dir = private_git_dir(&repo_root);
    let common_dir = common_git_dir(&git_dir);

    // Internal channel: raw notify events → debounce thread.
    let (tx_raw, rx_raw) = mpsc::channel::<notify::Event>();

    // Build the gitignore matcher.
    let ignorer = build_ignorer(&repo_root, &common_dir);

    let ctx = Context {
        repo_root: repo_root.clone(),
        git_dir: git_dir.clone(),
        common_dir: common_dir.clone(),
        ignorer,
    };

    // Debounce thread: blocks until an event arrives, drains for 50ms, then
    // classifies and sends one Refresh. Zero CPU while idle.
    thread::Builder::new()
        .name("watcher-debounce".to_owned())
        .spawn(move || {
            while let Ok(first) = rx_raw.recv() {
                let mut batch = vec![first];
                while let Ok(ev) = rx_raw.recv_timeout(DEBOUNCE) {
                    batch.push(ev);
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
    let watch_roots = scope::get_scope(&repo_root, &git_dir, &common_dir);
    for root in &watch_roots {
        if let Err(e) = watcher.watch(&root.path, root.mode) {
            tracing::warn!(path = ?root.path, ?e, "failed to watch directory");
        }
    }
    tracing::info!(count = watch_roots.len(), "watcher started");

    Ok(Watcher { _watcher: watcher })
}

fn build_ignorer(repo_root: &Path, common_dir: &Path) -> ignore::gitignore::Gitignore {
    let mut builder = GitignoreBuilder::new(repo_root);
    let gitignore_path = repo_root.join(".gitignore");
    if gitignore_path.exists() {
        let _ = builder.add(&gitignore_path);
    }
    let exclude_path = common_dir.join("info/exclude");
    if exclude_path.exists() {
        let _ = builder.add(&exclude_path);
    }
    builder.build().unwrap_or_else(|_| {
        let (gi, _) = ignore::gitignore::Gitignore::new(repo_root.join(".gitignore"));
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
