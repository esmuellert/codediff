//! The file, and its two versions.
//!
//! The first stage, and the only one that performs IO. Everything after this
//! is pure computation over the two texts it produces.
//!
//! There used to be a stage before this one that searched git for a file by
//! path. The list pipeline is that search, and a better one — it knows which
//! comparison the reader chose, where searching again invented a third. What
//! is left of the old stage is one line, and it is here.

use anyhow::{Context, Result};
use file_types::{DiffVersion, File, FileContent};
use vcs::Repository;

use vscode_diff::lines;

/// One file, with both versions read.
pub struct Contents {
    pub file: File,
    pub original: FileContent,
    pub modified: FileContent,
}

/// Answers stage one: get the two texts.
///
/// The file arrives already found, carrying the revisions of the comparison it
/// was found in, so all that is needed is the repository it lives in.
pub fn read(file: &File) -> Result<Contents> {
    let mut repository = Repository::open(file.path().root()).context("opening a repository")?;
    let file = file.clone();
    let original = repository
        .get_file_content(&file, DiffVersion::Original)
        .context("reading the before side")?;
    let modified = repository
        .get_file_content(&file, DiffVersion::Modified)
        .context("reading the after side")?;
    Ok(Contents {
        file,
        original,
        modified,
    })
}

impl Contents {
    /// Which file this is, for everything downstream.
    ///
    /// Handed on unchanged. There used to be a `label()` here that fused the
    /// path, the previous path and the added/deleted note into one string, and
    /// the status line could then neither style nor shorten them separately.
    /// The facts travel intact instead, and whatever draws them decides how.
    pub fn file(&self) -> &File {
        &self.file
    }

    /// A picture has no lines, so there is nothing to align.
    pub fn is_binary(&self) -> bool {
        self.original.is_binary() || self.modified.is_binary()
    }

    /// The lines of one version. Empty — genuinely — if it does not exist.
    ///
    /// The distinction is *absent*, never *empty*: a tracked file emptied to
    /// zero bytes still has a version to compare against, and gets a real
    /// two-column diff showing every line deleted. Only a file that does not
    /// exist on one side is left uncompared, and [`File::only`] is what says
    /// so. See D23.
    pub fn version(&self, version: DiffVersion) -> Vec<&str> {
        let content = match version {
            DiffVersion::Original => &self.original,
            DiffVersion::Modified => &self.modified,
        };
        content.text().map(lines).unwrap_or_default()
    }
}
