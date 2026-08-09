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
pub fn get_scope(repo_root: &Path, git_dir: &Path) -> Vec<WatchRoot> {
    let mut roots = worktree_roots(repo_root);
    roots.push(WatchRoot {
        path: git_dir.to_owned(),
        mode: RecursiveMode::NonRecursive,
    });
    // refs/ can have subdirectories (refs/heads/, refs/remotes/, refs/tags/).
    let refs_dir = git_dir.join("refs");
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
        let roots = get_scope(root, &root.join(".git"));
        assert!(roots.iter().any(|r| r.path == root.join(".git")));
    }

    #[test]
    fn refs_subdir_is_recursive() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path();
        std::fs::create_dir_all(root.join(".git/refs/heads")).unwrap();
        let roots = get_scope(root, &root.join(".git"));
        let refs_root = roots.iter().find(|r| r.path == root.join(".git/refs"));
        assert!(refs_root.is_some());
        assert_eq!(refs_root.unwrap().mode, RecursiveMode::Recursive);
    }
}
