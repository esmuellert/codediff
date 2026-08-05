//! Parsing `git status --porcelain=v2 -z`, on bytes captured from real git.
//!
//! In git's vocabulary, since that is the layer under test: `XY` codes, the
//! index, similarity scores. No repository needed, so these run everywhere and
//! pin the shapes that are awkward to produce on demand.

use file_types::ChangeType;
use vcs::git::{Code, status, to_file_diff};

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
    let entries = status::parse(&bytes).expect("parses");
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].path.as_str(), "modified.txt");
    assert_eq!(entries[0].xy.index, Code::Unmodified);
    assert_eq!(entries[0].xy.worktree, Code::Modified);
    assert_eq!(entries[0].original, None);
}

#[test]
fn staged_and_then_edited_again_reports_two_different_codes() {
    // The one case where X and Y disagree, and the reason both are kept.
    let bytes = stream(&["1 MM N... 100644 100644 100644 9c59e24 e019be0 staged-then-edited.txt"]);
    let entries = status::parse(&bytes).expect("parses");
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
    let entries = status::parse(&bytes).expect("parses");

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
        to_file_diff(entries[0].clone(), std::path::Path::new("/repo"), revs()).change(),
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
    let entries = status::parse(&bytes).expect("parses");
    assert_eq!(entries[0].xy.index, Code::Copied);
    assert_eq!(entries[0].score, Some(75));
}

#[test]
fn an_unmerged_record_has_three_stages() {
    // `u` carries three modes and three hashes rather than two, so the path
    // sits at a different offset from an ordinary record.
    let bytes =
        stream(&["u UU N... 100644 100644 100644 100644 df967b9 b19a1e9 950b81b conflict.txt"]);
    let entries = status::parse(&bytes).expect("parses");
    assert_eq!(entries[0].path.as_str(), "conflict.txt");
    assert_eq!(
        to_file_diff(entries[0].clone(), std::path::Path::new("/repo"), revs()).change(),
        ChangeType::Conflicted
    );
    assert_eq!(entries[0].xy.index, Code::Unmerged);
}

#[test]
fn untracked_and_ignored_are_worktree_only() {
    let bytes = stream(&["? untracked.txt", "! ignored.txt"]);
    let entries = status::parse(&bytes).expect("parses");
    assert_eq!(entries[0].xy.worktree, Code::Untracked);
    assert_eq!(entries[0].xy.index, Code::Unmodified);
    assert_eq!(
        to_file_diff(entries[0].clone(), std::path::Path::new("/repo"), revs()).change(),
        ChangeType::Untracked
    );
    assert_eq!(entries[1].xy.worktree, Code::Ignored);
}

#[test]
fn a_path_containing_spaces_survives() {
    // Whitespace splitting is the obvious way to parse this format and it is
    // wrong; the path runs to the end of the field.
    let bytes = stream(&["1 .M N... 100644 100644 100644 4cb29ea 4cb29ea with spaces.txt"]);
    let entries = status::parse(&bytes).expect("parses");
    assert_eq!(entries[0].path.as_str(), "with spaces.txt");
}

#[test]
fn a_path_outside_ascii_survives() {
    let bytes = stream(&["1 .M N... 100644 100644 100644 4cb29ea 4cb29ea ünïcodé-ファイル.txt"]);
    let entries = status::parse(&bytes).expect("parses");
    assert_eq!(entries[0].path.as_str(), "ünïcodé-ファイル.txt");
}

#[test]
fn a_path_containing_a_newline_survives() {
    // The reason -z is not optional: without it this breaks the format, since
    // records would be newline-separated.
    let bytes = stream(&["1 .M N... 100644 100644 100644 4cb29ea 4cb29ea two\nlines.txt"]);
    let entries = status::parse(&bytes).expect("parses");
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].path.as_str(), "two\nlines.txt");
}

#[test]
fn empty_output_is_a_clean_tree() {
    assert!(status::parse(b"").expect("parses").is_empty());
}

#[test]
fn an_unknown_record_type_is_an_error_rather_than_a_silent_skip() {
    let bytes = stream(&["x something unexpected"]);
    assert!(status::parse(&bytes).is_err());
}

#[test]
fn a_branch_header_is_ignored() {
    // Only produced with --branch, which we do not pass, but skipping it costs
    // nothing and makes the parser usable if we ever do.
    let bytes = stream(&[
        "# branch.oid 1234abcd",
        "1 .M N... 100644 100644 100644 4cb29ea 4cb29ea modified.txt",
    ]);
    let entries = status::parse(&bytes).expect("parses");
    assert_eq!(entries.len(), 1);
}
