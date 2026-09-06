//! Integration tests for unusual repository states.

use std::path::PathBuf;
use std::process::Command;

use file_types::{DiffVersion, File, RepoPath};
use vcs::{DiffType, Repository};

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

    fn open(&self) -> Repository {
        Repository::open(&self.dir).expect("opening the repository")
    }

    fn path(&self) -> &std::path::Path {
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
    // An unborn repository uses the empty tree as its before side.
    let repo = Repo::new("unborn");
    repo.write("a.txt", "hello\n");
    repo.git(&["add", "-A"]);

    let mut git = repo.open();
    let changes = git
        .get_changed_files(&DiffType::Worktree, &[])
        .expect("listing an unborn repository");
    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].path().as_str(), "a.txt");

    // And the file reads, with nothing on the before side.
    let content = git
        .get_file_content(&changes[0], DiffVersion::Original)
        .expect("reading the before side");
    assert!(
        matches!(content, file_types::FileContent::Absent),
        "an empty tree holds nothing"
    );
}

#[test]
fn a_rename_is_counted_the_same_whatever_the_reader_has_configured() {
    // Status and numstat must use the same rename detection.
    let repo = Repo::new("renames");
    repo.write("f.txt", "a\nb\nc\nd\ne\nf\ng\nh\n");
    repo.git(&["add", "-A"]);
    repo.git(&["commit", "-qm", "first"]);
    repo.git(&["config", "diff.renames", "false"]);
    repo.git(&["mv", "f.txt", "g.txt"]);

    let mut git = repo.open();
    let files = git
        .get_changed_files(&DiffType::Worktree, &[])
        .expect("listing");
    let counts = git
        .get_line_stats(&DiffType::Worktree, &[])
        .expect("counting");
    let moved = files
        .iter()
        .find(|file| file.path().as_str() == "g.txt")
        .expect("the renamed file is listed");
    // The new name must be counted at all. Defaulting a missing entry to zero
    // let an empty map pass, which is every way this could be broken.
    let stats = counts
        .of(moved)
        .unwrap_or_else(|| panic!("g.txt is not counted at all: {counts:?}"));
    // A pure rename has no line changes.
    assert!(
        stats.is_empty(),
        "a pure rename changed no lines, whatever the config says: {stats:?}"
    );
    let old = File::deleted(RepoPath::new("f.txt", repo.path()), moved.revs());
    assert_eq!(counts.of(&old), None, "and the old name is not counted");
}

#[test]
#[cfg(unix)]
fn a_symlink_is_its_target_and_not_the_file_it_points_at() {
    let repo = Repo::new("symlink");
    repo.write("real.txt", "many\nlines\nof\ntext\n");
    std::os::unix::fs::symlink("real.txt", repo.path().join("link.txt")).expect("a link");
    repo.git(&["add", "-A"]);
    repo.git(&["commit", "-qm", "first"]);
    std::fs::remove_file(repo.path().join("link.txt")).expect("removing the link");
    std::os::unix::fs::symlink("other.txt", repo.path().join("link.txt")).expect("a new link");

    let mut git = repo.open();
    let changes = git
        .get_changed_files(&DiffType::Worktree, &[])
        .expect("listing");
    let link = changes
        .iter()
        .find(|f| f.path().as_str() == "link.txt")
        .expect("the link is listed");
    let content = git
        .get_file_content(link, DiffVersion::Modified)
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
    let changes = git
        .get_changed_files(&DiffType::Worktree, &[])
        .expect("listing");
    // Each entry carries its own comparison revisions.
    assert_eq!(changes.len(), 2, "one path, two comparisons");
    assert_eq!(changes[0].revs().after, file_types::Rev::Worktree);
    assert_eq!(changes[1].revs().after, file_types::Rev::Index);

    let unstaged = git
        .get_file_content(&changes[0], DiffVersion::Original)
        .expect("reading");
    let staged = git
        .get_file_content(&changes[1], DiffVersion::Original)
        .expect("reading");
    // The before sides use different revisions.
    assert_ne!(
        format!("{unstaged:?}"),
        format!("{staged:?}"),
        "each row compares against its own revision"
    );
}

#[test]
fn a_repository_that_converts_line_endings_diffs_only_what_changed() {
    // Apply checkout filters to the stored side before comparing.
    let repo = Repo::new("autocrlf");
    repo.git(&["config", "core.autocrlf", "true"]);
    repo.write("a.txt", "one\ntwo\nthree\nfour\n");
    repo.git(&["add", "-A"]);
    repo.git(&["commit", "-qm", "first"]);
    repo.write("a.txt", "one\r\nTWO\r\nthree\r\nfour\r\n");

    let mut git = repo.open();
    let changes = git
        .get_changed_files(&DiffType::Worktree, &[])
        .expect("listing");
    let file = changes
        .iter()
        .find(|f| f.path().as_str() == "a.txt")
        .expect("the file is listed");

    let before = git
        .get_file_content(file, DiffVersion::Original)
        .expect("reading");
    let after = git
        .get_file_content(file, DiffVersion::Modified)
        .expect("reading");
    let (file_types::FileContent::Text(before), file_types::FileContent::Text(after)) =
        (before, after)
    else {
        panic!("both sides are text");
    };
    // Preserve carriage returns while comparing the two forms.
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
