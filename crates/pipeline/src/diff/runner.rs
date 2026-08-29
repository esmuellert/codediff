//! The last stage: runs the pipeline and hands over the result.

use std::sync::Arc;

use align::Alignment;
use anyhow::Result;
use file_types::{DiffVersion, File};

use crate::diff::contents::{self, Contents};
use crate::diff::diff;

/// What the pipeline produces for one file.
pub enum DiffContent {
    SingleFile(SingleFile),
    Diff(Diff),
}

/// A file with only one version (added, untracked, or deleted).
#[derive(Debug)]
pub struct SingleFile {
    pub file: File,
    pub lines: Arc<Vec<String>>,
}

/// Two versions of a file, paired line by line.
#[derive(Debug)]
pub struct Diff {
    pub file: File,
    pub alignment: Alignment,
}

impl DiffContent {
    /// Which file this is, whichever it is.
    pub fn file(&self) -> &File {
        match self {
            Self::SingleFile(single) => &single.file,
            Self::Diff(diff) => &diff.file,
        }
    }

    pub fn alignment(&self) -> Option<&align::Alignment> {
        match self {
            Self::Diff(diff) => Some(&diff.alignment),
            Self::SingleFile(_) => None,
        }
    }
}

impl SingleFile {
    /// Which side the file exists on.
    pub fn side(&self) -> DiffVersion {
        self.file.is_one_sided().unwrap_or(DiffVersion::Modified)
    }
}

/// Drives the four stages for one file.
pub struct Runner {
    pub contents: Contents,
}

impl Runner {
    /// Runs stage one: open the repository, read both sides.
    pub fn new(file: &file_types::File) -> Result<Self> {
        Ok(Self {
            contents: contents::read(file)?,
        })
    }

    /// The one version this file exists as, or `None` when it exists as both.
    pub fn is_one_sided(&self) -> Option<DiffVersion> {
        self.contents.file().is_one_sided()
    }

    /// A picture has no lines, so there is nothing to align.
    pub fn is_binary(&self) -> bool {
        self.contents.is_binary()
    }

    /// Runs stages two to four.
    pub fn compute_diff(&self) -> Result<DiffContent> {
        let file = self.contents.file().clone();
        match file.is_one_sided() {
            // Nothing to compare against, so neither two columns nor an
            // interleaving has anything to say.
            Some(version) => Ok(DiffContent::SingleFile(SingleFile {
                file,
                lines: Arc::new(
                    self.contents
                        .version(version)
                        .iter()
                        .map(|line| (*line).to_owned())
                        .collect(),
                ),
            })),
            None => {
                let original = self.contents.version(DiffVersion::Original);
                let modified = self.contents.version(DiffVersion::Modified);
                tracing::info!(path = %file.path(), lines = modified.len(), "computing diff");
                let changed = diff::compute(&original, &modified)?;
                let alignment = diff::align(changed, &original, &modified)?;
                Ok(DiffContent::Diff(Diff { file, alignment }))
            }
        }
    }
}
