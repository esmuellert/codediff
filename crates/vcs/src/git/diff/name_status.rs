//! `git diff --name-status` — what differs between two things git can name.
//!
//! The format every comparison but the working tree reads. `git status` has a
//! format of its own — two codes, three record types — because it describes
//! three things at once; a diff describes two, so one letter is enough.
//!
//! Read with `-z`, so a path holding a space, a quote or a newline arrives as
//! itself rather than in git's quoted spelling. Fields are NUL-separated, and
//! a rename spends three of them: `R100`, the old path, the new path.

use crate::Repo;
use crate::error::Result;

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

/// The flag that shapes the output. See the `diff` module doc.
const FORMAT: &str = "--name-status";

/// What differs between two things git can name.
///
/// `args` is what goes after `diff` — the revisions, and `--cached` when the
/// after side is the index.
pub fn run(repo: &Repo, args: &[&str], pathspec: &[String]) -> Result<Vec<Change>> {
    let command = super::command(FORMAT, args, pathspec);
    Ok(parse(&crate::git::run::run(&repo.root, &command)?))
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
