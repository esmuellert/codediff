//! Resolves the worktree-specific and common Git directories.

use std::fs;
use std::path::{Path, PathBuf};

/// The git dir belonging to this worktree alone, holding its `index` and `HEAD`.
///
/// A linked worktree has a `.git` *file* reading `gitdir: <path>`, and that is
/// the directory it names. Everywhere else `.git` is the directory itself.
pub(super) fn worktree_git_dir(repo_root: &Path) -> PathBuf {
    let dot_git = repo_root.join(".git");
    if !dot_git.is_file() {
        return dot_git;
    }
    let Ok(text) = fs::read_to_string(&dot_git) else {
        tracing::warn!(path = ?dot_git, "cannot read the .git file");
        return dot_git;
    };
    let Some(git_dir_text) = text.lines().find_map(|line| line.strip_prefix("gitdir: ")) else {
        tracing::warn!(path = ?dot_git, "the .git file names no gitdir");
        return dot_git;
    };
    resolve_against(repo_root, git_dir_text.trim())
}

/// The git dir shared with every other worktree, holding `refs/` and `packed-refs`.
///
/// A linked worktree's Git dir has a `commondir` file naming the common dir,
/// usually relatively. A plain repository has no such file.
pub(super) fn common_git_dir(worktree_git_dir: &Path) -> PathBuf {
    let Ok(text) = fs::read_to_string(worktree_git_dir.join("commondir")) else {
        return worktree_git_dir.to_owned();
    };
    let common_dir_text = text.trim();
    if common_dir_text.is_empty() {
        return worktree_git_dir.to_owned();
    }
    resolve_against(worktree_git_dir, common_dir_text)
}

/// Resolves `path_text` relative to `base` when needed.
fn resolve_against(base: &Path, path_text: &str) -> PathBuf {
    let path = Path::new(path_text);
    let absolute_path = if path.is_absolute() {
        path.to_owned()
    } else {
        base.join(path)
    };
    // `../..` from a worktree's git dir only names the common dir once resolved.
    absolute_path.canonicalize().unwrap_or(absolute_path)
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
    fn linked_worktree(git_dir_line: impl Fn(&Path) -> String) -> (tempfile::TempDir, PathBuf) {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().canonicalize().unwrap();
        let main = root.join("main");
        let worktree_git_dir = main.join(".git/worktrees/wt");
        fs::create_dir_all(main.join(".git/refs/heads")).unwrap();
        fs::create_dir_all(&worktree_git_dir).unwrap();
        fs::write(worktree_git_dir.join("commondir"), "../..\n").unwrap();

        let wt = root.join("wt");
        fs::create_dir_all(&wt).unwrap();
        fs::write(wt.join(".git"), git_dir_line(&worktree_git_dir)).unwrap();
        (tmp, wt)
    }

    #[test]
    fn plain_repo_git_dir_is_dot_git() {
        let tmp = plain_repo();
        assert_eq!(worktree_git_dir(tmp.path()), tmp.path().join(".git"));
    }

    #[test]
    fn plain_repo_common_dir_is_the_git_dir() {
        let tmp = plain_repo();
        let worktree_git_dir = worktree_git_dir(tmp.path());
        assert_eq!(common_git_dir(&worktree_git_dir), worktree_git_dir);
    }

    #[test]
    fn worktree_git_dir_comes_from_the_dot_git_file() {
        let (_tmp, wt) =
            linked_worktree(|worktree_git_dir| format!("gitdir: {}\n", worktree_git_dir.display()));
        assert_eq!(
            worktree_git_dir(&wt),
            wt.parent().unwrap().join("main/.git/worktrees/wt")
        );
    }

    #[test]
    fn worktree_git_dir_may_be_named_relatively() {
        let (_tmp, wt) = linked_worktree(|_| "gitdir: ../main/.git/worktrees/wt\n".to_owned());
        assert_eq!(
            worktree_git_dir(&wt),
            wt.parent().unwrap().join("main/.git/worktrees/wt")
        );
    }

    #[test]
    fn worktree_common_dir_comes_from_commondir() {
        let (_tmp, wt) =
            linked_worktree(|worktree_git_dir| format!("gitdir: {}\n", worktree_git_dir.display()));
        let worktree_git_dir = worktree_git_dir(&wt);
        assert_eq!(
            common_git_dir(&worktree_git_dir),
            wt.parent().unwrap().join("main/.git")
        );
    }

    #[test]
    fn unreadable_dot_git_file_falls_back_to_dot_git() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join(".git"), "nothing git would write\n").unwrap();
        assert_eq!(worktree_git_dir(tmp.path()), tmp.path().join(".git"));
    }
}
