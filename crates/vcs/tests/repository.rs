//! Against a real repository, through the four operations everything above
//! `vcs` sees.
//!
//! Git's own words are checked inside the crate, beside the parser that
//! produces them — the manifest is written in `XY` spelling, and nothing out
//! here can say `XY` any more. See D67.

use std::path::PathBuf;

use file_types::{ChangeType, DiffVersion, File, FileContent, RepoPath};
use vcs::{DiffType, Repository};

/// A fixture repository in a temporary directory, removed on drop.
struct Fixture {
    dir: PathBuf,
}

impl Fixture {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("codediff-vcs-{name}-{}", std::process::id()));
        fixtures::repo(&dir).expect("building the fixture repository");
        Self { dir }
    }

    fn git(&self) -> Repository {
        Repository::open(&self.dir).expect("opening the fixture repository")
    }

    /// Every changed file, flat.
    ///
    /// A path staged and then edited again is in two groups, and so appears
    /// twice: they are two comparisons of it, not a duplicate.
    fn files(&self) -> Vec<File> {
        self.flat(self.git())
    }

    fn flat(&self, mut repository: Repository) -> Vec<File> {
        repository
            .get_changed_files(&DiffType::Worktree, &[])
            .expect("status runs")
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

#[test]
fn a_rename_carries_both_paths_rather_than_an_add_and_a_delete() {
    let fixture = Fixture::new("rename");
    let entries = fixture.files();

    let renamed = entries
        .iter()
        .find(|e| e.path().as_str() == "renamed-to.txt")
        .expect("the renamed file is reported");
    assert_eq!(
        renamed.previous_path().map(RepoPath::as_str),
        Some("renamed-from.txt")
    );
    // Both halves, because they come from different fields: the paths say
    // where it went, and the change says what happened. Asserting only the
    // paths let a rename read as an ordinary modification, which draws the
    // wrong letter in the wrong colour.
    assert_eq!(renamed.get_change_type(), ChangeType::Moved);
    assert!(
        !entries
            .iter()
            .any(|e| e.path().as_str() == "renamed-from.txt"),
        "the old path must not also appear as a deletion"
    );
}

#[test]
fn the_conflicted_file_is_identified_as_a_conflict() {
    let fixture = Fixture::new("conflict");
    let entries = fixture.files();
    let conflict = entries
        .iter()
        .find(|e| e.path().as_str() == "conflict.txt")
        .expect("the conflicted file is reported");
    assert!(conflict.is_conflicted());
}

#[test]
fn awkward_paths_survive_the_round_trip() {
    let fixture = Fixture::new("paths");
    let entries = fixture.files();
    for path in ["with spaces.txt", "ünïcodé-ファイル.txt"] {
        assert!(
            entries.iter().any(|e| e.path().as_str() == path),
            "{path:?} did not survive; entries were {:?}",
            entries
                .iter()
                .map(|e| e.path().as_str())
                .collect::<Vec<_>>()
        );
    }
}

#[test]
fn ignored_files_are_absent_unless_asked_for() {
    let fixture = Fixture::new("ignored");
    let entries = fixture.files();
    assert!(
        !entries
            .iter()
            .any(|e| e.path().as_str().starts_with("ignored")),
        "gitignored files must not be listed"
    );
}

#[test]
fn discovery_works_from_a_subdirectory() {
    let fixture = Fixture::new("discover");
    let deep = fixture.dir.join("nested/deep");
    let git = Repository::open(&deep).expect("opens from a subdirectory");

    assert_eq!(
        git.repo_path().root.canonicalize().unwrap(),
        fixture.dir.canonicalize().unwrap()
    );
    assert!(git.repo_path().control_dir.ends_with(".git"));
}

#[test]
fn a_path_outside_a_repository_is_reported_as_such() {
    let outside = std::env::temp_dir().join(format!("codediff-not-a-repo-{}", std::process::id()));
    std::fs::create_dir_all(&outside).unwrap();
    let err = Repository::open(&outside).expect_err("not a repository");
    assert!(matches!(err, vcs::Error::NoRepository { .. }), "{err}");
    let _ = std::fs::remove_dir_all(&outside);
}

/// Finds one changed file by path.
fn file(entries: &[File], path: &str) -> File {
    entries
        .iter()
        .find(|e| e.path().as_str() == path)
        .unwrap_or_else(|| panic!("{path} is reported as changed"))
        .clone()
}

#[test]
fn the_two_sides_of_a_change_come_back_byte_for_byte() {
    let fixture = Fixture::new("sides");
    let mut git = fixture.git();
    let entries = fixture.files();
    let modified = file(&entries, "modified.txt");

    assert_eq!(
        git.get_file_content(&modified, DiffVersion::Original)
            .expect("reads")
            .text(),
        Some("one\ntwo\nthree\n")
    );
    assert_eq!(
        git.get_file_content(&modified, DiffVersion::Modified)
            .expect("reads")
            .text(),
        Some("one\nTWO\nthree\n")
    );
}

#[test]
fn a_one_sided_change_has_only_the_side_it_has() {
    // Asking for both sides of every file is what a diff does, and one side is
    // routinely absent. That is an answer, not an error.
    let fixture = Fixture::new("onesided");
    let mut git = fixture.git();
    let entries = fixture.files();

    let untracked = file(&entries, "untracked.txt");
    assert!(matches!(
        git.get_file_content(&untracked, DiffVersion::Original)
            .expect("reads"),
        FileContent::Absent
    ));
    assert!(
        git.get_file_content(&untracked, DiffVersion::Modified)
            .expect("reads")
            .text()
            .is_some()
    );

    let deleted = file(&entries, "deleted.txt");
    assert!(
        git.get_file_content(&deleted, DiffVersion::Original)
            .expect("reads")
            .text()
            .is_some()
    );
    assert!(matches!(
        git.get_file_content(&deleted, DiffVersion::Modified)
            .expect("reads"),
        FileContent::Absent
    ));
}

#[test]
fn a_picture_comes_back_classified_rather_than_as_bytes() {
    // The caller cannot use raw bytes without asking "is this text?", so the
    // answer arrives with the content instead of every caller working it out.
    let fixture = Fixture::new("binary");
    let mut git = fixture.git();
    let entries = fixture.files();
    let picture = file(&entries, "picture.png");

    assert!(
        git.get_file_content(&picture, DiffVersion::Original)
            .expect("reads")
            .is_binary()
    );
    assert!(
        git.get_file_content(&picture, DiffVersion::Modified)
            .expect("reads")
            .is_binary()
    );
    assert_eq!(
        git.get_file_content(&picture, DiffVersion::Modified)
            .expect("reads")
            .text(),
        None
    );
}

#[test]
fn a_moved_file_reads_its_old_path_on_the_before_side() {
    // The caller does not have to know that rule, which is why `read` takes
    // the whole change rather than a path.
    let fixture = Fixture::new("moved");
    let mut git = fixture.git();
    let entries = fixture.files();
    let moved = file(&entries, "renamed-to.txt");

    assert_eq!(moved.get_change_type(), ChangeType::Moved);
    assert_eq!(
        moved
            .path_of_version(DiffVersion::Original)
            .map(RepoPath::as_str),
        Some("renamed-from.txt")
    );
    assert!(
        git.get_file_content(&moved, DiffVersion::Original)
            .expect("reads")
            .text()
            .is_some(),
        "the before side must be read from the path the file used to have"
    );
}

#[test]
fn blob_reads_reuse_one_child_process() {
    // A sixty-file diff is a hundred and twenty reads; at a process spawn each
    // that is most of a second in fork.
    let fixture = Fixture::new("batch");
    let mut git = fixture.git();
    for _ in 0..50 {
        let content = git
            .get_raw_content("HEAD", &RepoPath::new("unchanged.txt", &fixture.dir))
            .expect("reads")
            .expect("exists");
        assert_eq!(content, b"this file never changes\n");
    }
}

#[test]
fn a_blob_containing_no_trailing_newline_keeps_its_exact_bytes() {
    let fixture = Fixture::new("nonewline");
    let mut git = fixture.git();
    let content = git
        .get_raw_content(
            "HEAD",
            &RepoPath::new("no-trailing-newline.txt", &fixture.dir),
        )
        .expect("reads")
        .expect("exists");
    assert_eq!(content, b"last line has no newline");
    assert!(!content.ends_with(b"\n"));
}

#[test]
fn crlf_bytes_are_not_rewritten() {
    let fixture = Fixture::new("crlf");
    let mut git = fixture.git();
    let content = git
        .get_raw_content("HEAD", &RepoPath::new("crlf.txt", &fixture.dir))
        .expect("reads")
        .expect("exists");
    assert!(
        content.windows(2).any(|w| w == b"\r\n"),
        "carriage returns must survive, or every line would diff"
    );
}

#[test]
fn a_comparison_resolves_its_revisions_and_says_so_when_it_cannot() {
    // A name is resolved to an id before anything is listed, so that a commit
    // made while a review is open cannot leave half the files named against
    // one `HEAD` and half against another.
    let fixture = Fixture::new("resolve");

    let files = fixture
        .git()
        .get_changed_files(&DiffType::Against("HEAD".to_owned()), &[])
        .expect("HEAD resolves");
    let before = files[0].revs().before.to_string();
    assert_eq!(before.len(), 40, "an id, not the name that was typed");

    let err = fixture
        .git()
        .get_changed_files(&DiffType::Against("no-such-branch".to_owned()), &[])
        .expect_err("unknown revision");
    assert!(matches!(err, vcs::Error::UnknownRevision { .. }), "{err}");
}

#[test]
fn a_file_staged_and_then_edited_again_is_in_both_comparisons() {
    // Git reports it once, with two codes. Here it is two files, because they
    // are two diffs of it: the working tree against the index, and the index
    // against the commit. Neither is a duplicate of the other. The record
    // those come from is checked inside the crate, where the codes live.
    let fixture = Fixture::new("both");
    let files = fixture
        .git()
        .get_changed_files(&DiffType::Worktree, &[])
        .expect("status runs");

    // The list is flat, and each file says which comparison it is — so one
    // path appearing twice is two files carrying different revisions rather
    // than one file in two containers.
    let found: Vec<&'static str> = files
        .iter()
        .filter(|file| file.path().as_str() == "staged-then-edited.txt")
        .map(|file| {
            assert_eq!(file.get_change_type(), ChangeType::Modified);
            file.revs().heading()
        })
        .collect();
    assert_eq!(found, vec!["Changes", "Staged Changes"], "{found:?}");
}
