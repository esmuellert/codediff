//! `git diff --name-status` — what differs between two things git can name.
//!
//! The format every comparison but the working tree reads. `git status` has a
//! format of its own — two codes, three record types — because it describes
//! three things at once; a diff describes two, so one letter is enough.
//!
//! Read with `-z`, so a path holding a space, a quote or a newline arrives as
//! itself rather than in git's quoted spelling. Fields are NUL-separated, and
//! a rename spends three of them: `R100`, the old path, the new path.

use file_types::{ChangeType, ChangedFile, File, RepoPath, Revs};

use crate::error::Result;
use crate::git::{Git, run};

/// One record, still in git's words.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    /// `M`, `A`, `D`, `R`, `C`, `T` or `U`.
    pub letter: char,
    pub path: String,
    /// Where a renamed or copied file came from.
    pub original: Option<String>,
    /// Rename or copy similarity, 0–100.
    pub score: Option<u8>,
}

impl Git {
    /// What differs between two things git can name.
    ///
    /// `args` is what goes after `diff` — the revisions, and `--cached` when
    /// the after side is the index. Rename detection is forced here as it is
    /// for the status, so what the list calls a rename does not depend on the
    /// reader's own configuration. See D56.
    pub fn name_status(&self, args: &[&str], pathspec: &[String]) -> Result<Vec<Change>> {
        let mut command = vec!["diff", "--name-status", "-z", "--find-renames"];
        command.extend_from_slice(args);
        if !pathspec.is_empty() {
            command.push("--");
        }
        let owned: Vec<&str> = pathspec.iter().map(String::as_str).collect();
        command.extend_from_slice(&owned);
        Ok(parse(&run::run(&self.repo().root, &command)?))
    }
}

/// Reads `--name-status -z` output.
fn parse(bytes: &[u8]) -> Vec<Change> {
    let mut changes = Vec::new();
    let mut fields = bytes
        .split(|&b| b == 0)
        .filter(|field| !field.is_empty())
        .map(|field| String::from_utf8_lossy(field).into_owned());

    while let Some(code) = fields.next() {
        let Some(letter) = code.chars().next() else {
            continue;
        };
        // The digits after `R` or `C` are how alike the two paths are. Only
        // those two letters carry them, so anything else is a code we do not
        // know and its path field is still the next one.
        let score = code[1..].parse::<u8>().ok();
        let moved = letter == 'R' || letter == 'C';
        let (original, path) = if moved {
            match (fields.next(), fields.next()) {
                (Some(from), Some(to)) => (Some(from), to),
                _ => break,
            }
        } else {
            match fields.next() {
                Some(path) => (None, path),
                None => break,
            }
        };
        changes.push(Change {
            letter,
            path,
            original,
            score,
        });
    }
    changes
}

/// One record, in the reviewer's terms.
///
/// The counterpart of [`to_file_diff`](crate::git::to_file_diff), which does
/// the same for a status record. Both live in this crate because it is the
/// only one allowed to know both vocabularies.
pub fn to_changed_file(change: Change, root: &std::path::Path, revs: Revs) -> ChangedFile {
    let path = RepoPath::new(change.path, root);
    let file = match (change.letter, change.original) {
        ('A', _) => File::added(path, revs),
        ('D', _) => File::deleted(path, revs),
        // A copy is a move as far as a reviewer is concerned: the question
        // either asks is "what does the new content say", and the old path is
        // shown beside it either way.
        ('R' | 'C', Some(from)) => File::renamed(RepoPath::new(from, root), path, revs),
        _ => File::unchanged_path(path, revs),
    };
    // `U` is an unresolved merge, which the paths cannot say. Nothing else
    // here needs a backend: added, deleted and moved are all readable from
    // the pair of paths.
    if change.letter == 'U' {
        return ChangedFile::reported(file, ChangeType::Conflicted);
    }
    ChangedFile::new(file, change.score)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_plain_record_is_a_letter_and_a_path() {
        let changes = parse(b"M\0src/main.rs\0");
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].letter, 'M');
        assert_eq!(changes[0].path, "src/main.rs");
        assert_eq!(changes[0].original, None);
    }

    #[test]
    fn a_rename_takes_three_fields_and_carries_its_score() {
        // The whole reason this cannot split on NUL and take pairs.
        let changes = parse(b"R100\0old.rs\0new.rs\0M\0after.rs\0");
        assert_eq!(changes.len(), 2, "and the record after it still parses");
        assert_eq!(changes[0].letter, 'R');
        assert_eq!(changes[0].original.as_deref(), Some("old.rs"));
        assert_eq!(changes[0].path, "new.rs");
        assert_eq!(changes[0].score, Some(100));
        assert_eq!(changes[1].path, "after.rs");
    }

    #[test]
    fn a_path_holding_a_newline_arrives_whole() {
        // What `-z` is for. With the default framing this would be two
        // records, and every record after it would be wrong.
        let changes = parse(b"M\0we\nird.txt\0");
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].path, "we\nird.txt");
    }

    #[test]
    fn a_truncated_record_is_dropped_rather_than_guessed() {
        // A killed git leaves a half-written last record. Taking the letter
        // and inventing a path would put a file in the list that is not there.
        assert!(parse(b"M\0").is_empty());
        assert!(parse(b"R100\0only-one-path.txt\0").is_empty());
    }

    #[test]
    fn nothing_in_means_nothing_out() {
        assert!(parse(b"").is_empty());
    }
}
