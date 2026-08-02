//! Deciding what to compare, and finding it.
//!
//! The first stage. Everything awkward about *which* file is answered here, so
//! nothing downstream has to ask.
//!
//! Today there is one kind of request — a path in the worktree, against HEAD.
//! Comparing revisions, or two loose files on disk, are additional variants
//! rather than changes to the stages that follow.

use anyhow::{Context, Result, bail};
use vcs::{Diff as _, DiffKind, FileDiff, Git, RelPath};

/// What the reader asked to see.
#[derive(Debug, Clone)]
pub enum Request<'a> {
    /// One file of this repository, worktree against HEAD.
    Worktree { path: &'a str },
}

/// A file found in a repository, before either side has been read.
pub struct Resolved {
    pub git: Git,
    pub file: FileDiff,
}

/// Answers stage one: what am I being asked to diff.
pub fn resolve(request: &Request<'_>) -> Result<Resolved> {
    match request {
        Request::Worktree { path } => {
            let cwd = std::env::current_dir().context("finding the current directory")?;
            let mut git = Git::open(&cwd).context("opening a repository")?;
            let file = find(&mut git, path)?;
            Ok(Resolved { git, file })
        }
    }
}

/// Locates a file among those git reports as changed.
///
/// By path as given, then relative to the repository root, so it works from a
/// subdirectory the way git's own commands do.
fn find(git: &mut Git, path: &str) -> Result<FileDiff> {
    let files = git.files().context("listing changed files")?;
    let wanted = RelPath::new(path);

    if let Some(found) = files
        .iter()
        .find(|f| f.path == wanted || f.previous_path.as_ref() == Some(&wanted))
    {
        return Ok(found.clone());
    }

    // Not changed, but it may still exist — comparing a file with itself is a
    // legitimate thing to ask for, and produces an empty diff rather than an
    // error.
    let absolute = wanted.to_absolute(&git.repo().root);
    if absolute.exists() {
        return Ok(FileDiff {
            path: wanted,
            previous_path: None,
            kind: DiffKind::Modified,
            similarity: None,
        });
    }

    bail!(
        "{path} is neither changed nor present; git reports {} changed file(s)",
        files.len()
    )
}
