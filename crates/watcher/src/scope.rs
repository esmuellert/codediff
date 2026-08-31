//! Computes the complete set of paths that notify should watch.
//!
//! On Linux: one NonRecursive watch per non-ignored directory (so target/ is
//! never watched). On macOS/Windows: one Recursive watch on the root (native
//! backends don't cost per-directory handles) and filtering happens later.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[cfg(target_os = "linux")]
use notify::EventKind;
#[cfg(target_os = "linux")]
use notify::event::{CreateKind, ModifyKind, RemoveKind};
use notify::{Event, RecursiveMode, Watcher};

/// The paths and recursion modes currently registered with notify.
#[derive(Default)]
pub(super) struct WatchScope {
    paths: HashMap<PathBuf, RecursiveMode>,
}

impl WatchScope {
    pub fn install(watcher: &mut impl Watcher, desired: Self) -> Self {
        let mut installed = Self::default();
        installed.update(watcher, desired);
        installed
    }

    pub fn update(&mut self, watcher: &mut impl Watcher, desired: Self) {
        for (path, mode) in &desired.paths {
            if !self.paths.contains_key(path) {
                match watcher.watch(path, *mode) {
                    Ok(()) => {
                        self.paths.insert(path.clone(), *mode);
                    }
                    Err(e) => tracing::warn!(?path, ?e, "failed to watch directory"),
                }
            }
        }

        let stale_paths: Vec<_> = self
            .paths
            .keys()
            .filter(|path| !desired.paths.contains_key(*path))
            .cloned()
            .collect();
        for path in stale_paths {
            let _ = watcher.unwatch(&path);
            self.paths.remove(&path);
        }
    }

    #[cfg(target_os = "linux")]
    pub fn directory_tree_changed(&self, event: &Event) -> bool {
        match event.kind {
            EventKind::Create(CreateKind::Folder) | EventKind::Remove(RemoveKind::Folder) => true,
            EventKind::Create(_)
            | EventKind::Modify(ModifyKind::Name(_))
            | EventKind::Remove(_) => event
                .paths
                .iter()
                .any(|path| path.is_dir() || self.paths.contains_key(path)),
            _ => false,
        }
    }

    #[cfg(not(target_os = "linux"))]
    pub fn directory_tree_changed(&self, _event: &Event) -> bool {
        false
    }

    pub fn len(&self) -> usize {
        self.paths.len()
    }
}

/// Computes the current watch scope for a repository.
///
/// `git_dir` is this worktree's own — `index` and `HEAD`. `common_dir` is the
/// one it shares with every other worktree — `refs/` and `packed-refs`. In a
/// plain repository the two are the same directory.
pub(super) fn compute(repo_root: &Path, git_dir: &Path, common_dir: &Path) -> WatchScope {
    let mut paths = worktree_paths(repo_root);
    paths.insert(git_dir.to_owned(), RecursiveMode::NonRecursive);

    // packed-refs lives beside refs/, in the shared dir.
    if common_dir != git_dir {
        paths.insert(common_dir.to_owned(), RecursiveMode::NonRecursive);
    }
    let info_dir = common_dir.join("info");
    if info_dir.is_dir() {
        paths.insert(info_dir, RecursiveMode::NonRecursive);
    }
    // refs/ can have subdirectories (refs/heads/, refs/remotes/, refs/tags/).
    let refs_dir = common_dir.join("refs");
    if refs_dir.is_dir() {
        paths.insert(refs_dir, RecursiveMode::Recursive);
    }
    WatchScope { paths }
}

#[cfg(target_os = "linux")]
fn worktree_paths(repo_root: &Path) -> HashMap<PathBuf, RecursiveMode> {
    use ignore::WalkBuilder;

    WalkBuilder::new(repo_root)
        .hidden(false)
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true)
        .parents(true)
        .filter_entry(|entry| entry.file_name() != ".git")
        .build()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_type().is_some_and(|kind| kind.is_dir()))
        .map(|entry| (entry.into_path(), RecursiveMode::NonRecursive))
        .collect()
}

#[cfg(not(target_os = "linux"))]
fn worktree_paths(repo_root: &Path) -> HashMap<PathBuf, RecursiveMode> {
    HashMap::from([(repo_root.to_owned(), RecursiveMode::Recursive)])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn git_dir_is_always_included() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        std::fs::create_dir_all(root.join(".git/refs/heads")).unwrap();
        let git_dir = root.join(".git");
        let scope = compute(root, &git_dir, &git_dir);
        assert_eq!(
            scope.paths.get(&git_dir),
            Some(&RecursiveMode::NonRecursive)
        );
    }

    #[test]
    fn info_subdir_is_watched() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        std::fs::create_dir_all(root.join(".git/info")).unwrap();
        let git_dir = root.join(".git");
        let scope = compute(root, &git_dir, &git_dir);
        assert_eq!(
            scope.paths.get(&root.join(".git/info")),
            Some(&RecursiveMode::NonRecursive)
        );
    }

    #[test]
    fn refs_subdir_is_recursive() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        std::fs::create_dir_all(root.join(".git/refs/heads")).unwrap();
        let git_dir = root.join(".git");
        let scope = compute(root, &git_dir, &git_dir);
        assert_eq!(
            scope.paths.get(&root.join(".git/refs")),
            Some(&RecursiveMode::Recursive)
        );
    }

    /// Lays out a main repository with one linked worktree, and answers
    /// (worktree root, private git dir, common dir).
    fn linked_worktree(tmp: &tempfile::TempDir) -> (PathBuf, PathBuf, PathBuf) {
        let root = tmp.path();
        let common = root.join("main/.git");
        let private = common.join("worktrees/wt");
        let wt = root.join("wt");
        std::fs::create_dir_all(common.join("refs/heads")).unwrap();
        std::fs::create_dir_all(&private).unwrap();
        std::fs::create_dir_all(&wt).unwrap();
        (wt, private, common)
    }

    #[test]
    fn worktree_watches_both_git_dirs() {
        let tmp = tempfile::tempdir().unwrap();
        let (wt, private, common) = linked_worktree(&tmp);
        let scope = compute(&wt, &private, &common);

        assert_eq!(
            scope.paths.get(&private),
            Some(&RecursiveMode::NonRecursive),
            "the private git dir must be watched"
        );
        assert_eq!(
            scope.paths.get(&common),
            Some(&RecursiveMode::NonRecursive),
            "the common dir must be watched"
        );
    }

    #[test]
    fn worktree_watches_the_shared_refs() {
        let tmp = tempfile::tempdir().unwrap();
        let (wt, private, common) = linked_worktree(&tmp);
        let scope = compute(&wt, &private, &common);
        assert_eq!(
            scope.paths.get(&common.join("refs")),
            Some(&RecursiveMode::Recursive),
            "refs/ is shared, not private"
        );
    }

    #[test]
    fn an_ordinary_file_edit_does_not_change_the_directory_tree() {
        let scope = WatchScope::default();
        let event = Event {
            kind: notify::EventKind::Modify(notify::event::ModifyKind::Data(
                notify::event::DataChange::Any,
            )),
            paths: vec![PathBuf::from("/repo/file.txt")],
            attrs: Default::default(),
        };
        assert!(!scope.directory_tree_changed(&event));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn renaming_a_file_does_not_change_the_directory_tree() {
        let scope = WatchScope::default();
        let event = Event {
            kind: EventKind::Modify(ModifyKind::Name(notify::event::RenameMode::Both)),
            paths: vec![
                PathBuf::from("/repo/old.txt"),
                PathBuf::from("/repo/new.txt"),
            ],
            attrs: Default::default(),
        };
        assert!(!scope.directory_tree_changed(&event));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn moving_a_watched_directory_changes_the_directory_tree() {
        let old = PathBuf::from("/repo/old-dir");
        let scope = WatchScope {
            paths: HashMap::from([(old.clone(), RecursiveMode::NonRecursive)]),
        };
        let event = Event {
            kind: EventKind::Modify(ModifyKind::Name(notify::event::RenameMode::From)),
            paths: vec![old],
            attrs: Default::default(),
        };
        assert!(scope.directory_tree_changed(&event));
    }
}
