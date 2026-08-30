mod repository;

use std::fs;
use std::time::Duration;

#[cfg(target_os = "linux")]
use repository::{committed, start};
use repository::{drain_events, wait_for_refresh, watched};

#[test]
fn gitignore_changes_take_effect_without_restarting_the_watcher() {
    let (repo, _watcher, rx) = watched();
    fs::write(repo.path().join(".gitignore"), "target/\nfile.txt\n").unwrap();
    assert!(
        wait_for_refresh(&rx).worktree,
        "the rule change must be reported"
    );
    std::thread::sleep(Duration::from_millis(300));
    drain_events(&rx);

    fs::write(repo.path().join("file.txt"), "ignored now\n").unwrap();
    assert!(
        rx.recv_timeout(Duration::from_millis(300)).is_err(),
        "a rule added after startup must suppress later events"
    );
}

#[cfg(target_os = "linux")]
#[test]
fn one_thousand_existing_directories_are_watched() {
    let repo = committed();
    for n in 0..1_000 {
        fs::create_dir(repo.path().join(format!("dir-{n}"))).unwrap();
    }

    let (_watcher, rx) = start(&repo);
    fs::write(repo.path().join("dir-999/file.txt"), "changed").unwrap();
    assert!(
        wait_for_refresh(&rx).worktree,
        "a file in the last registered directory must be seen"
    );
}

#[cfg(target_os = "linux")]
#[test]
fn directory_created_after_start_keeps_being_watched() {
    let (repo, _watcher, rx) = watched();
    let new_dir = repo.path().join("created-later");
    fs::create_dir(&new_dir).unwrap();
    assert!(
        wait_for_refresh(&rx).worktree,
        "the directory creation must be reported"
    );
    std::thread::sleep(Duration::from_millis(300));
    drain_events(&rx);

    fs::write(new_dir.join("after-the-first-event.txt"), "later").unwrap();
    assert!(
        wait_for_refresh(&rx).worktree,
        "later writes inside the new directory must still be watched"
    );
}
