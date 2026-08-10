//! Determines which directories to hand to notify.
//!
//! On Linux: one NonRecursive watch per non-ignored directory (so target/ is
//! never watched). On macOS/Windows: one Recursive watch on the root (native
//! backends don't cost per-directory handles) and filtering happens later.

use std::path::{Path, PathBuf};

use notify::RecursiveMode;

/// A directory to watch and how deep.
pub struct WatchRoot {
    pub path: PathBuf,
    pub mode: RecursiveMode,
}

/// Returns the directories to watch for a given repo.
///
/// `git_dir` is this worktree's own — `index` and `HEAD`. `common_dir` is the
/// one it shares with every other worktree — `refs/` and `packed-refs`. In a
/// plain repository the two are the same directory.
pub fn get_scope(repo_root: &Path, git_dir: &Path, common_dir: &Path) -> Vec<WatchRoot> {
    let mut roots = worktree_roots(repo_root);
    roots.push(WatchRoot {
        path: git_dir.to_owned(),
        mode: RecursiveMode::NonRecursive,
    });
    // packed-refs lives beside refs/, in the shared dir.
    if common_dir != git_dir {
        roots.push(WatchRoot {
            path: common_dir.to_owned(),
            mode: RecursiveMode::NonRecursive,
        });
    }
    // refs/ can have subdirectories (refs/heads/, refs/remotes/, refs/tags/).
    let refs_dir = common_dir.join("refs");
    if refs_dir.is_dir() {
        roots.push(WatchRoot {
            path: refs_dir,
            mode: RecursiveMode::Recursive,
        });
    }
    roots
}

#[cfg(target_os = "linux")]
fn worktree_roots(repo_root: &Path) -> Vec<WatchRoot> {
    use ignore::WalkBuilder;

    WalkBuilder::new(repo_root)
        .hidden(false)
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true)
        .parents(true)
        .filter_entry(|e| e.file_name() != ".git")
        .build()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_some_and(|t| t.is_dir()))
        .map(|e| WatchRoot {
            path: e.into_path(),
            mode: RecursiveMode::NonRecursive,
        })
        .collect()
}

#[cfg(not(target_os = "linux"))]
fn worktree_roots(repo_root: &Path) -> Vec<WatchRoot> {
    vec![WatchRoot {
        path: repo_root.to_owned(),
        mode: RecursiveMode::Recursive,
    }]
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
        let roots = get_scope(root, &git_dir, &git_dir);
        assert!(roots.iter().any(|r| r.path == git_dir));
    }

    #[test]
    fn refs_subdir_is_recursive() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        std::fs::create_dir_all(root.join(".git/refs/heads")).unwrap();
        let git_dir = root.join(".git");
        let roots = get_scope(root, &git_dir, &git_dir);
        let refs_root = roots.iter().find(|r| r.path == root.join(".git/refs"));
        assert!(refs_root.is_some());
        assert_eq!(refs_root.unwrap().mode, RecursiveMode::Recursive);
    }

    #[test]
    fn plain_repo_watches_its_git_dir_once() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        std::fs::create_dir_all(root.join(".git/refs/heads")).unwrap();
        let git_dir = root.join(".git");
        let roots = get_scope(root, &git_dir, &git_dir);
        let times = roots.iter().filter(|r| r.path == git_dir).count();
        assert_eq!(times, 1, "the one git dir should be watched once");
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
        let roots = get_scope(&wt, &private, &common);

        let private_root = roots.iter().find(|r| r.path == private);
        assert!(
            private_root.is_some(),
            "the private git dir must be watched"
        );
        assert_eq!(private_root.unwrap().mode, RecursiveMode::NonRecursive);

        let common_root = roots.iter().find(|r| r.path == common);
        assert!(common_root.is_some(), "the common dir must be watched");
        assert_eq!(common_root.unwrap().mode, RecursiveMode::NonRecursive);
    }

    #[test]
    fn worktree_watches_the_shared_refs() {
        let tmp = tempfile::tempdir().unwrap();
        let (wt, private, common) = linked_worktree(&tmp);
        let roots = get_scope(&wt, &private, &common);
        let refs_root = roots.iter().find(|r| r.path == common.join("refs"));
        assert!(refs_root.is_some(), "refs/ is shared, not private");
        assert_eq!(refs_root.unwrap().mode, RecursiveMode::Recursive);
    }
}
