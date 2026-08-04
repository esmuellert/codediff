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
use syntax::Highlighted;

use crate::highlight::{self, Spans};

/// One version of a file, and its lines.
#[derive(Debug)]
pub struct SingleFile {
    file: File,
    lines: Vec<String>,
    /// How far the file has been coloured. One, not two: there is only one
    /// version here, which is the whole difference from a diff.
    syntax: Highlighted,
}

impl SingleFile {
    /// Copies the lines in, as [`Alignment::new`] does, so a caller holding
    /// borrowed lines need not convert them first.
    ///
    /// [`Alignment::new`]: align::Alignment::new
    pub fn new(file: File, lines: &[&str]) -> Self {
        let lines: Vec<String> = lines.iter().map(|line| (*line).to_owned()).collect();
        // Whichever side exists: a lone file is one or the other, and asking
        // for the side it is not would find no path and colour nothing.
        let version = file.only().unwrap_or(DiffVersion::Modified);
        let syntax = highlight::begin(&file, version, &lines);
        Self {
            file,
            lines,
            syntax,
        }
    }

    /// The colouring, for a frame.
    pub fn spans(&self) -> Spans<'_> {
        Spans::One(&self.syntax)
    }

    /// Colours up to the given line, numbered from 1.
    pub fn reach(&mut self, number: u32) {
        highlight::reach(&mut self.syntax, number, &self.lines);
    }

    /// Whether the file has been coloured as far as the given line.
    pub fn caught_up(&self, number: u32) -> bool {
        highlight::caught_up(&self.syntax, number)
    }

    /// Colours a little more, and says whether there was anything to do.
    pub fn read_more(&mut self) -> bool {
        highlight::read_more(&mut self.syntax, &self.lines)
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
