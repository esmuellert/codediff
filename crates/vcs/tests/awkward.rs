//! Repositories that are legal but not ordinary.
//!
//! Each of these was a real failure found by driving the interface, not a
//! hypothetical: a repository with no commit yet refused to open at all, and a
//! symlink was read through, so an unchanged link looked like a whole file
//! rewritten.

use std::path::{Path, PathBuf};
use std::process::Command;

use file_types::DiffVersion;
use vcs::Git;

/// Where each comparison sits in what `worktree_changes` returns: the working
/// tree against the index first, then the index against the commit.
const UNSTAGED: usize = 0;
const STAGED: usize = 1;

/// A repository built by hand, removed on drop.
struct Repo {
    dir: PathBuf,
}

impl Repo {
    fn new(name: &str) -> Self {
        let dir =
            std::env::temp_dir().join(format!("codediff-awkward-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("a directory");
        let repo = Self { dir };
        repo.git(&["init", "-q"]);
        repo.git(&["config", "user.email", "test@example.com"]);
        repo.git(&["config", "user.name", "Test"]);
        repo
    }

    fn git(&self, args: &[&str]) {
        let out = Command::new("git")
            .args(args)
            .current_dir(&self.dir)
            .output()
            .expect("running git");
        assert!(out.status.success(), "git {args:?}: {out:?}");
    }

    fn write(&self, path: &str, text: &str) {
        std::fs::write(self.dir.join(path), text).expect("writing a file");
    }

    fn open(&self) -> Git {
        Git::open(&self.dir).expect("opening the repository")
    }

    fn path(&self) -> &Path {
        &self.dir
    }
}

impl Drop for Repo {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

#[test]
fn a_repository_with_no_commit_yet_lists_what_is_staged() {
    // `git init` then `git add` is the moment a reviewer has the most to look
    // at, and it used to fail outright: there is no HEAD to resolve.
    let repo = Repo::new("unborn");
    repo.write("a.txt", "hello\n");
    repo.git(&["add", "-A"]);

    let mut git = repo.open();
    let changes = git
        .worktree_changes(&[])
        .expect("listing an unborn repository");
    assert_eq!(changes[STAGED].files.len(), 1);
    assert_eq!(changes[STAGED].files[0].path().as_str(), "a.txt");

    // And the file reads, with nothing on the before side.
    let content = git
        .read(&changes[STAGED].files[0], DiffVersion::Original)
        .expect("reading the before side");
    assert!(
        matches!(content, file_types::FileContent::Absent),
        "an empty tree holds nothing"
    );
}

#[test]
fn a_rename_is_counted_the_same_whatever_the_reader_has_configured() {
    // The status forces rename detection and numstat did not, so a reader with
    // `diff.renames=false` was shown a row that called a file a rename and
    // counted it as a whole new file.
    let repo = Repo::new("renames");
    repo.write("f.txt", "a\nb\nc\nd\ne\nf\ng\nh\n");
    repo.git(&["add", "-A"]);
    repo.git(&["commit", "-qm", "first"]);
    repo.git(&["config", "diff.renames", "false"]);
    repo.git(&["mv", "f.txt", "g.txt"]);

    let git = repo.open();
    let counts = git.staged_counts().expect("counting");
    // The new name must be *in* the map. Defaulting a missing entry to zero
    // let an empty map pass, which is every way this could be broken.
    let stats = counts
        .get("g.txt")
        .unwrap_or_else(|| panic!("g.txt is not counted at all: {counts:?}"));
    // Zero and zero, which the row draws as nothing. Without rename detection
    // git sees an add and a delete instead and reports the whole file as
    // gained — which is what the reader saw.
    assert!(
        stats.is_empty(),
        "a pure rename changed no lines, whatever the config says: {stats:?}"
    );
    assert_eq!(counts.get("f.txt"), None, "and the old name is not counted");
}

#[test]
fn a_symlink_is_its_target_and_not_the_file_it_points_at() {
    let repo = Repo::new("symlink");
    repo.write("real.txt", "many\nlines\nof\ntext\n");
    std::os::unix::fs::symlink("real.txt", repo.path().join("link.txt")).expect("a link");
    repo.git(&["add", "-A"]);
    repo.git(&["commit", "-qm", "first"]);
    std::fs::remove_file(repo.path().join("link.txt")).expect("removing the link");
    std::os::unix::fs::symlink("other.txt", repo.path().join("link.txt")).expect("a new link");

    let mut git = repo.open();
    let changes = git.worktree_changes(&[]).expect("listing");
    let link = changes[UNSTAGED]
        .files
        .iter()
        .find(|f| f.path().as_str() == "link.txt")
        .expect("the link is listed");
    let content = git
        .read(link, DiffVersion::Modified)
        .expect("reading the link");
    match content {
        file_types::FileContent::Text(text) => assert_eq!(
            text.trim_end(),
            "other.txt",
            "the link's target, not four lines of the file it points at"
        ),
        other => panic!("expected the target as text, got {other:?}"),
    }
}

#[test]
fn a_file_staged_and_then_edited_again_is_two_different_comparisons() {
    let repo = Repo::new("mm");
    repo.write("a.txt", "one\n");
    repo.git(&["add", "-A"]);
    repo.git(&["commit", "-qm", "first"]);
    repo.write("a.txt", "two\n");
    repo.git(&["add", "-A"]);
    repo.write("a.txt", "three\n");

    let mut git = repo.open();
    let changes = git.worktree_changes(&[]).expect("listing");
    assert_eq!(
        changes[UNSTAGED].files.len(),
        1,
        "the working tree against the index"
    );
    assert_eq!(
        changes[STAGED].files.len(),
        1,
        "the index against the commit"
    );

    let unstaged = git
        .read(&changes[UNSTAGED].files[0], DiffVersion::Original)
        .expect("reading");
    let staged = git
        .read(&changes[STAGED].files[0], DiffVersion::Original)
        .expect("reading");
    // The whole reason one row could not show both: their before sides are
    // different files.
    assert_ne!(
        format!("{unstaged:?}"),
        format!("{staged:?}"),
        "each row compares against its own revision"
    );
}

#[test]
fn a_repository_that_converts_line_endings_diffs_only_what_changed() {
    // With `core.autocrlf` git stores LF and checks out CRLF, so comparing the
    // stored bytes with the bytes on disk marked *every* line changed. The
    // stored side has to be converted the way a checkout would convert it.
    let repo = Repo::new("autocrlf");
    repo.git(&["config", "core.autocrlf", "true"]);
    repo.write("a.txt", "one\ntwo\nthree\nfour\n");
    repo.git(&["add", "-A"]);
    repo.git(&["commit", "-qm", "first"]);
    repo.write("a.txt", "one\r\nTWO\r\nthree\r\nfour\r\n");

    let mut git = repo.open();
    let changes = git.worktree_changes(&[]).expect("listing");
    let file = changes[UNSTAGED]
        .files
        .iter()
        .find(|f| f.path().as_str() == "a.txt")
        .expect("the file is listed");

    let before = git.read(file, DiffVersion::Original).expect("reading");
    let after = git.read(file, DiffVersion::Modified).expect("reading");
    let (file_types::FileContent::Text(before), file_types::FileContent::Text(after)) =
        (before, after)
    else {
        panic!("both sides are text");
    };
    // Split on the newline rather than by `lines()`, which strips a trailing
    // carriage return and so cannot tell the two forms apart at all — the
    // first version of this test did that and passed with the fix removed.
    let same = before
        .split('\n')
        .zip(after.split('\n'))
        .filter(|(a, b)| a == b)
        .count();
    assert_eq!(
        same, 4,
        "three lines and the empty tail should be identical:\n{before:?}\n{after:?}"
    );
}
