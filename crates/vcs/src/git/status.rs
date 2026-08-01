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
//! **`-z` is not optional.** Without it git quotes any path containing a space,
//! a quote or a non-ASCII byte, and a path containing a newline breaks the
//! format outright.

use crate::error::{Error, Result};
use crate::path::RelPath;

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

/// A git object id, kept as text.
///
/// Never parsed into bytes: it is only handed back to git or compared, and git
/// prints abbreviated ids of varying length.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Oid(String);

impl Oid {
    pub fn new(text: impl Into<String>) -> Self {
        Self(text.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// True for the all-zero id git prints where an object does not exist,
    /// such as the after side of a deletion.
    pub fn is_null(&self) -> bool {
        !self.0.is_empty() && self.0.chars().all(|c| c == '0')
    }
}

impl std::fmt::Display for Oid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// One record of `git status --porcelain=v2`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub xy: Xy,
    pub path: RelPath,
    /// Where a renamed or copied file came from.
    pub original: Option<RelPath>,
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
        path: RelPath::new(path),
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
        path: RelPath::new(path),
        original: Some(RelPath::new(original)),
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
        path: RelPath::new(path),
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
        path: RelPath::new(path),
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
