//! Against a real repository.
//!
//! Mostly through the neutral [`Diff`] trait, which is what the rest of the
//! program sees. The manifest comparison reaches into `git`'s own layer, since
//! the manifest is written in git's `XY` spelling and there is nothing to gain
//! from restating it in ours.

use std::path::{Path, PathBuf};

use vcs::git::Untracked;
use vcs::{Content, Diff, DiffKind, FileDiff, Git, RelPath};

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

    fn git(&self) -> Git {
        Git::open(&self.dir).expect("opening the fixture repository")
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// `index worktree path [<- original]`, sorted — the manifest's format, which
/// is git's own.
fn render(entries: &[vcs::git::Entry]) -> Vec<String> {
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

fn manifest(dir: &Path) -> Vec<String> {
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
    let entries = fixture.git().entries().expect("status runs");
    assert_eq!(render(&entries), manifest(&fixture.dir));
}

#[test]
fn a_rename_carries_both_paths_rather_than_an_add_and_a_delete() {
    let fixture = Fixture::new("rename");
    let entries = fixture.git().files().expect("status runs");

    let renamed = entries
        .iter()
        .find(|e| e.path.as_str() == "renamed-to.txt")
        .expect("the renamed file is reported");
    assert_eq!(
        renamed.previous_path.as_ref().map(|p| p.as_str()),
        Some("renamed-from.txt")
    );
    assert!(
        !entries
            .iter()
            .any(|e| e.path.as_str() == "renamed-from.txt"),
        "the old path must not also appear as a deletion"
    );
}

#[test]
fn the_conflicted_file_is_identified_as_a_conflict() {
    let fixture = Fixture::new("conflict");
    let entries = fixture.git().files().expect("status runs");
    let conflict = entries
        .iter()
        .find(|e| e.path.as_str() == "conflict.txt")
        .expect("the conflicted file is reported");
    assert!(conflict.is_conflicted());
}

#[test]
fn awkward_paths_survive_the_round_trip() {
    let fixture = Fixture::new("paths");
    let entries = fixture.git().files().expect("status runs");
    for path in ["with spaces.txt", "ünïcodé-ファイル.txt"] {
        assert!(
            entries.iter().any(|e| e.path.as_str() == path),
            "{path:?} did not survive; entries were {:?}",
            entries.iter().map(|e| e.path.as_str()).collect::<Vec<_>>()
        );
    }
}

#[test]
fn ignored_files_are_absent_unless_asked_for() {
    let fixture = Fixture::new("ignored");
    let entries = fixture.git().files().expect("status runs");
    assert!(
        !entries
            .iter()
            .any(|e| e.path.as_str().starts_with("ignored")),
        "gitignored files must not be listed"
    );
}

#[test]
fn untracked_directories_collapse_when_asked() {
    let fixture = Fixture::new("untracked");

    let all = fixture.git().files().expect("status runs");
    assert!(
        all.iter()
            .any(|e| e.path.as_str() == "untracked-dir/inside.txt")
    );

    let mut normal = fixture.git().with_untracked(Untracked::Normal);
    let collapsed = normal.files().expect("status runs");
    assert!(
        collapsed
            .iter()
            .any(|e| e.path.as_str() == "untracked-dir/"),
        "Normal mode should report the directory, not its contents"
    );

    let mut none = fixture.git().with_untracked(Untracked::No);
    let without = none.files().expect("status runs");
    assert!(!without.iter().any(|e| e.kind == DiffKind::Untracked));
}

#[test]
fn discovery_works_from_a_subdirectory() {
    let fixture = Fixture::new("discover");
    let deep = fixture.dir.join("nested/deep");
    let git = Git::open(&deep).expect("opens from a subdirectory");

    assert_eq!(
        git.repo().root.canonicalize().unwrap(),
        fixture.dir.canonicalize().unwrap()
    );
    assert!(git.repo().control_dir.ends_with(".git"));
}

#[test]
fn a_path_outside_a_repository_is_reported_as_such() {
    let outside = std::env::temp_dir().join(format!("codediff-not-a-repo-{}", std::process::id()));
    std::fs::create_dir_all(&outside).unwrap();
    let err = Git::open(&outside).expect_err("not a repository");
    assert!(matches!(err, vcs::Error::NoRepository { .. }), "{err}");
    let _ = std::fs::remove_dir_all(&outside);
}

/// Finds one changed file by path.
fn file(entries: &[FileDiff], path: &str) -> FileDiff {
    entries
        .iter()
        .find(|e| e.path.as_str() == path)
        .unwrap_or_else(|| panic!("{path} is reported as changed"))
        .clone()
}

#[test]
fn the_two_sides_of_a_change_come_back_byte_for_byte() {
    let fixture = Fixture::new("sides");
    let mut git = fixture.git();
    let entries = git.files().expect("status runs");
    let modified = file(&entries, "modified.txt");

    assert_eq!(
        git.before(&modified).expect("reads").text(),
        Some("one\ntwo\nthree\n")
    );
    assert_eq!(
        git.after(&modified).expect("reads").text(),
        Some("one\nTWO\nthree\n")
    );
}

#[test]
fn a_one_sided_change_has_only_the_side_it_has() {
    // Asking for both sides of every file is what a diff does, and one side is
    // routinely absent. That is an answer, not an error.
    let fixture = Fixture::new("onesided");
    let mut git = fixture.git();
    let entries = git.files().expect("status runs");

    let untracked = file(&entries, "untracked.txt");
    assert!(matches!(
        git.before(&untracked).expect("reads"),
        Content::Absent
    ));
    assert!(git.after(&untracked).expect("reads").text().is_some());

    let deleted = file(&entries, "deleted.txt");
    assert!(git.before(&deleted).expect("reads").text().is_some());
    assert!(matches!(
        git.after(&deleted).expect("reads"),
        Content::Absent
    ));
}

#[test]
fn a_picture_comes_back_classified_rather_than_as_bytes() {
    // The caller cannot use raw bytes without asking "is this text?", so the
    // answer arrives with the content instead of every caller working it out.
    let fixture = Fixture::new("binary");
    let mut git = fixture.git();
    let entries = git.files().expect("status runs");
    let picture = file(&entries, "picture.png");

    assert!(git.before(&picture).expect("reads").is_binary());
    assert!(git.after(&picture).expect("reads").is_binary());
    assert_eq!(git.after(&picture).expect("reads").text(), None);
}

#[test]
fn a_moved_file_reads_its_old_path_on_the_before_side() {
    // The caller does not have to know that rule, which is why `before` takes
    // the whole change rather than a path.
    let fixture = Fixture::new("moved");
    let mut git = fixture.git();
    let entries = git.files().expect("status runs");
    let moved = file(&entries, "renamed-to.txt");

    assert_eq!(moved.kind, DiffKind::Moved);
    assert_eq!(moved.before_path().as_str(), "renamed-from.txt");
    assert!(
        git.before(&moved).expect("reads").text().is_some(),
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
            .cat_file("HEAD", &RelPath::new("unchanged.txt"))
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
        .cat_file("HEAD", &RelPath::new("no-trailing-newline.txt"))
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
        .cat_file("HEAD", &RelPath::new("crlf.txt"))
        .expect("reads")
        .expect("exists");
    assert!(
        content.windows(2).any(|w| w == b"\r\n"),
        "carriage returns must survive, or every line would diff"
    );
}

#[test]
fn revisions_resolve_and_unknown_ones_fail() {
    let fixture = Fixture::new("resolve");
    let git = fixture.git();

    let head = git.resolve("HEAD").expect("HEAD resolves");
    assert_eq!(head.as_str().len(), 40);
    assert!(!head.is_null());

    let err = git.resolve("no-such-branch").expect_err("unknown revision");
    assert!(matches!(err, vcs::Error::UnknownRevision { .. }), "{err}");
}

#[test]
fn a_file_staged_then_edited_again_is_one_entry() {
    let fixture = Fixture::new("both");
    let entries = fixture.git().files().expect("status runs");
    let entry = file(&entries, "staged-then-edited.txt");
    assert_eq!(entry.kind, DiffKind::Modified);

    // Git's own layer still has both codes for anything that wants them.
    let raw = fixture.git().entries().expect("status runs");
    let raw = raw
        .iter()
        .find(|e| e.path.as_str() == "staged-then-edited.txt")
        .expect("reported");
    assert_eq!(raw.xy.index, vcs::git::Code::Modified);
    assert_eq!(raw.xy.worktree, vcs::git::Code::Modified);
}
