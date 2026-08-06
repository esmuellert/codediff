//! The git backend.
//!
//! Everything below this module speaks git — `cat_file`, `rev_parse`, `status`
//! are named for the commands they run, so the file tree answers "which command
//! is this?" before you open anything. [`to_change`] is the one place git's
//! model is translated into the neutral one, and the only place that has to
//! change if git's vocabulary and ours ever disagree.

pub mod cat_file;
mod changes;
mod name_status;
mod numstat;
pub mod rev_parse;
pub mod run;
pub mod status;
pub mod worktree;

use std::path::Path;

use crate::Repo;
use crate::error::Result;
use file_types::{ChangeType, ChangedFile, DiffVersion, Revs};
use file_types::{File, FileContent, RepoPath};

pub use changes::Changes;
pub use name_status::{Change, to_changed_file};
pub use numstat::Counts;
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
    /// What the before side means, as the caller spelled it. `HEAD` unless
    /// told otherwise.
    before: String,
    /// [`before`](Self::before) resolved, with what the after side is.
    ///
    /// Resolved on first use, so a status-only run never pays for the extra
    /// process — and resolved **once**, so a commit made while a review is
    /// open cannot leave half its files named against one `HEAD` and half
    /// against another.
    revs: Option<Revs>,
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
            revs: None,
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
        self.revs = None;
        self
    }

    /// What the two sides of this comparison are, resolved.
    ///
    /// The before side becomes an id rather than staying a name, because a
    /// name moves and an id is what says which bytes were read.
    pub fn revs(&mut self) -> Result<Revs> {
        if self.revs.is_none() {
            let commit = rev_parse::resolve_or_empty(&self.repo, &self.before)?;
            self.revs = Some(Revs::worktree_against(commit));
        }
        Ok(self.revs.clone().expect("just resolved"))
    }

    /// The raw records, in git's own terms.
    ///
    /// Runs `git --no-optional-locks status --porcelain=v2 -z`. Available for
    /// anything that genuinely needs git's staging state, which the neutral
    /// [`ChangedFile`] deliberately does not carry.
    pub fn entries(&self, pathspec: &[String]) -> Result<Vec<Entry>> {
        let mut args = vec![
            "status",
            "--porcelain=v2",
            "-z",
            self.untracked.flag(),
            // Renames are the whole point of the `2` record; without this a
            // moved file appears as an unrelated add and delete.
            "--find-renames",
        ];
        if !pathspec.is_empty() {
            args.push("--");
        }
        let owned: Vec<&str> = pathspec.iter().map(String::as_str).collect();
        args.extend_from_slice(&owned);
        status::parse(&run::run(&self.repo.root, &args)?)
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
        let revs = self.revs()?;
        Ok(self
            .entries(&[])?
            .into_iter()
            .map(|entry| to_file_diff(entry, &root, revs.clone()))
            .collect())
    }

    /// The content of one side of a file.
    ///
    /// Takes the whole [`ChangedFile`] rather than a path so that a move reads
    /// its old path without the caller having to know that rule. Returns
    /// [`FileContent`] rather than bytes because a repository holds pictures as
    /// readily as source.
    ///
    /// One function for both sides: which side it is says nothing about where
    /// to look, and the file's own revision does.
    pub fn read(&mut self, file: &ChangedFile, version: DiffVersion) -> Result<FileContent> {
        let Some(path) = file.file.on(version).cloned() else {
            return Ok(FileContent::Absent);
        };
        match file.file.rev(version).stored() {
            None => Ok(FileContent::of(worktree::read(&path)?)),
            // Cloned because reading borrows `self` mutably, and the revision
            // lives in the file rather than in the batch.
            Some(rev) => {
                let rev = rev.to_owned();
                // Against the working tree, the stored side is converted the
                // way a checkout would convert it. A repository with
                // `core.autocrlf` stores LF and checks out CRLF, so comparing
                // the stored bytes with the bytes on disk marked **every line**
                // changed — measured, on a file where one line had been
                // edited. The same is true of any clean/smudge filter.
                //
                // Not batched: `cat-file --batch --filters` reports the size
                // of the object *before* filtering and then writes the
                // filtered bytes, so a reader framing by that size falls out
                // of step with the stream. One process per file is the price
                // of being right, and it is paid only when a file is opened.
                if file.file.rev(version.other()) == &file_types::Rev::Worktree {
                    return Ok(FileContent::of(filtered(&self.repo, &rev, &path)?));
                }
                Ok(FileContent::of(self.blobs()?.read(&rev, &path)?))
            }
        }
    }
}

/// A stored version, converted as a checkout would convert it.
///
/// Runs `git cat-file --filters <rev>:<path>`. `None` when the object does not
/// exist, which is ordinary: one side of a diff is routinely absent.
fn filtered(repo: &Repo, rev: &str, path: &RepoPath) -> Result<Option<Vec<u8>>> {
    let spec = format!("{rev}:{path}");
    match run::run(&repo.root, &["cat-file", "--filters", &spec]) {
        Ok(bytes) => Ok(Some(bytes)),
        // Only what git says when the object is not there. Reading *every*
        // failure as "missing" turned a broken clean filter, a corrupt object
        // and a killed process into a file that had simply been added — a
        // whole-file diff, with nothing to say the read had failed.
        Err(crate::error::Error::Git { stderr, .. }) if is_missing(&stderr) => Ok(None),
        Err(other) => Err(other),
    }
}

/// Whether git's complaint means the object does not exist.
///
/// Matched on the message because `cat-file` exits 128 for everything. The
/// wordings are git's own, and a wording we do not know is treated as a real
/// failure — the safe way round, since the cost is an error the reader can
/// read rather than a diff that quietly lies.
fn is_missing(stderr: &str) -> bool {
    stderr.contains("does not exist")
        || stderr.contains("Not a valid object name")
        || stderr.contains("unknown revision")
        || stderr.ends_with("missing")
        || stderr.contains("exists on disk, but not in")
}

/// Git's model, in the reviewer's terms.
///
/// The one place the two vocabularies meet. Git reports two codes because it
/// compares three things — `HEAD`, the index and the working tree — while a
/// reviewer looking at "what changed since the last commit" wants one answer.
/// The index code wins where they differ, since it is the one that describes
/// the file's relationship to `HEAD`.
pub fn to_file_diff(entry: Entry, root: &std::path::Path, revs: Revs) -> ChangedFile {
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
        (ChangeType::Added | ChangeType::Untracked, _) => File::added(path, revs),
        (ChangeType::Deleted, _) => File::deleted(path, revs),
        (_, Some(previous)) => File::renamed(RepoPath::new(previous, root), path, revs),
        (_, None) => File::unchanged_path(path, revs),
    };

    // Only the two the paths cannot express are carried; the rest is read back
    // off `file`, so `Added` and `Moved` have exactly one source.
    if change.needs_a_backend() {
        ChangedFile::reported(file, change)
    } else {
        ChangedFile::new(file, entry.score)
    }
}
