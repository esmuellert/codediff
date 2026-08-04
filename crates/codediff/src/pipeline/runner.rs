//! Running the stages, and handing over the result.
//!
//! The last stage, and the only place that constructs the value `ui` is
//! given to draw. It is a file of its own rather than part of `mod.rs` so that
//! the folder listing reads as the pipeline: five stages, five files, none of
//! them hidden in the signpost.

use anyhow::Result;
use file_types::DiffVersion;
use ui::{Buffer, Diff, DiffLayout};

use crate::pipeline::contents::{self, Contents};
use crate::pipeline::diff;
use crate::pipeline::resolver::{self, Request};

/// Drives the five stages for one request.
///
/// Stages one and two run in [`Runner::new`] and their results are owned here.
/// Stages three to five run in [`Runner::run`], which returns what they
/// produce rather than lending it — possible only because an alignment owns
/// the two files it describes. See D27.
pub struct Runner {
    pub contents: Contents,
}

impl Runner {
    /// Runs stages one and two: find the file, read both sides.
    pub fn new(request: &Request<'_>) -> Result<Self> {
        let resolved = resolver::resolve(request)?;
        Ok(Self {
            contents: contents::read(resolved)?,
        })
    }

    /// The one version this file exists as, or `None` when it exists as both.
    pub fn only(&self) -> Option<DiffVersion> {
        self.contents.file().only()
    }

    /// A picture has no lines, so there is nothing to align.
    pub fn is_binary(&self) -> bool {
        self.contents.is_binary()
    }

    /// Runs stages three to five and returns something to draw.
    ///
    /// Which *kind* of buffer is decided here, from how many sides were read,
    /// because this is the last place that knows. A file existing on one side
    /// has nothing to compare against, so it becomes a plain text buffer
    /// rather than a diff with an empty column. See D23.
    pub fn run(&self) -> Result<Buffer> {
        let file = self.contents.file().clone();
        match file.only() {
            // Nothing to compare against, so neither two columns nor an
            // interleaving has anything to say. Both diff modes land here.
            Some(version) => Ok(Buffer::single_file(file, &self.contents.version(version))),
            None => {
                let original = self.contents.version(DiffVersion::Original);
                let modified = self.contents.version(DiffVersion::Modified);
                let changed = diff::compute(&original, &modified)?;
                let alignment = diff::align(changed, &original, &modified)?;
                // Side by side is where a reader starts; `t` reads the same
                // diff inline without rebuilding any of it.
                Ok(Buffer::diff(
                    Diff::new(file, alignment),
                    DiffLayout::SideBySide,
                ))
            }
        }
    }
}
