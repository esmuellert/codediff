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

use std::sync::Arc;

use crate::syntax::{Spans, Store, Syntax, SyntaxRequest, Version, path_of};

/// One version of a file, and its lines.
#[derive(Debug)]
pub struct SingleFile {
    file: File,
    /// Shared, so the thread that colours can be handed the text rather than
    /// a copy of it.
    lines: Arc<Vec<String>>,
    /// Which side the file is on. One, not two: there is only one version
    /// here, which is the whole difference from a diff.
    side: DiffVersion,
    version: Version,
}

impl SingleFile {
    /// Copies the lines in, as [`Alignment::new`] does, so a caller holding
    /// borrowed lines need not convert them first.
    ///
    /// [`Alignment::new`]: align::Alignment::new
    pub fn new(file: File, lines: &[&str]) -> Self {
        // Whichever side exists: a lone file is one or the other, and the side
        // it is not has no path and so no language.
        let side = file.only().unwrap_or(DiffVersion::Modified);
        Self {
            side,
            file,
            lines: Arc::new(lines.iter().map(|line| (*line).to_owned()).collect()),
            version: Version(0),
        }
    }

    /// What names this file's content, if it is on the side it claims.
    ///
    /// Asked of the file rather than held, because it is derived — a copy
    /// kept beside the file is how one comes to disagree with the other.
    fn key(&self) -> Option<String> {
        self.file.name(self.side)
    }

    /// The colouring, for a frame.
    pub fn spans<'a>(&self, store: &'a Store) -> Spans<'a> {
        match self.key().and_then(|key| store.get(&key)) {
            Some(colours) => Spans::One(colours),
            None => Spans::Off,
        }
    }

    /// Asks for everything up to `want`.
    pub fn request(&mut self, syntax: &mut Syntax, store: &mut Store, version: Version, want: u32) {
        self.version = version;
        let (Some(key), Some(path)) = (self.key(), path_of(&self.file, self.side)) else {
            return;
        };
        if self.lines.is_empty() || syntax.busy(&key) {
            return;
        }
        let want = want.min(self.lines.len() as u32 - 1);
        store.want(&key, version);
        let have = store.have(&key);
        if have > want {
            return;
        }
        syntax.send(SyntaxRequest {
            key,
            path,
            version,
            text: Arc::clone(&self.lines),
            have,
            want,
        });
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
