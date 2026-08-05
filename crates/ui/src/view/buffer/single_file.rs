//! One version of a file, shown alone.
//!
//! A peer of [`SideBySide`] and [`Inline`], not a content type. It is what
//! both diff layouts fall back to when a file exists on only one side: there
//! is nothing to lay out against, so neither two columns nor an interleaving
//! has anything to say.
//!
//! No second version means no alignment, no filler and no divider — one column
//! of numbered lines, in the ordinary colours. Nothing here changed *relative
//! to* anything, so nothing is highlighted; marking every line of a new file
//! green says nothing the word "added" does not. VSCode reached the same place
//! and stopped opening a diff editor for added, untracked and deleted files.
//! See D23.
//!
//! It holds no [`Diff`](crate::diff::Diff) for the same reason, which is why
//! that field cannot move up to the parent: an `Option<Diff>` there would be
//! the empty-model trap D23 records.
//!
//! [`SideBySide`]: super::SideBySide
//! [`Inline`]: super::Inline

use align::DiffVersion;
use file_types::File;

use crate::paint::{Colours, Job, Painted, Painter, Spans, Version, path_of};

/// One version of a file, and its lines.
#[derive(Debug)]
pub struct SingleFile {
    file: File,
    lines: Vec<String>,
    /// The colours the painter has sent back. One set, not two: there is only
    /// one version here, which is the whole difference from a diff.
    colours: Colours,
    version: Version,
}

impl SingleFile {
    /// Copies the lines in, as [`Alignment::new`] does, so a caller holding
    /// borrowed lines need not convert them first.
    ///
    /// [`Alignment::new`]: align::Alignment::new
    pub fn new(file: File, lines: &[&str]) -> Self {
        Self {
            file,
            lines: lines.iter().map(|line| (*line).to_owned()).collect(),
            colours: Colours::default(),
            version: Version(0),
        }
    }

    /// The colouring, for a frame.
    pub fn spans(&self) -> Spans<'_> {
        Spans::One(&self.colours)
    }

    /// Asks the painter for this file.
    pub fn start_painting(&mut self, painter: &Painter, version: Version) {
        self.version = version;
        // Whichever side exists: a lone file is one or the other, and asking
        // for the side it is not would find no path and colour nothing.
        let side = self.file.only().unwrap_or(DiffVersion::Modified);
        let Some(path) = path_of(&self.file, side) else {
            return;
        };
        painter.paint(Job {
            version,
            path,
            lines: self.lines.clone(),
        });
    }

    /// Whether the file is still waiting for colours.
    pub fn painting(&self) -> bool {
        self.colours.lines() < self.lines.len() as u32
    }

    /// Installs a piece the painter finished, if it is still wanted.
    pub fn install(&mut self, painted: Painted) -> bool {
        if painted.version != self.version {
            return false;
        }
        self.colours.install(painted);
        true
    }

    /// Which file this is — structured, so a status line can style and shorten
    /// its parts independently.
    pub fn file(&self) -> &File {
        &self.file
    }

    pub fn lines(&self) -> u32 {
        self.lines.len() as u32
    }

    pub fn line(&self, view_line: u32) -> Option<&str> {
        self.lines.get(view_line as usize).map(String::as_str)
    }
}
