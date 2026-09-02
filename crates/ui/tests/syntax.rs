//! Tests for SyntaxService::line_spans.

use std::path::Path;

use file_types::{DiffVersion, File, Oid, RepoPath, Revs};
use syntax::{Store, SyntaxResponse, Version};
use ui::services::syntax::SyntaxService;

fn file(path: &str) -> File {
    File::unchanged_path(
        RepoPath::new(path, Path::new("/repo")),
        Revs::worktree_against(Oid::new("abc")),
    )
}

fn span(start: u32, end: u32) -> syntax::Span {
    syntax::Span::new(start..end, syntax::Style::pen(syntax::Pen(1)))
}

fn response(key: &str, from: u32, lines: Vec<Vec<syntax::Span>>) -> SyntaxResponse {
    SyntaxResponse {
        key: key.to_owned(),
        version: Version(1),
        from,
        spans: lines,
        more: false,
    }
}

#[test]
fn line_spans_reads_from_the_store() {
    let mut store = Store::new();
    let f = file("src/app.rs");
    let key = f.name(DiffVersion::Modified).expect("a key");

    store.ensure_cache(&key, Version(1));
    store.install(response(
        &key,
        0,
        vec![vec![span(0, 2), span(3, 8)], vec![span(0, 5)], vec![]],
    ));

    // Line 1 (1-based) → index 0, two spans.
    let spans = SyntaxService::line_spans(&store, &f, DiffVersion::Modified, 1);
    assert_eq!(spans.len(), 2, "line 1 has two spans: {spans:?}");

    // Line 2 → index 1, one span.
    let spans = SyntaxService::line_spans(&store, &f, DiffVersion::Modified, 2);
    assert_eq!(spans.len(), 1, "line 2 has one span: {spans:?}");

    // Line 3 → index 2, empty.
    let spans = SyntaxService::line_spans(&store, &f, DiffVersion::Modified, 3);
    assert!(spans.is_empty(), "line 3 has no spans: {spans:?}");

    // Line 4 → not yet coloured.
    let spans = SyntaxService::line_spans(&store, &f, DiffVersion::Modified, 4);
    assert!(spans.is_empty(), "line 4 has not arrived: {spans:?}");
}

#[test]
fn line_spans_returns_empty_for_a_missing_version() {
    let store = Store::new();
    let f = file("src/app.rs");
    let spans = SyntaxService::line_spans(&store, &f, DiffVersion::Original, 1);
    assert!(spans.is_empty(), "no cache → empty: {spans:?}");
}
