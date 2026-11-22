//! Integration tests for commit history and commit walk orders.

use std::path::PathBuf;
use std::process::Command;

use file_types::Oid;
use vcs::{CommitOrder, Repository};

/// A temporary git repository removed on drop.
struct Repo {
    dir: PathBuf,
}

impl Repo {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("codediff-revs-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("creating temp repo dir");
        let repo = Self { dir };
        repo.git(&["init", "-q", "-b", "main"]);
        repo.git(&["config", "user.email", "test@example.com"]);
        repo.git(&["config", "user.name", "Alice, Author"]);
        repo.git(&["config", "core.autocrlf", "false"]);
        repo
    }

    fn git(&self, args: &[&str]) -> String {
        self.git_dated(args, "2026-11-20T12:00:00Z", "2026-11-20T12:00:00Z")
    }

    fn git_dated(&self, args: &[&str], author_date: &str, committer_date: &str) -> String {
        let out = Command::new("git")
            .args(args)
            .current_dir(&self.dir)
            .env("GIT_AUTHOR_DATE", author_date)
            .env("GIT_COMMITTER_DATE", committer_date)
            .output()
            .expect("running git");
        assert!(
            out.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout).trim().to_owned()
    }

    fn write(&self, path: &str, text: &str) {
        std::fs::write(self.dir.join(path), text).expect("writing file");
    }

    fn open(&self) -> Repository {
        Repository::open(&self.dir).expect("opening repository")
    }
}

impl Drop for Repo {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

#[test]
fn unborn_repository_returns_empty_commit_history() {
    let repo = Repo::new("unborn");
    let git = repo.open();
    assert!(
        git.list_commits(CommitOrder::default(), 50)
            .expect("list_commits succeeds on unborn repo")
            .is_empty()
    );

    repo.write("draft.txt", "pending\n");
    assert!(
        git.list_commits(CommitOrder::CommitDate, 50)
            .expect("list_commits succeeds with untracked file")
            .is_empty()
    );
}

#[test]
fn list_commits_returns_topological_merge_graph_and_stripped_labels() {
    let repo = Repo::new("merge-graph");
    repo.write("base.txt", "base\n");
    repo.git(&["add", "-A"]);
    repo.git(&["commit", "-qm", "root commit"]);
    let root_oid = Oid::new(repo.git(&["rev-parse", "--short", "HEAD"]));
    repo.git(&["tag", "v1.0.0"]);

    repo.git(&["checkout", "-qb", "feat/topic"]);
    repo.write("topic.txt", "topic\n");
    repo.git(&["add", "-A"]);
    repo.git(&[
        "commit",
        "-qm",
        "feat: handle | pipes, commas, and \"quotes\"",
    ]);
    let topic_oid = Oid::new(repo.git(&["rev-parse", "--short", "HEAD"]));
    repo.git(&["update-ref", "refs/remotes/origin/feat/topic", "HEAD"]);
    repo.git(&[
        "symbolic-ref",
        "refs/remotes/origin/HEAD",
        "refs/remotes/origin/feat/topic",
    ]);

    repo.git(&["checkout", "-q", "main"]);
    repo.write("main.txt", "main\n");
    repo.git(&["add", "-A"]);
    repo.git(&["commit", "-qm", "main line work"]);
    let main_oid = Oid::new(repo.git(&["rev-parse", "--short", "HEAD"]));

    repo.git(&[
        "merge",
        "--no-ff",
        "-qm",
        "merge topic into main",
        "feat/topic",
    ]);
    let merge_oid = Oid::new(repo.git(&["rev-parse", "--short", "HEAD"]));

    // Stash helper commits must not pollute commit history.
    repo.write("main.txt", "stash me\n");
    repo.git(&["stash", "push", "-q", "-m", "temp"]);

    let git = repo.open();
    let commits = git
        .list_commits(CommitOrder::Topological, 20)
        .expect("listing commits");
    assert_eq!(commits.len(), 4, "stash commits excluded: {commits:?}");

    assert_eq!(commits[0].oid, merge_oid);
    assert_eq!(
        commits[0].parents,
        vec![main_oid.clone(), topic_oid.clone()]
    );
    assert!(commits[0].current);
    assert_eq!(commits[0].branches, vec!["main"]);
    assert!(commits[0].remotes.is_empty());
    assert!(commits[0].tags.is_empty());
    assert_eq!(commits[0].subject, "merge topic into main");
    assert_eq!(commits[0].author, "Alice, Author");
    assert_eq!(commits[0].date, "2026-11-20");

    let topic = commits
        .iter()
        .find(|commit| commit.oid == topic_oid)
        .expect("topic commit present");
    assert!(!topic.current);
    assert_eq!(topic.parents, vec![root_oid.clone()]);
    assert_eq!(
        topic.subject,
        "feat: handle | pipes, commas, and \"quotes\""
    );
    assert_eq!(topic.branches, vec!["feat/topic"]);
    assert_eq!(
        topic.remotes,
        vec!["origin/feat/topic"],
        "origin/HEAD symref is omitted"
    );

    let root_commit = commits.last().expect("root is last in topo order");
    assert_eq!(root_commit.oid, root_oid);
    assert!(root_commit.parents.is_empty());
    assert_eq!(root_commit.tags, vec!["v1.0.0"]);

    // Limit caps the number of returned commits.
    let limited = git
        .list_commits(CommitOrder::Topological, 2)
        .expect("limited commits");
    assert_eq!(limited.len(), 2);
    assert_eq!(limited[0].oid, merge_oid);
    assert!(
        git.list_commits(CommitOrder::Topological, 0)
            .expect("zero limit")
            .is_empty()
    );
}

#[test]
fn detached_checkout_marks_current_commit_without_inventing_branch() {
    let repo = Repo::new("detached");
    repo.write("a.txt", "one\n");
    repo.git(&["add", "-A"]);
    repo.git(&["commit", "-qm", "first"]);
    let first_oid = Oid::new(repo.git(&["rev-parse", "--short", "HEAD"]));

    repo.git(&["checkout", "-q", "--detach"]);
    repo.write("a.txt", "two\n");
    repo.git(&["add", "-A"]);
    repo.git(&["commit", "-qm", "detached commit"]);
    let detached_oid = Oid::new(repo.git(&["rev-parse", "--short", "HEAD"]));

    let commits = repo
        .open()
        .list_commits(CommitOrder::Topological, 10)
        .expect("listing commits");
    assert_eq!(commits.len(), 2);
    assert_eq!(commits[0].oid, detached_oid);
    assert!(commits[0].current);
    assert!(commits[0].branches.is_empty());

    assert_eq!(commits[1].oid, first_oid);
    assert!(!commits[1].current);
    assert_eq!(commits[1].branches, vec!["main"]);
}

#[test]
fn commit_orders_distinguish_topological_commit_date_and_author_date() {
    let repo = Repo::new("orders");

    repo.write("root.txt", "root\n");
    repo.git(&["add", "-A"]);
    repo.git_dated(
        &["commit", "-qm", "root"],
        "2026-01-01T00:00:00Z",
        "2026-01-01T00:00:00Z",
    );

    repo.git(&["checkout", "-qb", "side"]);
    repo.write("side1.txt", "1\n");
    repo.git(&["add", "-A"]);
    repo.git_dated(
        &["commit", "-qm", "side-1"],
        "2026-01-02T00:00:00Z",
        "2026-01-02T00:00:00Z",
    );
    repo.write("side2.txt", "2\n");
    repo.git(&["add", "-A"]);
    repo.git_dated(
        &["commit", "-qm", "side-2"],
        "2026-01-04T00:00:00Z",
        "2026-01-04T00:00:00Z",
    );

    repo.git(&["checkout", "-q", "main"]);
    repo.write("main1.txt", "1\n");
    repo.git(&["add", "-A"]);
    repo.git_dated(
        &["commit", "-qm", "main-1"],
        "2026-01-05T00:00:00Z",
        "2026-01-03T00:00:00Z",
    );
    repo.write("main2.txt", "2\n");
    repo.git(&["add", "-A"]);
    repo.git_dated(
        &["commit", "-qm", "main-2"],
        "2026-01-06T00:00:00Z",
        "2026-01-03T00:01:00Z",
    );

    repo.git_dated(
        &["merge", "--no-ff", "-qm", "merge", "side"],
        "2026-01-07T00:00:00Z",
        "2026-01-07T00:00:00Z",
    );

    let git = repo.open();
    let subjects = |order: CommitOrder| -> Vec<String> {
        git.list_commits(order, 20)
            .expect("list_commits succeeds")
            .into_iter()
            .map(|c| c.subject)
            .collect()
    };

    let topo = subjects(CommitOrder::Topological);
    let by_commit = subjects(CommitOrder::CommitDate);
    let by_author = subjects(CommitOrder::AuthorDate);

    // CommitDate interleaves side-2 -> main-2 -> main-1 -> side-1 by committer time.
    assert_eq!(
        by_commit,
        vec!["merge", "side-2", "main-2", "main-1", "side-1", "root"]
    );
    // AuthorDate orders main-2 -> main-1 (Jan 6, Jan 5) ahead of side-2 -> side-1 (Jan 4, Jan 2),
    // whereas Topological walks the second parent (side-2 -> side-1) first before main-2 -> main-1!
    assert_eq!(
        topo,
        vec!["merge", "side-2", "side-1", "main-2", "main-1", "root"]
    );
    assert_eq!(
        by_author,
        vec!["merge", "main-2", "main-1", "side-2", "side-1", "root"]
    );
    assert_ne!(topo, by_commit);
    assert_ne!(topo, by_author);
    assert_ne!(by_commit, by_author);
}
