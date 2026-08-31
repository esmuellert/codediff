mod repository;

use std::fs;
use std::path::Path;

use repository::{drain_events, wait_for_refresh, watched};

fn subscribe(path: &Path) -> anyhow::Result<watcher::Subscription> {
    let (tx, _rx) = std::sync::mpsc::channel();
    watcher::subscribe(path, channel::Emitter::new(tx, std::convert::identity))
}

#[test]
fn a_non_repository_is_rejected_instead_of_starting_partially() {
    let dir = tempfile::tempdir().unwrap();
    assert!(
        subscribe(dir.path()).is_err(),
        "missing git roots must make startup fail"
    );
}

#[test]
fn an_empty_dot_git_directory_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    fs::create_dir(dir.path().join(".git")).unwrap();
    assert!(subscribe(dir.path()).is_err());
}

#[test]
fn a_malformed_dot_git_file_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join(".git"), "not a gitdir pointer\n").unwrap();
    assert!(subscribe(dir.path()).is_err());
}

#[test]
fn a_missing_worktree_git_dir_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join(".git"), "gitdir: missing\n").unwrap();
    assert!(subscribe(dir.path()).is_err());
}

#[test]
fn a_missing_common_git_dir_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let worktree = dir.path().join("worktree");
    let worktree_git_dir = dir.path().join("main/.git/worktrees/worktree");
    fs::create_dir_all(&worktree).unwrap();
    fs::create_dir_all(&worktree_git_dir).unwrap();
    fs::write(worktree_git_dir.join("HEAD"), "ref: refs/heads/main\n").unwrap();
    fs::write(
        worktree.join(".git"),
        format!("gitdir: {}\n", worktree_git_dir.display()),
    )
    .unwrap();
    fs::write(worktree_git_dir.join("commondir"), "../missing\n").unwrap();

    assert!(subscribe(&worktree).is_err());
}

#[test]
fn one_hundred_start_stop_cycles_leave_a_usable_watcher() {
    let (repo, first, first_rx) = watched();
    drop(first);
    drop(first_rx);

    for _ in 0..100 {
        let (tx, rx) = std::sync::mpsc::channel();
        let emitter = channel::Emitter::new(tx, std::convert::identity);
        let watcher = watcher::subscribe(repo.path(), emitter).unwrap();
        drop(watcher);
        drop(rx);
    }

    let (tx, rx) = std::sync::mpsc::channel();
    let emitter = channel::Emitter::new(tx, std::convert::identity);
    let final_watcher = watcher::subscribe(repo.path(), emitter).unwrap();
    std::thread::sleep(std::time::Duration::from_millis(300));
    drain_events(&rx);

    fs::write(repo.path().join("file.txt"), "after cycles\n").unwrap();
    let refresh = wait_for_refresh(&rx);
    assert!(
        refresh.worktree,
        "the final watcher must still deliver events"
    );
    drop(final_watcher);
}
