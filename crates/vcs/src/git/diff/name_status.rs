//! Parses `git diff --name-status -z`.
//!
//! Fields are NUL-separated. A rename or copy contains its score, new path,
//! and original path.

use crate::Repo;
use crate::error::Result;

/// One parsed name-status record.
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

const FORMAT: &str = "--name-status";

/// Runs `git diff` with `args` and `pathspec`.
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
        // Rename and copy scores follow the status letter; their two paths follow.
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
        // Rename records contain two paths.
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
        // NUL framing keeps newlines inside a path.
        let changes = parse(b"M\0we\nird.txt\0");
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].path, "we\nird.txt");
    }

    #[test]
    fn a_truncated_record_is_dropped_rather_than_guessed() {
        // Incomplete records are ignored.
        assert!(parse(b"M\0").is_empty());
        assert!(parse(b"R100\0only-one-path.txt\0").is_empty());
    }

    #[test]
    fn nothing_in_means_nothing_out() {
        assert!(parse(b"").is_empty());
    }
}
