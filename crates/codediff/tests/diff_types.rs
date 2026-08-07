//! Every way of asking what to compare, against a real repository.
//!
//! The point of the shape: a way to compare is one arm of `vcs::DiffType` and
//! one arm of `git::plan`, and nothing between them learns about it. These tests are what says that is true rather than
//! intended — each runs the whole thing, from the command line to the rows.

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
    // A *tracked* file edited and not staged. Without one, `--cached` and no
    // `--cached` list the same files and nothing tells them apart — which is
    // how the first version of this passed with `--cached` removed.
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
    // The case the Neovim plugin could not express: neither side is the index
    // or the working tree, so it belongs to no fixed category. If our groups
    // were still a fixed pair of lists this is where it would show.
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
    // Not `dirty.txt`: `git diff` does not report an untracked file, whatever
    // it is compared against, and the plugin's `:CodeDiff <rev>` runs the same
    // command and shows the same list.
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

    // Against `side` directly, both branches' files differ. Against where they
    // parted, only what `side` added does — which is what `...` is for.
    let out = repo.list(&["--rev", "HEAD...side"]);
    assert!(out.contains("only-on-side.txt"), "{out}");
    assert!(
        !out.contains("only-on-main.txt"),
        "the merge base was not used:\n{out}"
    );
}

#[test]
fn a_group_says_which_two_versions_it_compares() {
    // The whole model in one assertion: a group is a revision pair, so every
    // file in it carries that pair and the heading is only a name for it.
    let repo = built("revs");
    let out = repo.list(&[]);
    assert!(out.contains("staged -> working tree"), "{out}");
    assert!(out.contains(" -> staged"), "{out}");
}
