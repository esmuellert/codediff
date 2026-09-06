//! Exercises every comparison mode through the binary.

use std::path::PathBuf;
use std::process::Command;

/// A repository built by hand, removed on drop.
struct Repo {
    dir: PathBuf,
}

impl Repo {
    fn new(name: &str) -> Self {
        let dir =
            std::env::temp_dir().join(format!("codediff-types-{name}-{}", std::process::id()));
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

    /// What `codediff debug list` prints for these arguments.
    fn list(&self, args: &[&str]) -> String {
        let out = Command::new(env!("CARGO_BIN_EXE_codediff"))
            .arg("debug")
            .arg("list")
            .args(args)
            .current_dir(&self.dir)
            .output()
            .expect("running codediff");
        assert!(
            out.status.success(),
            "codediff debug list {args:?} failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8(out.stdout).expect("output is utf-8")
    }
}

impl Drop for Repo {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// A repository with a commit, a staged change and an unstaged one.
fn built(name: &str) -> Repo {
    let repo = Repo::new(name);
    repo.write("kept.txt", "one\n");
    repo.write("changed.txt", "before\n");
    repo.git(&["add", "-A"]);
    repo.git(&["commit", "-qm", "first"]);

    repo.write("changed.txt", "after\n");
    repo.write("added.txt", "new\n");
    repo.git(&["add", "-A"]);
    repo.git(&["commit", "-qm", "second"]);

    repo.write("staged.txt", "staged\n");
    repo.git(&["add", "staged.txt"]);
    repo.write("dirty.txt", "not staged\n");
    // Keep a tracked unstaged change to distinguish the worktree comparisons.
    repo.write("kept.txt", "edited but not staged\n");
    repo
}

#[test]
fn the_working_tree_is_two_comparisons() {
    let repo = built("worktree");
    let out = repo.list(&[]);
    assert!(out.contains("Changes"), "{out}");
    assert!(out.contains("Staged Changes"), "{out}");
    assert!(out.contains("dirty.txt"), "{out}");
    assert!(out.contains("staged.txt"), "{out}");
}

#[test]
fn two_revisions_are_one_comparison() {
    // Revisions not involving the index or worktree form one group.
    let repo = built("between");
    let out = repo.list(&["--rev", "HEAD~1", "HEAD"]);
    assert_eq!(out.matches("group ").count(), 1, "one group:\n{out}");
    assert!(out.contains("changed.txt"), "{out}");
    assert!(out.contains("added.txt"), "{out}");
    assert!(!out.contains("dirty.txt"), "nothing uncommitted:\n{out}");
}

#[test]
fn one_revision_is_against_the_file_on_disk() {
    let repo = built("against");
    let out = repo.list(&["--rev", "HEAD"]);
    assert_eq!(out.matches("group ").count(), 1, "one group:\n{out}");
    assert!(out.contains("staged.txt"), "{out}");
    // Git diff does not list untracked files.
    assert!(!out.contains("dirty.txt"), "{out}");
}

#[test]
fn staged_is_the_index_against_a_revision() {
    let repo = built("staged");
    let out = repo.list(&["--staged"]);
    assert_eq!(out.matches("group ").count(), 1, "one group:\n{out}");
    assert!(out.contains("staged.txt"), "{out}");
    assert!(!out.contains("dirty.txt"), "nothing unstaged:\n{out}");
    assert!(!out.contains("kept.txt"), "nothing unstaged:\n{out}");
}

#[test]
fn three_dots_compare_against_where_the_branches_parted() {
    let repo = Repo::new("mergebase");
    repo.write("base.txt", "base\n");
    repo.git(&["add", "-A"]);
    repo.git(&["commit", "-qm", "base"]);
    repo.git(&["checkout", "-qb", "side"]);
    repo.write("only-on-side.txt", "side\n");
    repo.git(&["add", "-A"]);
    repo.git(&["commit", "-qm", "side"]);
    repo.git(&["checkout", "-q", "-"]);
    repo.write("only-on-main.txt", "main\n");
    repo.git(&["add", "-A"]);
    repo.git(&["commit", "-qm", "main"]);

    // `HEAD...side` compares the target with the merge base.
    let out = repo.list(&["--rev", "HEAD...side"]);
    assert!(out.contains("only-on-side.txt"), "{out}");
    assert!(
        !out.contains("only-on-main.txt"),
        "the merge base was not used:\n{out}"
    );
}

#[test]
fn a_group_says_which_two_versions_it_compares() {
    // Each group carries the revision pair it compares.
    let repo = built("revs");
    let out = repo.list(&[]);
    assert!(out.contains("staged -> working tree"), "{out}");
    assert!(out.contains(" -> staged"), "{out}");
}
