//! The file, and its two versions.
//!
//! The second stage, and the last one that performs IO. Everything after this
//! is pure computation over the two texts it produces.

use anyhow::{Context, Result};
use file_types::{ChangedFile, DiffVersion, File, FileContent};

use vscode_diff::lines;

use crate::pipeline::resolver::Resolved;

/// One file, with both versions read.
pub struct Contents {
    pub diff: ChangedFile,
    pub original: FileContent,
    pub modified: FileContent,
}

/// Answers stage two: get the two texts.
pub fn read(resolved: Resolved) -> Result<Contents> {
    let Resolved { mut git, file } = resolved;
    let original = git
        .read(&file, DiffVersion::Original)
        .context("reading the before side")?;
    let modified = git
        .read(&file, DiffVersion::Modified)
        .context("reading the after side")?;
    Ok(Contents {
        diff: file,
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
        &self.diff.file
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
