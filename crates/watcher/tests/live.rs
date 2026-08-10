//! Integration tests: real temp repos, real notify, real git commands.

use std::fs;
use std::path::Path;
use std::process::Command;
use std::sync::mpsc::Receiver;
use std::time::Duration;

use watcher::{Refresh, Watcher};

/// How long to wait for a refresh event.
const TIMEOUT: Duration = Duration::from_secs(3);

/// Short wait to confirm nothing arrives.
const SHORT: Duration = Duration::from_millis(300);

/// Long enough for a self-sustaining loop (~15 Hz) to show itself.
const QUIET: Duration = Duration::from_secs(1);

struct Repo {
    dir: tempfile::TempDir,
}

impl Repo {
    fn path(&self) -> &Path {
        self.dir.path()
    }

    fn git(&self, args: &[&str]) -> bool {
        Command::new("git")
            .args(args)
            .current_dir(self.path())
            .env("GIT_AUTHOR_NAME", "Test")
            .env("GIT_AUTHOR_EMAIL", "t@t")
            .env("GIT_COMMITTER_NAME", "Test")
            .env("GIT_COMMITTER_EMAIL", "t@t")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .unwrap()
            .success()
    }
}

fn setup() -> (Repo, Watcher, Receiver<Refresh>) {
    let dir = tempfile::tempdir().unwrap();
    let repo = Repo { dir };

    // git init + initial commit + .gitignore
    repo.git(&["init", "-b", "main"]);
    fs::write(
        repo.path().join(".gitignore"),
        "target/\n*.o\nnode_modules/\n",
    )
    .unwrap();
    fs::write(repo.path().join("file.txt"), "hello").unwrap();
    repo.git(&["add", "."]);
    repo.git(&["commit", "-m", "init"]);

    let (watcher, rx) = watcher::start(repo.path()).unwrap();
    std::thread::sleep(Duration::from_millis(500));
    // Drain any events from the setup operations that arrived after start.
    while rx.try_recv().is_ok() {}

    (repo, watcher, rx)
}

fn drain(rx: &Receiver<Refresh>) -> Refresh {
    let mut combined = Refresh::default();
    while let Ok(r) = rx.try_recv() {
        combined = combined.union(r);
    }
    combined
}

fn wait_for(rx: &Receiver<Refresh>) -> Refresh {
    let first = rx.recv_timeout(TIMEOUT).unwrap_or_default();
    // Drain any additional coalesced events.
    std::thread::sleep(Duration::from_millis(100));
    let mut combined = first;
    while let Ok(r) = rx.try_recv() {
        combined = combined.union(r);
    }
    combined
}

/// Asserts the exact bits and that no further event arrives.
fn assert_only(rx: &Receiver<Refresh>, expected: Refresh) {
    let got = wait_for(rx);
    assert_eq!(got, expected, "wrong bits: expected {expected}, got {got}");
    // No duplicate.
    assert!(
        rx.recv_timeout(SHORT).is_err(),
        "unexpected second event after {expected}"
    );
}

// === Worktree changes ===

#[test]
fn edit_tracked_file() {
    let (repo, _w, rx) = setup();
    fs::write(repo.path().join("file.txt"), "changed").unwrap();
    assert_only(
        &rx,
        Refresh {
            worktree: true,
            ..Default::default()
        },
    );
}

#[test]
fn create_new_file() {
    let (repo, _w, rx) = setup();
    fs::write(repo.path().join("new.txt"), "new").unwrap();
    assert_only(
        &rx,
        Refresh {
            worktree: true,
            ..Default::default()
        },
    );
}

#[test]
fn delete_tracked_file() {
    let (repo, _w, rx) = setup();
    fs::remove_file(repo.path().join("file.txt")).unwrap();
    assert_only(
        &rx,
        Refresh {
            worktree: true,
            ..Default::default()
        },
    );
}

#[test]
fn rename_tracked_file() {
    let (repo, _w, rx) = setup();
    fs::rename(
        repo.path().join("file.txt"),
        repo.path().join("renamed.txt"),
    )
    .unwrap();
    let r = wait_for(&rx);
    assert!(r.worktree, "expected worktree, got {r}");
}

#[test]
fn create_directory_with_file() {
    let (repo, _w, rx) = setup();
    fs::create_dir_all(repo.path().join("src")).unwrap();
    fs::write(repo.path().join("src/lib.rs"), "// lib").unwrap();
    assert_only(
        &rx,
        Refresh {
            worktree: true,
            ..Default::default()
        },
    );
}

#[test]
fn edit_untracked_file() {
    let (repo, _w, rx) = setup();
    fs::write(repo.path().join("untracked.txt"), "data").unwrap();
    assert_only(
        &rx,
        Refresh {
            worktree: true,
            ..Default::default()
        },
    );
}

// === Should trigger nothing ===

#[test]
fn write_to_gitignored_file() {
    let (repo, _w, rx) = setup();
    fs::create_dir_all(repo.path().join("target/debug")).unwrap();
    std::thread::sleep(Duration::from_millis(100));
    drain(&rx); // clear any dir creation events
    fs::write(repo.path().join("target/debug/binary"), "binary").unwrap();
    assert!(
        rx.recv_timeout(SHORT).is_err(),
        "should not trigger for ignored file"
    );
}

#[test]
fn write_to_nested_ignored_dir() {
    let (repo, _w, rx) = setup();
    fs::create_dir_all(repo.path().join("target/release/deps")).unwrap();
    std::thread::sleep(Duration::from_millis(100));
    drain(&rx);
    fs::write(repo.path().join("target/release/deps/foo.o"), "obj").unwrap();
    assert!(
        rx.recv_timeout(SHORT).is_err(),
        "should not trigger for nested ignored"
    );
}

#[test]
fn lock_file_is_silent() {
    let (repo, _w, rx) = setup();
    fs::write(repo.path().join(".git/index.lock"), "lock").unwrap();
    std::thread::sleep(Duration::from_millis(50));
    fs::remove_file(repo.path().join(".git/index.lock")).unwrap();
    assert!(
        rx.recv_timeout(SHORT).is_err(),
        "should not trigger for .lock file"
    );
}

#[test]
fn git_objects_is_silent() {
    let (repo, _w, rx) = setup();
    fs::create_dir_all(repo.path().join(".git/objects/ab")).unwrap();
    fs::write(repo.path().join(".git/objects/ab/cdef1234"), "obj").unwrap();
    assert!(
        rx.recv_timeout(SHORT).is_err(),
        "should not trigger for git objects"
    );
}

#[test]
fn git_hooks_is_silent() {
    let (repo, _w, rx) = setup();
    fs::write(repo.path().join(".git/hooks/pre-commit"), "#!/bin/sh").unwrap();
    assert!(
        rx.recv_timeout(SHORT).is_err(),
        "should not trigger for git hooks"
    );
}

// === Git index ===

#[test]
fn git_add_triggers_index() {
    let (repo, _w, rx) = setup();
    fs::write(repo.path().join("file.txt"), "modified").unwrap();
    let _ = wait_for(&rx); // consume worktree event
    std::thread::sleep(SHORT);
    drain(&rx);
    repo.git(&["add", "file.txt"]);
    let r = wait_for(&rx);
    assert!(r.index, "git add must set index, got {r}");
}

#[test]
fn git_reset_triggers_index() {
    let (repo, _w, rx) = setup();
    fs::write(repo.path().join("file.txt"), "mod").unwrap();
    repo.git(&["add", "file.txt"]);
    std::thread::sleep(Duration::from_millis(200));
    drain(&rx);
    repo.git(&["reset", "file.txt"]);
    let r = wait_for(&rx);
    assert!(r.index, "git reset must set index, got {r}");
}

// === Git HEAD ===

#[test]
fn git_checkout_branch_triggers_head() {
    let (repo, _w, rx) = setup();
    repo.git(&["branch", "feature"]);
    std::thread::sleep(Duration::from_millis(200));
    drain(&rx);
    repo.git(&["checkout", "feature"]);
    let r = wait_for(&rx);
    assert!(r.head, "checkout must set head, got {r}");
}

#[test]
fn git_switch_branch_triggers_head() {
    let (repo, _w, rx) = setup();
    repo.git(&["branch", "other"]);
    std::thread::sleep(Duration::from_millis(200));
    drain(&rx);
    repo.git(&["switch", "other"]);
    let r = wait_for(&rx);
    assert!(r.head, "switch must set head, got {r}");
}

// === Git refs ===

#[test]
fn git_commit_triggers_refs() {
    let (repo, _w, rx) = setup();
    fs::write(repo.path().join("file.txt"), "v2").unwrap();
    repo.git(&["add", "."]);
    std::thread::sleep(Duration::from_millis(200));
    drain(&rx);
    repo.git(&["commit", "-m", "second"]);
    let r = wait_for(&rx);
    assert!(r.refs, "commit must set refs, got {r}");
}

#[test]
fn git_branch_create_triggers_refs() {
    let (repo, _w, rx) = setup();
    repo.git(&["branch", "new-branch"]);
    let r = wait_for(&rx);
    assert!(r.refs, "branch create must set refs, got {r}");
    assert!(
        !r.worktree,
        "branch create should not trigger worktree, got {r}"
    );
}

#[test]
fn git_tag_triggers_refs() {
    let (repo, _w, rx) = setup();
    repo.git(&["tag", "v1.0"]);
    let r = wait_for(&rx);
    assert!(r.refs, "tag must set refs, got {r}");
    assert!(!r.worktree, "tag should not trigger worktree, got {r}");
}

// === Combined ===

#[test]
fn git_commit_all_triggers_index_and_refs() {
    let (repo, _w, rx) = setup();
    fs::write(repo.path().join("file.txt"), "v3").unwrap();
    std::thread::sleep(Duration::from_millis(200));
    drain(&rx);
    repo.git(&["commit", "-am", "all"]);
    let r = wait_for(&rx);
    assert!(r.index || r.refs, "expected index|refs, got {r}");
}

// === Stress / coalescing ===

#[test]
fn rapid_edits_coalesce() {
    let (repo, _w, rx) = setup();
    for i in 0..100 {
        fs::write(repo.path().join("file.txt"), format!("edit {i}")).unwrap();
    }
    // Wait for all debounce windows to close.
    std::thread::sleep(Duration::from_secs(1));
    let mut count = 0;
    while rx.try_recv().is_ok() {
        count += 1;
    }
    assert!(
        count <= 5,
        "100 rapid edits should coalesce to ≤5 refreshes, got {count}"
    );
    assert!(count >= 1, "at least one refresh expected");
}

#[test]
fn build_in_ignored_dir_triggers_nothing() {
    let (repo, _w, rx) = setup();
    fs::create_dir_all(repo.path().join("target/debug")).unwrap();
    std::thread::sleep(Duration::from_millis(200));
    drain(&rx);
    for i in 0..50 {
        fs::write(repo.path().join(format!("target/debug/file{i}.o")), "obj").unwrap();
    }
    std::thread::sleep(Duration::from_millis(500));
    let mut count = 0;
    while rx.try_recv().is_ok() {
        count += 1;
    }
    assert_eq!(count, 0, "ignored dir writes should trigger 0, got {count}");
}

// === Edge cases ===

#[test]
fn gitignore_change_triggers_worktree() {
    let (repo, _w, rx) = setup();
    fs::write(repo.path().join(".gitignore"), "target/\n*.o\nbuild/\n").unwrap();
    let r = wait_for(&rx);
    assert!(
        r.worktree,
        "expected worktree for .gitignore change, got {r}"
    );
}

#[test]
fn heavy_non_ignored_writes_stay_responsive() {
    let (repo, _w, rx) = setup();
    // Create a non-ignored directory with 500 files rapidly.
    // All events pass the filter — stress the classify path.
    fs::create_dir_all(repo.path().join("src")).unwrap();
    let start = std::time::Instant::now();
    for i in 0..500 {
        fs::write(
            repo.path().join(format!("src/file{i}.rs")),
            format!("// {i}"),
        )
        .unwrap();
    }
    let write_time = start.elapsed();

    // Wait for all debounce windows to close.
    std::thread::sleep(Duration::from_secs(2));
    let mut count = 0;
    while rx.try_recv().is_ok() {
        count += 1;
    }

    // The 500 writes should coalesce into a small number of refreshes.
    assert!(
        count <= 10,
        "500 non-ignored writes should coalesce to ≤10 refreshes, got {count}"
    );
    assert!(count >= 1, "at least one refresh expected");
    // The writes themselves shouldn't be blocked by the watcher.
    assert!(
        write_time < Duration::from_secs(5),
        "writing 500 files took {write_time:?} — watcher may be blocking"
    );
}

// === Read-only access must never wake the watcher ===

/// Every read codediff performs while it refreshes.
fn read_like_a_refresh(repo: &Repo) {
    repo.git(&["--no-optional-locks", "status", "--porcelain=v2", "-z"]);
    let _ = fs::read(repo.path().join(".git/HEAD"));
    let _ = fs::read(repo.path().join(".git/index"));
    let _ = fs::read(repo.path().join("file.txt"));
    let _ = fs::read(repo.path().join(".gitignore"));
}

#[test]
fn reading_files_triggers_nothing() {
    let (repo, _w, rx) = setup();
    read_like_a_refresh(&repo);
    assert!(
        rx.recv_timeout(QUIET).is_err(),
        "reads must not trigger a refresh"
    );
}

#[test]
fn refresh_does_not_feed_itself() {
    let (repo, _w, rx) = setup();

    // One real edit starts the cycle.
    fs::write(repo.path().join("file.txt"), "changed").unwrap();

    // Answer every refresh with the reads a refresh performs. If a read
    // counts as a change, this never settles.
    let mut refreshes = 0;
    let deadline = std::time::Instant::now() + Duration::from_secs(10);
    while std::time::Instant::now() < deadline {
        match rx.recv_timeout(QUIET) {
            Ok(_) => {
                refreshes += 1;
                assert!(
                    refreshes <= 20,
                    "feedback loop: {refreshes} refreshes and still going"
                );
                read_like_a_refresh(&repo);
            }
            Err(_) => break,
        }
    }

    assert!(refreshes >= 1, "the edit itself must produce a refresh");
    assert!(
        refreshes <= 3,
        "should settle in a few refreshes, got {refreshes}"
    );
}
