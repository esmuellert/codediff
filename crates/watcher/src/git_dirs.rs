//! Resolves the worktree-specific and common Git directories.

use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use anyhow::{Context, bail};

/// The git dir belonging to this worktree alone, holding its `index` and `HEAD`.
///
/// A linked worktree has a `.git` *file* reading `gitdir: <path>`, and that is
/// the directory it names. Everywhere else `.git` is the directory itself.
pub(super) fn worktree_git_dir(repo_root: &Path) -> anyhow::Result<PathBuf> {
    let dot_git = repo_root.join(".git");
    if dot_git.is_dir() {
        return validate_worktree_git_dir(&dot_git);
    }
    if !dot_git.is_file() {
        bail!("{} is not a Git worktree", repo_root.display());
    }

    let text = fs::read_to_string(&dot_git)
        .with_context(|| format!("failed to read {}", dot_git.display()))?;
    let Some(git_dir_text) = text.lines().find_map(|line| line.strip_prefix("gitdir: ")) else {
        bail!("{} contains no gitdir pointer", dot_git.display());
    };
    let git_dir_text = git_dir_text.trim();
    if git_dir_text.is_empty() {
        bail!("{} contains an empty gitdir pointer", dot_git.display());
    }
    validate_worktree_git_dir(&resolve_against(repo_root, git_dir_text))
}

/// The git dir shared with every other worktree, holding `refs/` and `packed-refs`.
///
/// A linked worktree's Git dir has a `commondir` file naming the common dir,
/// usually relatively. A plain repository has no such file.
pub(super) fn common_git_dir(worktree_git_dir: &Path) -> anyhow::Result<PathBuf> {
    let commondir = worktree_git_dir.join("commondir");
    let text = match fs::read_to_string(&commondir) {
        Ok(text) => text,
        Err(error) if error.kind() == ErrorKind::NotFound => {
            return Ok(worktree_git_dir.to_owned());
        }
        Err(error) => {
            return Err(error).with_context(|| format!("failed to read {}", commondir.display()));
        }
    };
    let common_dir_text = text.trim();
    if common_dir_text.is_empty() {
        bail!("{} contains no common Git directory", commondir.display());
    }
    let common_dir = resolve_against(worktree_git_dir, common_dir_text);
    canonical_directory(&common_dir, "common Git directory")
}

/// Resolves `path_text` relative to `base` when needed.
fn resolve_against(base: &Path, path_text: &str) -> PathBuf {
    let path = Path::new(path_text);
    if path.is_absolute() {
        path.to_owned()
    } else {
        base.join(path)
    }
}

fn validate_worktree_git_dir(path: &Path) -> anyhow::Result<PathBuf> {
    let git_dir = canonical_directory(path, "worktree Git directory")?;
    if !git_dir.join("HEAD").is_file() {
        bail!("worktree Git directory has no HEAD: {}", git_dir.display());
    }
    Ok(git_dir)
}

fn canonical_directory(path: &Path, description: &str) -> anyhow::Result<PathBuf> {
    let canonical = path
        .canonicalize()
        .with_context(|| format!("failed to resolve {}", path.display()))?;
    if !canonical.is_dir() {
        bail!("{description} is not a directory: {}", canonical.display());
    }
    Ok(canonical)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A plain repository: `.git/` with a `refs/`, and no worktree files.
    fn plain_repo() -> tempfile::TempDir {
        let tmp = tempfile::tempdir().unwrap();
        fs::create_dir_all(tmp.path().join(".git/refs/heads")).unwrap();
        fs::write(tmp.path().join(".git/HEAD"), "ref: refs/heads/main\n").unwrap();
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
        fs::write(worktree_git_dir.join("HEAD"), "ref: refs/heads/main\n").unwrap();
        fs::write(worktree_git_dir.join("commondir"), "../..\n").unwrap();

        let wt = root.join("wt");
        fs::create_dir_all(&wt).unwrap();
        fs::write(wt.join(".git"), git_dir_line(&worktree_git_dir)).unwrap();
        (tmp, wt)
    }

    #[test]
    fn plain_repo_git_dir_is_dot_git() {
        let tmp = plain_repo();
        assert_eq!(
            worktree_git_dir(tmp.path()).unwrap(),
            tmp.path().join(".git").canonicalize().unwrap()
        );
    }

    #[test]
    fn plain_repo_common_dir_is_the_git_dir() {
        let tmp = plain_repo();
        let worktree_git_dir = worktree_git_dir(tmp.path()).unwrap();
        assert_eq!(common_git_dir(&worktree_git_dir).unwrap(), worktree_git_dir);
    }

    #[test]
    fn worktree_git_dir_comes_from_the_dot_git_file() {
        let (_tmp, wt) =
            linked_worktree(|worktree_git_dir| format!("gitdir: {}\n", worktree_git_dir.display()));
        assert_eq!(
            worktree_git_dir(&wt).unwrap(),
            wt.parent().unwrap().join("main/.git/worktrees/wt")
        );
    }

    #[test]
    fn worktree_git_dir_may_be_named_relatively() {
        let (_tmp, wt) = linked_worktree(|_| "gitdir: ../main/.git/worktrees/wt\n".to_owned());
        assert_eq!(
            worktree_git_dir(&wt).unwrap(),
            wt.parent().unwrap().join("main/.git/worktrees/wt")
        );
    }

    #[test]
    fn worktree_common_dir_comes_from_commondir() {
        let (_tmp, wt) =
            linked_worktree(|worktree_git_dir| format!("gitdir: {}\n", worktree_git_dir.display()));
        let worktree_git_dir = worktree_git_dir(&wt).unwrap();
        assert_eq!(
            common_git_dir(&worktree_git_dir).unwrap(),
            wt.parent().unwrap().join("main/.git")
        );
    }

    #[test]
    fn malformed_dot_git_file_is_rejected() {
        let tmp = tempfile::tempdir().unwrap();
        fs::write(tmp.path().join(".git"), "nothing git would write\n").unwrap();
        assert!(worktree_git_dir(tmp.path()).is_err());
    }
}
