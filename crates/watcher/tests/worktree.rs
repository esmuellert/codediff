//! Linked-worktree integration tests.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc::Receiver;
use std::time::Duration;

use watcher::{Refresh, Subscription};

fn git_in(dir: &Path, args: &[&str]) -> bool {
    Command::new("git")
        .args(args)
        .current_dir(dir)
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

fn drain(rx: &Receiver<Refresh>) {
    while rx.try_recv().is_ok() {}
}

const TIMEOUT: Duration = Duration::from_secs(3);
const SHORT: Duration = Duration::from_millis(300);

fn wait_for(rx: &Receiver<Refresh>) -> Refresh {
    let first = rx.recv_timeout(TIMEOUT).unwrap_or_default();
    std::thread::sleep(Duration::from_millis(100));
    let mut combined = first;
    while let Ok(r) = rx.try_recv() {
        combined = combined.union(r);
    }
    combined
}

// === Linked worktrees ===

/// A repository with a linked worktree beside it: `main/` holds the only
/// `.git/` directory, `wt/` holds a `.git` *file* naming its own git dir
/// under `main/.git/worktrees/wt`.
struct Linked {
    _dir: tempfile::TempDir,
    wt: PathBuf,
}

impl Linked {
    fn git(&self, args: &[&str]) -> bool {
        git_in(&self.wt, args)
    }
}

fn setup_worktree() -> (Linked, Subscription, Receiver<Refresh>) {
    let dir = tempfile::tempdir().unwrap();
    let main = dir.path().join("main");
    fs::create_dir(&main).unwrap();

    git_in(&main, &["init", "-b", "main"]);
    fs::write(main.join(".gitignore"), "target/\n*.o\n").unwrap();
    fs::write(main.join("file.txt"), "hello").unwrap();
    git_in(&main, &["add", "."]);
    git_in(&main, &["commit", "-m", "init"]);

    let wt = dir.path().join("wt");
    assert!(
        git_in(
            &main,
            &["worktree", "add", "-b", "feature", wt.to_str().unwrap()]
        ),
        "git worktree add failed"
    );
    assert!(wt.join(".git").is_file(), "a worktree's .git is a file");

    let linked = Linked { _dir: dir, wt };
    let (tx, rx) = std::sync::mpsc::channel();
    let emitter = channel::Emitter::new(tx, std::convert::identity);
    let watcher = watcher::subscribe(&linked.wt, emitter).unwrap();
    std::thread::sleep(Duration::from_millis(500));
    while rx.try_recv().is_ok() {}

    (linked, watcher, rx)
}

#[test]
fn worktree_edit_triggers_worktree() {
    let (linked, _w, rx) = setup_worktree();
    fs::write(linked.wt.join("file.txt"), "changed").unwrap();
    let r = wait_for(&rx);
    assert!(r.worktree, "edit in a worktree must set worktree, got {r}");
}

#[test]
fn worktree_add_triggers_index() {
    let (linked, _w, rx) = setup_worktree();
    fs::write(linked.wt.join("file.txt"), "modified").unwrap();
    let _ = wait_for(&rx);
    std::thread::sleep(SHORT);
    drain(&rx);
    linked.git(&["add", "file.txt"]);
    let r = wait_for(&rx);
    assert!(
        r.index,
        "the worktree's own index is under main/.git/worktrees, got {r}"
    );
}

#[test]
fn worktree_commit_triggers_refs() {
    let (linked, _w, rx) = setup_worktree();
    fs::write(linked.wt.join("file.txt"), "v2").unwrap();
    linked.git(&["add", "."]);
    std::thread::sleep(Duration::from_millis(200));
    drain(&rx);
    linked.git(&["commit", "-m", "second"]);
    let r = wait_for(&rx);
    assert!(r.refs, "refs are shared with the main repo, got {r}");
}

#[test]
fn worktree_branch_create_triggers_refs() {
    let (linked, _w, rx) = setup_worktree();
    linked.git(&["branch", "another"]);
    let r = wait_for(&rx);
    assert!(r.refs, "branch create must set refs, got {r}");
}

#[test]
fn worktree_switch_triggers_head() {
    let (linked, _w, rx) = setup_worktree();
    std::thread::sleep(Duration::from_millis(200));
    drain(&rx);
    linked.git(&["switch", "--detach", "HEAD"]);
    let r = wait_for(&rx);
    assert!(r.head, "the worktree's own HEAD moved, got {r}");
}
