//! Running the stages, and handing over the result.
//!
//! The last stage, and what it produces. It is a file of its own rather than
//! part of `mod.rs` so that the folder listing reads as the pipeline: four
//! stages, four files, none of them hidden in the signpost.

use std::sync::Arc;

use align::Alignment;
use anyhow::Result;
use file_types::{DiffVersion, File};

use crate::file::contents::{self, Contents};
use crate::file::diff;

/// One file, read: what the four stages produce.
///
/// The producer defines it, which is the direction that keeps the graph
/// acyclic — `ui` depends on this crate, so a type of `ui`'s here would be a
/// cycle. Its two cases are [`DiffType`]'s, so the answer says which of the
/// three ways of showing a file this one is rather than leaving it to be
/// worked out again from an `Option` somewhere else. See D60.
///
/// Each case is a struct, because the interface holds it rather than copying
/// out of it: a side-by-side buffer and an inline one are two readings of one
/// [`Diff`], and switching between them moves it rather than rebuilding it.
/// Both structs used to sit in `ui` as well, with the same two fields each and
/// nothing added, converted from this on arrival. See D61.
///
/// Neither carries a colour, a width or a position. Those are the interface's,
/// and it keeps them beside this rather than in it.
///
/// [`DiffType`]: file_types::DiffType
pub enum DiffContent {
    SingleFile(SingleFile),
    Diff(Diff),
}

/// The one version there is.
///
/// An added, untracked or deleted file has nothing on the other side, so there
/// is no pairing to make and no empty column to draw. See D23.
#[derive(Debug)]
pub struct SingleFile {
    pub file: File,
    /// Shared rather than owned outright, as [`Alignment`] shares its two
    /// sides, so the thread that colours can be handed the text without
    /// copying it.
    pub lines: Arc<Vec<String>>,
}

/// One file's two versions, paired line by line.
///
/// Which of the two paired layouts it is read in is the reader's choice and
/// can change without re-reading, so it is not decided here.
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
}

impl SingleFile {
    /// Which side the file exists on.
    ///
    /// Derived rather than stored: a copy kept beside the file is how one
    /// comes to disagree with the other. A file existing on both sides would
    /// have been paired instead of landing here.
    pub fn side(&self) -> DiffVersion {
        self.file.only().unwrap_or(DiffVersion::Modified)
    }
}

/// Drives the four stages for one request.
///
/// Stage one runs in [`Runner::new`] and its result is owned here. Stages two
/// to four run in [`Runner::run`], which returns what they produce rather than
/// lending it — possible only because an alignment owns the two files it
/// describes. See D27.
pub struct Runner {
    pub contents: Contents,
}

impl Runner {
    /// Runs stage one: open the repository, read both sides.
    pub fn new(file: &file_types::ChangedFile) -> Result<Self> {
        Ok(Self {
            contents: contents::read(file)?,
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

    /// Runs stages two to four and returns what was read.
    ///
    /// Two cases, decided by how many versions exist. Which of the three ways
    /// of showing a file that makes it is carried in the answer, so nothing
    /// downstream has to work it out again. See D23 and D60.
    pub fn run(&self) -> Result<DiffContent> {
        let file = self.contents.file().clone();
        match file.only() {
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
                let changed = diff::compute(&original, &modified)?;
                let alignment = diff::align(changed, &original, &modified)?;
                Ok(DiffContent::Diff(Diff { file, alignment }))
            }
        }
    }
}
