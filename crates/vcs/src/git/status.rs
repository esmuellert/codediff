//! `git status --porcelain=v2 -z`, in git's own vocabulary.
//!
//! This module keeps git's words — `XY` codes, the index, object ids — because
//! forcing them into neutral names would either lose meaning or invent a
//! concept other systems do not share. [`super::to_change`] is the single place
//! they are translated.
//!
//! The format is documented in `git-status(1)`. Records are NUL-terminated and
//! there are five kinds:
//!
//! ```text
//! 1 XY sub mH mI mW hH hI path                  ordinary change
//! 2 XY sub mH mI mW hH hI Xscore path NUL orig  rename or copy
//! u XY sub m1 m2 m3 mW h1 h2 h3 path            unmerged
//! ? path                                        untracked
//! ! path                                        ignored
//! ```
//!
//! **A rename record spans two NUL-terminated fields.** Splitting the stream on
//! NUL and treating every piece as a record silently turns one rename into a
//! record plus a garbage entry, so the parser consumes fields in order.
//!
//! **`-z` is not optional.** Without it git quotes any path containing a layout,
//! a quote or a non-ASCII byte, and a path containing a newline breaks the
//! format outright.

use crate::error::{Error, Result};

/// One of git's single-letter status codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Code {
    Unmodified,
    Modified,
    Added,
    Deleted,
    Renamed,
    Copied,
    /// Changed between a regular file, a symlink and a submodule.
    TypeChanged,
    Unmerged,
    Untracked,
    Ignored,
}

impl Code {
    pub fn parse(code: char) -> Option<Self> {
        Some(match code {
            '.' => Code::Unmodified,
            'M' => Code::Modified,
            'A' => Code::Added,
            'D' => Code::Deleted,
            'R' => Code::Renamed,
            'C' => Code::Copied,
            'T' => Code::TypeChanged,
            'U' => Code::Unmerged,
            '?' => Code::Untracked,
            '!' => Code::Ignored,
            _ => return None,
        })
    }

    /// Git's own letter for this code.
    ///
    /// The inverse of the parse above, and only the manifest check needs it:
    /// nothing draws an `XY` code, because nothing outside this crate can see
    /// one. It stays because the manifest is written in these letters and a
    /// check that restated them in our words would be checking itself.
    #[cfg(test)]
    pub fn letter(self) -> char {
        match self {
            Code::Unmodified => '.',
            Code::Modified => 'M',
            Code::Added => 'A',
            Code::Deleted => 'D',
            Code::Renamed => 'R',
            Code::Copied => 'C',
            Code::TypeChanged => 'T',
            Code::Unmerged => 'U',
            Code::Untracked => '?',
            Code::Ignored => '!',
        }
    }
}

/// The two codes git reports per file.
///
/// Git compares three things, not two: `HEAD`, the index and the working tree.
/// `index` is `HEAD` against the index — what committing now would record —
/// and `worktree` is the index against what is on disk.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Xy {
    pub index: Code,
    pub worktree: Code,
}

/// One record of `git status --porcelain=v2`.
///
/// Paths are plain strings, as git spelled them: parsing has no repository
/// root to resolve them against. They become a [`RepoPath`] in
/// [`to_file_diff`](crate::git::to_file_diff), which does.
///
/// [`RepoPath`]: file_types::RepoPath
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub xy: Xy,
    pub path: String,
    /// Where a renamed or copied file came from.
    pub original: Option<String>,
    /// Rename or copy similarity, 0–100.
    pub score: Option<u8>,
}

/// Which untracked files git should report.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Untracked {
    /// Every untracked file, recursing into untracked directories.
    #[default]
    All,
    /// Untracked directories collapsed to one entry.
    Normal,
    /// None at all.
    No,
}

impl Untracked {
    pub(crate) fn flag(self) -> &'static str {
        match self {
            Untracked::All => "--untracked-files=all",
            Untracked::Normal => "--untracked-files=normal",
            Untracked::No => "--untracked-files=no",
        }
    }
}

/// Parses the whole output of `git status --porcelain=v2 -z`.
pub fn parse(bytes: &[u8]) -> Result<Vec<Entry>> {
    let mut fields = Fields::new(bytes);
    let mut out = Vec::new();

    while let Some(record) = fields.next_field()? {
        if record.is_empty() {
            continue;
        }
        let (kind, rest) = record.split_once(' ').unwrap_or((record, ""));
        let entry = match kind {
            "1" => ordinary(rest)?,
            // Only this kind reads a second field.
            "2" => rename(rest, fields.next_field()?)?,
            "u" => unmerged(rest)?,
            "?" => simple(rest, Code::Untracked),
            "!" => simple(rest, Code::Ignored),
            // `#` headers appear with --branch, which we do not pass.
            "#" => continue,
            other => {
                return Err(Error::Parse {
                    what: format!("unknown status record type {other:?}"),
                });
            }
        };
        out.push(entry);
    }
    Ok(out)
}

/// `XY sub mH mI mW hH hI path` — eight fields. `splitn` leaves the whole
/// remainder in the last one, so a path containing spaces stays intact.
fn ordinary(rest: &str) -> Result<Entry> {
    let mut parts = rest.splitn(8, ' ');
    let xy = codes(parts.next())?;
    // The modes and hashes are skipped: what to show is decided by XY, and
    // content is fetched by path.
    let path = parts
        .nth(6)
        .ok_or_else(|| missing("ordinary record path"))?;
    Ok(Entry {
        xy,
        path: path.to_owned(),
        original: None,
        score: None,
    })
}

/// `XY sub mH mI mW hH hI Xscore path` — nine fields, plus the original path as
/// the next NUL-terminated field.
fn rename(rest: &str, original: Option<&str>) -> Result<Entry> {
    let mut parts = rest.splitn(9, ' ');
    let xy = codes(parts.next())?;
    let score = parts.nth(6).ok_or_else(|| missing("rename score"))?;
    let path = parts.next().ok_or_else(|| missing("rename record path"))?;
    let original = original.ok_or_else(|| missing("rename original path"))?;

    Ok(Entry {
        xy,
        path: path.to_owned(),
        original: Some(original.to_owned()),
        // "R100" or "C75": a letter then a percentage.
        score: score.get(1..).and_then(|n| n.parse().ok()),
    })
}

/// `XY sub m1 m2 m3 mW h1 h2 h3 path` — ten fields: three stages, so three
/// modes and three hashes rather than two.
fn unmerged(rest: &str) -> Result<Entry> {
    let mut parts = rest.splitn(10, ' ');
    let xy = codes(parts.next())?;
    let path = parts
        .nth(8)
        .ok_or_else(|| missing("unmerged record path"))?;
    Ok(Entry {
        xy,
        path: path.to_owned(),
        original: None,
        score: None,
    })
}

fn simple(path: &str, code: Code) -> Entry {
    Entry {
        xy: Xy {
            // Git reports no index state for these; they exist only on disk.
            index: Code::Unmodified,
            worktree: code,
        },
        path: path.to_owned(),
        original: None,
        score: None,
    }
}

fn codes(field: Option<&str>) -> Result<Xy> {
    let field = field.ok_or_else(|| missing("status codes"))?;
    let mut chars = field.chars();
    let index = chars.next().and_then(Code::parse);
    let worktree = chars.next().and_then(Code::parse);
    match (index, worktree) {
        (Some(index), Some(worktree)) => Ok(Xy { index, worktree }),
        _ => Err(Error::Parse {
            what: format!("status codes {field:?}"),
        }),
    }
}

fn missing(what: &str) -> Error {
    Error::Parse {
        what: format!("missing {what}"),
    }
}

/// Walks NUL-terminated fields, so a record needing a second one can ask.
struct Fields<'a> {
    rest: &'a [u8],
}

impl<'a> Fields<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { rest: bytes }
    }

    fn next_field(&mut self) -> Result<Option<&'a str>> {
        if self.rest.is_empty() {
            return Ok(None);
        }
        let (field, rest) = match self.rest.iter().position(|b| *b == 0) {
            Some(i) => (&self.rest[..i], &self.rest[i + 1..]),
            // Git terminates every field; a truncated read should not panic.
            None => (self.rest, &self.rest[self.rest.len()..]),
        };
        self.rest = rest;
        // Paths are bytes on Unix and need not be UTF-8. One we cannot decode
        // we could neither display nor hand back to git, so it is an error
        // rather than a lossy conversion that looks right and then fails.
        std::str::from_utf8(field)
            .map(Some)
            .map_err(|_| Error::NotUtf8 {
                command: "git status".to_owned(),
            })
    }
}

#[cfg(test)]
mod tests {
    //! On bytes captured from real git, in git's vocabulary — `XY` codes, the
    //! index, similarity scores. No repository needed, so these run everywhere
    //! and pin the shapes that are awkward to produce on demand.

    use super::*;
    use file_types::ChangeType;

    /// Builds a NUL-terminated stream the way git writes one.
    fn stream(fields: &[&str]) -> Vec<u8> {
        let mut out = Vec::new();
        for f in fields {
            out.extend_from_slice(f.as_bytes());
            out.push(0);
        }
        out
    }

    /// The ordinary comparison. No test here is about which revisions these are.
    fn revs() -> file_types::Revs {
        file_types::Revs::worktree_against(file_types::Oid::new("b87b24c"))
    }

    #[test]
    fn an_ordinary_change_carries_both_codes() {
        let bytes = stream(&["1 .M N... 100644 100644 100644 4cb29ea 4cb29ea modified.txt"]);
        let entries = parse(&bytes).expect("parses");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].path.as_str(), "modified.txt");
        assert_eq!(entries[0].xy.index, Code::Unmodified);
        assert_eq!(entries[0].xy.worktree, Code::Modified);
        assert_eq!(entries[0].original, None);
    }

    #[test]
    fn staged_and_then_edited_again_reports_two_different_codes() {
        // The one case where X and Y disagree, and the reason both are kept.
        let bytes =
            stream(&["1 MM N... 100644 100644 100644 9c59e24 e019be0 staged-then-edited.txt"]);
        let entries = parse(&bytes).expect("parses");
        assert_eq!(entries[0].xy.index, Code::Modified);
        assert_eq!(entries[0].xy.worktree, Code::Modified);
    }

    #[test]
    fn a_rename_spans_two_fields() {
        // The trap: the original path is a *separate* NUL-terminated field, so a
        // parser that splits the stream and treats each piece as a record produces
        // one rename plus one garbage entry.
        let bytes = stream(&[
            "2 R. N... 100644 100644 100644 148c84a 148c84a R100 renamed-to.txt",
            "renamed-from.txt",
            "1 .M N... 100644 100644 100644 4cb29ea 4cb29ea after.txt",
        ]);
        let entries = parse(&bytes).expect("parses");

        assert_eq!(
            entries.len(),
            2,
            "the original path must not become a record"
        );
        assert_eq!(entries[0].path.as_str(), "renamed-to.txt");
        assert_eq!(entries[0].original.as_deref(), Some("renamed-from.txt"));
        assert_eq!(entries[0].xy.index, Code::Renamed);
        assert_eq!(entries[0].score, Some(100));
        assert_eq!(
            crate::repository::changed_file::to_file_diff(
                entries[0].clone(),
                std::path::Path::new("/repo"),
                revs()
            )
            .get_change_type(),
            ChangeType::Moved
        );
        // The record after a rename must still be read correctly.
        assert_eq!(entries[1].path.as_str(), "after.txt");
    }

    #[test]
    fn a_copy_is_told_apart_from_a_rename() {
        let bytes = stream(&[
            "2 C. N... 100644 100644 100644 148c84a 148c84a C75 copy.txt",
            "source.txt",
        ]);
        let entries = parse(&bytes).expect("parses");
        assert_eq!(entries[0].xy.index, Code::Copied);
        assert_eq!(entries[0].score, Some(75));
    }

    #[test]
    fn an_unmerged_record_has_three_stages() {
        // `u` carries three modes and three hashes rather than two, so the path
        // sits at a different offset from an ordinary record.
        let bytes =
            stream(&["u UU N... 100644 100644 100644 100644 df967b9 b19a1e9 950b81b conflict.txt"]);
        let entries = parse(&bytes).expect("parses");
        assert_eq!(entries[0].path.as_str(), "conflict.txt");
        assert_eq!(
            crate::repository::changed_file::to_file_diff(
                entries[0].clone(),
                std::path::Path::new("/repo"),
                revs()
            )
            .get_change_type(),
            ChangeType::Conflicted
        );
        assert_eq!(entries[0].xy.index, Code::Unmerged);
    }

    #[test]
    fn untracked_and_ignored_are_worktree_only() {
        let bytes = stream(&["? untracked.txt", "! ignored.txt"]);
        let entries = parse(&bytes).expect("parses");
        assert_eq!(entries[0].xy.worktree, Code::Untracked);
        assert_eq!(entries[0].xy.index, Code::Unmodified);
        assert_eq!(
            crate::repository::changed_file::to_file_diff(
                entries[0].clone(),
                std::path::Path::new("/repo"),
                revs()
            )
            .get_change_type(),
            ChangeType::Untracked
        );
        assert_eq!(entries[1].xy.worktree, Code::Ignored);
    }

    #[test]
    fn a_path_containing_spaces_survives() {
        // Whitespace splitting is the obvious way to parse this format and it is
        // wrong; the path runs to the end of the field.
        let bytes = stream(&["1 .M N... 100644 100644 100644 4cb29ea 4cb29ea with spaces.txt"]);
        let entries = parse(&bytes).expect("parses");
        assert_eq!(entries[0].path.as_str(), "with spaces.txt");
    }

    #[test]
    fn a_path_outside_ascii_survives() {
        let bytes =
            stream(&["1 .M N... 100644 100644 100644 4cb29ea 4cb29ea ünïcodé-ファイル.txt"]);
        let entries = parse(&bytes).expect("parses");
        assert_eq!(entries[0].path.as_str(), "ünïcodé-ファイル.txt");
    }

    #[test]
    fn a_path_containing_a_newline_survives() {
        // The reason -z is not optional: without it this breaks the format, since
        // records would be newline-separated.
        let bytes = stream(&["1 .M N... 100644 100644 100644 4cb29ea 4cb29ea two\nlines.txt"]);
        let entries = parse(&bytes).expect("parses");
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].path.as_str(), "two\nlines.txt");
    }

    #[test]
    fn empty_output_is_a_clean_tree() {
        assert!(parse(b"").expect("parses").is_empty());
    }

    #[test]
    fn an_unknown_record_type_is_an_error_rather_than_a_silent_skip() {
        let bytes = stream(&["x something unexpected"]);
        assert!(parse(&bytes).is_err());
    }

    #[test]
    fn a_branch_header_is_ignored() {
        // Only produced with --branch, which we do not pass, but skipping it costs
        // nothing and makes the parser usable if we ever do.
        let bytes = stream(&[
            "# branch.oid 1234abcd",
            "1 .M N... 100644 100644 100644 4cb29ea 4cb29ea modified.txt",
        ]);
        let entries = parse(&bytes).expect("parses");
        assert_eq!(entries.len(), 1);
    }

    // ---- against a real repository -------------------------------------
    //
    // The manifest is written by hand in git's `XY` spelling, so the check is
    // in git's words and belongs beside the parser that produces them. It used
    // to be `codediff debug status`, which meant a subcommand printing status
    // codes it had no business knowing. See D67.

    struct Fixture {
        dir: std::path::PathBuf,
    }

    impl Fixture {
        fn new(name: &str) -> Self {
            let dir =
                std::env::temp_dir().join(format!("codediff-status-{name}-{}", std::process::id()));
            fixtures::repo(&dir).expect("building the fixture repository");
            Self { dir }
        }

        fn entries(&self) -> Vec<Entry> {
            let repo = crate::git::rev_parse::discover(&self.dir).expect("opening");
            crate::git::entries(&repo, Untracked::default(), &[]).expect("status runs")
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }

    /// `index worktree path [<- original]`, sorted — the manifest's format.
    fn render(entries: &[Entry]) -> Vec<String> {
        let mut lines: Vec<String> = entries
            .iter()
            .map(|e| {
                let mut line = format!(
                    "{}  {}  {}",
                    e.xy.index.letter(),
                    e.xy.worktree.letter(),
                    e.path
                );
                if let Some(original) = &e.original {
                    line.push_str(&format!(" <- {original}"));
                }
                line
            })
            .collect();
        lines.sort();
        lines
    }

    fn manifest(dir: &std::path::Path) -> Vec<String> {
        let text = std::fs::read_to_string(dir.join(fixtures::MANIFEST)).expect("manifest exists");
        let mut lines: Vec<String> = text
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .map(str::to_owned)
            .collect();
        lines.sort();
        lines
    }

    #[test]
    fn status_matches_the_manifest_exactly() {
        let fixture = Fixture::new("manifest");
        assert_eq!(render(&fixture.entries()), manifest(&fixture.dir));
    }

    #[test]
    fn a_file_staged_and_then_edited_again_keeps_both_codes() {
        // One entry, two codes. The reviewer's layer splits it into two
        // comparisons; this is the record it splits.
        let fixture = Fixture::new("both-codes");
        let entries = fixture.entries();
        let entry = entries
            .iter()
            .find(|e| e.path == "staged-then-edited.txt")
            .expect("reported");
        assert_eq!(entry.xy.index, Code::Modified);
        assert_eq!(entry.xy.worktree, Code::Modified);
    }
}
