//! How many lines each file gained and lost.
//!
//! `git diff --numstat`, which prints a tab-separated count per file and `-`
//! for anything it could not count. Two runs, because the explorer shows two
//! comparisons and one number cannot describe both.
//!
//! Separate from the status because it is a separate question, and an
//! expensive one: it reads the content of every changed file, where a status
//! reads none of it.

use std::collections::HashMap;

use crate::error::Result;
use file_types::Stats;

use crate::git::{Git, run};

/// Lines gained and lost, by the path git spelled.
pub type Counts = HashMap<String, Stats>;

/// Forced, because the status is read with it forced.
///
/// A reader with `diff.renames=false` in their config would otherwise get a
/// list that calls a file a rename and counts it as a whole new file — the two
/// commands would be describing different sets of changes. Neither of them may
/// be left to a setting the other does not see.
const RENAMES: &str = "--find-renames";

impl Git {
    /// The line counts for the working tree against the index.
    pub fn unstaged_counts(&self) -> Result<Counts> {
        self.counts(&["diff", "--numstat", "-z", RENAMES])
    }

    /// The line counts for the index against the commit.
    pub fn staged_counts(&self) -> Result<Counts> {
        self.counts(&["diff", "--numstat", "-z", RENAMES, "--cached"])
    }

    /// The line counts for any comparison `git diff` can name.
    ///
    /// The counterpart of the two above for every other diff type: the same
    /// numbers, against arguments the caller chose.
    pub fn diff_counts(&self, args: &[&str], pathspec: &[String]) -> Result<Counts> {
        let mut command = vec!["diff", "--numstat", "-z", RENAMES];
        command.extend_from_slice(args);
        if !pathspec.is_empty() {
            command.push("--");
        }
        let owned: Vec<&str> = pathspec.iter().map(String::as_str).collect();
        command.extend_from_slice(&owned);
        self.counts(&command)
    }

    fn counts(&self, args: &[&str]) -> Result<Counts> {
        let out = run::run(&self.repo().root, args)?;
        Ok(parse(&out))
    }
}

/// Reads `--numstat -z` output.
///
/// With `-z` the fields are tab-separated and each record ends with a NUL, so
/// a path holding a space, a quote or a newline arrives as itself rather than
/// as git's quoted spelling. A rename spends two extra records on its old and
/// new paths, which is why this cannot simply split on NUL and take threes.
fn parse(bytes: &[u8]) -> Counts {
    let mut counts = Counts::new();
    let mut records = bytes
        .split(|&b| b == 0)
        .filter(|record| !record.is_empty())
        .map(|record| String::from_utf8_lossy(record).into_owned());

    while let Some(record) = records.next() {
        let mut fields = record.splitn(3, '\t');
        let (Some(added), Some(removed), Some(path)) =
            (fields.next(), fields.next(), fields.next())
        else {
            continue;
        };
        // A rename leaves its path field empty and puts the old and the new
        // path in the two records after it.
        let path = if path.is_empty() {
            let (_old, new) = (records.next(), records.next());
            match new {
                Some(path) => path,
                None => continue,
            }
        } else {
            path.to_owned()
        };
        // `-` where a number should be means git did not count the lines,
        // which is what it prints for a binary file. Zero would claim a
        // measurement that was never made, so the file is left out entirely.
        let (Ok(added), Ok(removed)) = (added.parse(), removed.parse()) else {
            continue;
        };
        counts.insert(path, Stats::new(added, removed));
    }
    counts
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_plain_record_is_two_counts_and_a_path() {
        let counts = parse(b"4\t2\tsrc/main.rs\0");
        assert_eq!(counts.get("src/main.rs"), Some(&Stats::new(4, 2)));
    }

    #[test]
    fn a_path_with_a_tab_in_it_still_parses() {
        // The reason for `splitn(3, ..)`: splitting on every tab would cut the
        // path in half and file the counts under a name no file has.
        let counts = parse(b"1\t0\tan\tawkward.txt\0");
        assert_eq!(counts.get("an\tawkward.txt"), Some(&Stats::new(1, 0)));
    }

    #[test]
    fn a_rename_is_counted_against_its_new_path() {
        // Three records: the counts with an empty path, then old, then new.
        let counts = parse(b"3\t1\t\0old.rs\0new.rs\0");
        assert_eq!(counts.get("new.rs"), Some(&Stats::new(3, 1)));
        assert_eq!(counts.get("old.rs"), None, "the name it no longer has");
    }

    #[test]
    fn a_file_git_could_not_count_is_left_out() {
        // A picture. Recording zero would say it did not change, which is the
        // one thing that is certainly false about a file in this list.
        let counts = parse(b"-\t-\tpicture.png\0");
        assert!(counts.is_empty());
    }

    #[test]
    fn several_records_are_all_read() {
        let counts = parse(b"1\t0\ta.rs\0-\t-\tb.png\0" as &[u8]);
        assert_eq!(counts.len(), 1);
        assert_eq!(counts.get("a.rs"), Some(&Stats::new(1, 0)));
    }
}
