//! Parses `git status --porcelain=v2 -z`.
//!
//! Records are NUL-terminated; rename and copy records consume a second path.
//! Record kinds are `1`, `2`, `u`, `?`, and `!`.

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

    /// Git's letter for this code, used by parser tests.
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

/// Git's index and worktree status codes for one path.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Xy {
    pub index: Code,
    pub worktree: Code,
}

/// One record of `git status --porcelain=v2`.
///
/// Paths are plain strings, as git spelled them: parsing has no repository
/// root to resolve them against. The repository layer turns them into
/// `file_types::RepoPath` values.
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
}

impl Untracked {
    pub(crate) fn flag(self) -> &'static str {
        match self {
            Untracked::All => "--untracked-files=all",
        }
    }
}

/// Parses `git status --porcelain=v2 -z` output.
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

/// Ordinary records have eight fields; the path is the final field.
fn ordinary(rest: &str) -> Result<Entry> {
    let mut parts = rest.splitn(8, ' ');
    let xy = codes(parts.next())?;
    // Callers fetch content by path; modes and hashes are not needed here.
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

/// Unmerged records have ten fields: three stages, modes, and hashes.
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
            // Untracked and ignored files have no index state.
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
        // Paths must be UTF-8 for display and later Git commands.
        std::str::from_utf8(field)
            .map(Some)
            .map_err(|_| Error::NotUtf8 {
                command: "git status".to_owned(),
            })
    }
}

#[cfg(test)]
mod tests {
    //! Parser tests for Git's NUL-terminated status records.

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
        // A staged-then-edited path carries both states.
        let bytes =
            stream(&["1 MM N... 100644 100644 100644 9c59e24 e019be0 staged-then-edited.txt"]);
        let entries = parse(&bytes).expect("parses");
        assert_eq!(entries[0].xy.index, Code::Modified);
        assert_eq!(entries[0].xy.worktree, Code::Modified);
    }

    #[test]
    fn a_rename_spans_two_fields() {
        // The original path is a separate NUL-terminated field.
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
            crate::repository::list::status_entry_to_file(
                entries[0].clone(),
                std::path::Path::new("/repo"),
                revs()
            )
            .get_change_type(),
            ChangeType::Moved
        );
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
        // Unmerged records have three mode/hash pairs.
        let bytes =
            stream(&["u UU N... 100644 100644 100644 100644 df967b9 b19a1e9 950b81b conflict.txt"]);
        let entries = parse(&bytes).expect("parses");
        assert_eq!(entries[0].path.as_str(), "conflict.txt");
        assert_eq!(
            crate::repository::list::status_entry_to_file(
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
            crate::repository::list::status_entry_to_file(
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
        // The path occupies the remainder of the field.
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
        // NUL framing preserves newlines in paths.
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
        // Ignore branch headers if they are present.
        let bytes = stream(&[
            "# branch.oid 1234abcd",
            "1 .M N... 100644 100644 100644 4cb29ea 4cb29ea modified.txt",
        ]);
        let entries = parse(&bytes).expect("parses");
        assert_eq!(entries.len(), 1);
    }

    // ---- repository integration -----------------------------------------

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
            let repo = crate::git::rev_parse::find_repo(&self.dir).expect("opening");
            crate::git::status_entries(&repo, Untracked::default(), &[]).expect("status runs")
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
        // One entry can represent two comparisons.
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
