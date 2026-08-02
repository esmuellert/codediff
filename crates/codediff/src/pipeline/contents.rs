//! The file, and its two versions.
//!
//! The second stage, and the last one that performs IO. Everything after this
//! is pure computation over the two texts it produces.

use align::Side;
use anyhow::{Context, Result};
use vcs::{Content, Diff as _, FileDiff};
use vscode_diff::lines;

use crate::pipeline::resolver::Resolved;

/// One file, with both sides read.
pub struct Contents {
    pub file: FileDiff,
    pub before: Content,
    pub after: Content,
}

/// Answers stage two: get the two texts.
pub fn read(resolved: Resolved) -> Result<Contents> {
    let Resolved { mut git, file } = resolved;
    let before = git.before(&file).context("reading the before side")?;
    let after = git.after(&file).context("reading the after side")?;
    Ok(Contents {
        file,
        before,
        after,
    })
}

impl Contents {
    /// A picture has no lines, so there is nothing to align.
    pub fn is_binary(&self) -> bool {
        self.before.is_binary() || self.after.is_binary()
    }

    /// The one side this file exists on, or `None` when it exists on both.
    ///
    /// `Some` means there is nothing to compare against: an added file has only
    /// a modified side, a deleted one only an original.
    ///
    /// The distinction is *absent*, never *empty*: a tracked file emptied to
    /// zero bytes still has a side to compare against, and gets a real
    /// two-column diff showing every line deleted. Only a file that does not
    /// exist on one side is left uncompared. See D23.
    pub fn only(&self) -> Option<Side> {
        match (self.before.text(), self.after.text()) {
            (None, Some(_)) => Some(Side::Modified),
            (Some(_), None) => Some(Side::Original),
            _ => None,
        }
    }

    /// The lines of one side. Empty — genuinely — if that side is absent.
    pub fn side(&self, side: Side) -> Vec<&str> {
        let content = match side {
            Side::Original => &self.before,
            Side::Modified => &self.after,
        };
        content.text().map(lines).unwrap_or_default()
    }

    /// What the status line calls this file.
    ///
    /// A one-sided file says so, because with a single pane there is otherwise
    /// nothing on screen to distinguish a new file from an unchanged one.
    /// VSCode does not need this: it opens such a file in an ordinary editor
    /// tab, and the tab itself is the cue.
    pub fn label(&self) -> String {
        let path = self.file.path.as_str();
        let name = match &self.file.previous_path {
            Some(previous) => format!("{} → {path}", previous.as_str()),
            None => path.to_owned(),
        };
        match self.only() {
            Some(Side::Modified) => format!("{name}   (added)"),
            Some(Side::Original) => format!("{name}   (deleted)"),
            None => name,
        }
    }
}
