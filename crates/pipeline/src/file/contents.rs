//! Stage one: read both versions of a file from git.

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
        .read(&file, DiffVersion::Original)
        .context("reading the before side")?;
    let modified = repository
        .read(&file, DiffVersion::Modified)
        .context("reading the after side")?;
    Ok(Contents {
        file,
        original,
        modified,
    })
}

impl Contents {
    /// Which file this is, for everything downstream.
    pub fn file(&self) -> &File {
        &self.file
    }

    pub fn is_binary(&self) -> bool {
        self.original.is_binary() || self.modified.is_binary()
    }

    /// The lines of one version. Empty if the version does not exist.
    pub fn version(&self, version: DiffVersion) -> Vec<&str> {
        let content = match version {
            DiffVersion::Original => &self.original,
            DiffVersion::Modified => &self.modified,
        };
        content.text().map(lines).unwrap_or_default()
    }
}
