//! The git backend.
//!
//! Everything below this module speaks git — `cat_file`, `rev_parse`, `status`
//! are named for the commands they run, so the file tree answers "which command
//! is this?" before you open anything. [`to_change`] is the one place git's
//! model is translated into the neutral one, and the only place that has to
//! change if git's vocabulary and ours ever disagree.

pub mod cat_file;
pub mod rev_parse;
pub mod run;
pub mod status;
pub mod worktree;

use std::path::Path;

use crate::Repo;
use crate::error::Result;
use file_types::{ChangeType, ChangedFile};
use file_types::{File, FileContent, RepoPath};

pub use status::{Code, Entry, Oid, Untracked, Xy};

/// Reads a git repository by running `git`.
///
/// Running the real binary rather than reimplementing it means the user's own
/// config, `.gitignore` rules, worktrees, sparse checkout and clean filters
/// already apply. Those rules decide which files appear at all, so matching
/// them matters more than the milliseconds a library would save. See D21.
#[derive(Debug)]
pub struct Git {
    repo: Repo,
    untracked: Untracked,
    /// What the before side means. `HEAD` for the default worktree comparison.
    before: String,
    /// Opened on first use, so a status-only run never pays for the child.
    blobs: Option<cat_file::Batch>,
}

impl Git {
    /// Opens the repository containing `path`.
    ///
    /// Runs `git rev-parse --show-toplevel` and `--absolute-git-dir`.
    pub fn open(path: &Path) -> Result<Self> {
        Ok(Self {
            repo: rev_parse::discover(path)?,
            untracked: Untracked::default(),
            before: "HEAD".to_owned(),
            blobs: None,
        })
    }

    pub fn with_untracked(mut self, untracked: Untracked) -> Self {
        self.untracked = untracked;
        self
    }

    /// Compares against a revision other than `HEAD`.
    pub fn with_before(mut self, rev: impl Into<String>) -> Self {
        self.before = rev.into();
        self
    }

    /// The raw records, in git's own terms.
    ///
    /// Runs `git --no-optional-locks status --porcelain=v2 -z`. Available for
    /// anything that genuinely needs git's staging state, which the neutral
    /// [`ChangedFile`] deliberately does not carry.
    pub fn entries(&self) -> Result<Vec<Entry>> {
        let out = run::run(
            &self.repo.root,
            &[
                "status",
                "--porcelain=v2",
                "-z",
                self.untracked.flag(),
                // Renames are the whole point of the `2` record; without this a
                // moved file appears as an unrelated add and delete.
                "--find-renames",
            ],
        )?;
        status::parse(&out)
    }

    /// A file's content at a revision, straight from the object store.
    ///
    /// Runs `git cat-file --batch`. The neutral trait offers only the two sides
    /// of a *changed* file, which is all a reviewer needs; this is here for
    /// comparisons the trait does not describe, such as one revision against
    /// another. `None` when the object does not exist.
    pub fn cat_file(&mut self, rev: &str, path: &RepoPath) -> Result<Option<Vec<u8>>> {
        self.blobs()?.read(rev, path)
    }

    /// Resolves a revision to an object id. Runs `git rev-parse --verify`.
    pub fn resolve(&self, rev: &str) -> Result<Oid> {
        rev_parse::resolve(&self.repo, rev)
    }

    fn blobs(&mut self) -> Result<&mut cat_file::Batch> {
        if self.blobs.is_none() {
            self.blobs = Some(cat_file::Batch::open(&self.repo)?);
        }
        Ok(self.blobs.as_mut().expect("just opened"))
    }
}

/// What a reviewer needs from git.
///
/// There is no trait here. The contract a backend must meet is the *types*:
/// everything below returns `file-types` values, and `cargo xtask lint-arch`
/// forbids `file-types` from naming `vcs`, so no git concept can reach them.
/// A second backend hits the same target by returning the same types, and the
/// pipeline that calls these methods is what checks it did. See D30.
impl Git {
    /// Where the repository is.
    pub fn repo(&self) -> &Repo {
        &self.repo
    }

    /// Every file that differs between the two sides.
    pub fn files(&mut self) -> Result<Vec<ChangedFile>> {
        let root = self.repo.root.clone();
        Ok(self
            .entries()?
            .into_iter()
            .map(|entry| to_file_diff(entry, &root))
            .collect())
    }

    /// The file's content before the change.
    ///
    /// Takes the whole [`ChangedFile`] rather than a path so that a move reads
    /// its old path without the caller having to know that rule. Returns
    /// [`FileContent`] rather than bytes because a repository holds pictures as
    /// readily as source, and every caller would otherwise have to work that
    /// out for itself.
    pub fn before(&mut self, file: &ChangedFile) -> Result<FileContent> {
        // An untracked file has no before side at all, and asking git for one
        // would spend a round trip to be told so.
        if file.change() == ChangeType::Untracked || file.change() == ChangeType::Added {
            return Ok(FileContent::Absent);
        }
        let rev = self.before.clone();
        let path = file.before_path().clone();
        Ok(FileContent::of(self.blobs()?.read(&rev, &path)?))
    }

    /// The file's content after the change.
    pub fn after(&mut self, file: &ChangedFile) -> Result<FileContent> {
        if file.change() == ChangeType::Deleted {
            return Ok(FileContent::Absent);
        }
        // The after side of the default comparison is the working tree, which
        // is on disk rather than in the object store.
        Ok(FileContent::of(worktree::read(file.path())?))
    }
}

/// Git's model, in the reviewer's terms.
///
/// The one place the two vocabularies meet. Git reports two codes because it
/// compares three things — `HEAD`, the index and the working tree — while a
/// reviewer looking at "what changed since the last commit" wants one answer.
/// The index code wins where they differ, since it is the one that describes
/// the file's relationship to `HEAD`.
pub fn to_file_diff(entry: Entry, root: &std::path::Path) -> ChangedFile {
    let change = match (entry.xy.index, entry.xy.worktree) {
        // Unresolved merges first: nothing else about the codes matters.
        (Code::Unmerged, _) | (_, Code::Unmerged) => ChangeType::Conflicted,
        (_, Code::Untracked) => ChangeType::Untracked,
        (_, Code::Ignored) => ChangeType::Untracked,
        (Code::Renamed | Code::Copied, _) => ChangeType::Moved,
        (Code::Added, _) => ChangeType::Added,
        // Deleted in the index but present on disk is a file staged for
        // deletion and then rewritten — the content differs from HEAD, so it
        // reads as a modification.
        (Code::Deleted, Code::Unmodified) => ChangeType::Deleted,
        (_, Code::Deleted) => ChangeType::Deleted,
        _ => ChangeType::Modified,
    };

    // The one place a path gains its absolute form, because this is the first
    // place that has both git's spelling and the root. Which versions exist is
    // recorded here, in the paths themselves — `File`'s pair *is* that fact,
    // and `ChangedFile` stores nothing that could contradict it.
    let path = RepoPath::new(entry.path, root);
    let file = match (change, entry.original) {
        (ChangeType::Added | ChangeType::Untracked, _) => File::added(path),
        (ChangeType::Deleted, _) => File::deleted(path),
        (_, Some(previous)) => File::renamed(RepoPath::new(previous, root), path),
        (_, None) => File::unchanged_path(path),
    };

    // Only the two the paths cannot express are carried; the rest is read back
    // off `file`, so `Added` and `Moved` have exactly one source.
    if change.needs_a_backend() {
        ChangedFile::reported(file, change)
    } else {
        ChangedFile::new(file, entry.score)
    }
}
