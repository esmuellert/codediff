mod repository;

use std::fs;
use std::time::Duration;

use repository::{committed, drain_events, subscribe, wait_for_refresh, watched};

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

#[test]
fn nested_gitignore_changes_take_effect_without_restarting() {
    let repo = committed();
    let nested = repo.path().join("nested");
    fs::create_dir(&nested).unwrap();
    fs::write(nested.join(".gitignore"), "").unwrap();
    fs::write(nested.join("file.txt"), "visible\n").unwrap();
    let (_watcher, rx) = subscribe(&repo);

    fs::write(nested.join(".gitignore"), "file.txt\n").unwrap();
    assert!(
        wait_for_refresh(&rx).worktree,
        "the rule change must be reported"
    );
    std::thread::sleep(Duration::from_millis(300));
    drain_events(&rx);

    fs::write(nested.join("file.txt"), "ignored now\n").unwrap();
    assert!(
        rx.recv_timeout(Duration::from_millis(300)).is_err(),
        "a nested rule added after startup must suppress later events"
    );
}

#[test]
fn info_exclude_changes_take_effect_without_restarting() {
    let repo = committed();
    fs::write(repo.path().join("untracked.txt"), "visible\n").unwrap();
    let (_watcher, rx) = subscribe(&repo);

    fs::write(repo.path().join(".git/info/exclude"), "untracked.txt\n").unwrap();
    assert!(
        wait_for_refresh(&rx).worktree,
        "changing info/exclude must refresh the file list"
    );
    std::thread::sleep(Duration::from_millis(300));
    drain_events(&rx);

    fs::write(repo.path().join("untracked.txt"), "ignored now\n").unwrap();
    assert!(
        rx.recv_timeout(Duration::from_millis(300)).is_err(),
        "a new info/exclude rule must suppress later events"
    );
}

#[test]
fn removing_an_ignore_rule_starts_watching_that_directory() {
    let repo = committed();
    let ignored = repo.path().join("target");
    fs::create_dir(&ignored).unwrap();
    fs::write(ignored.join("file.txt"), "ignored\n").unwrap();
    let (_watcher, rx) = subscribe(&repo);

    fs::write(repo.path().join(".gitignore"), "").unwrap();
    assert!(
        wait_for_refresh(&rx).worktree,
        "the rule change must be reported"
    );
    std::thread::sleep(Duration::from_millis(300));
    drain_events(&rx);

    fs::write(ignored.join("file.txt"), "visible now\n").unwrap();
    assert!(
        wait_for_refresh(&rx).worktree,
        "removing the rule must add the directory to the watch scope"
    );
}

#[cfg(target_os = "linux")]
#[test]
fn one_thousand_existing_directories_are_watched() {
    let repo = committed();
    for n in 0..1_000 {
        fs::create_dir(repo.path().join(format!("dir-{n}"))).unwrap();
    }

    let (_watcher, rx) = subscribe(&repo);
    fs::write(repo.path().join("dir-999/file.txt"), "changed").unwrap();
    assert!(
        wait_for_refresh(&rx).worktree,
        "a file in the last registered directory must be seen"
    );
}

#[cfg(target_os = "linux")]
#[test]
fn a_removed_directory_can_be_watched_again_at_the_same_path() {
    let (repo, _watcher, rx) = watched();
    let dir = repo.path().join("replace-me");

    fs::create_dir(&dir).unwrap();
    assert!(wait_for_refresh(&rx).worktree);
    std::thread::sleep(Duration::from_millis(300));
    drain_events(&rx);

    fs::remove_dir(&dir).unwrap();
    assert!(wait_for_refresh(&rx).worktree);
    std::thread::sleep(Duration::from_millis(300));
    drain_events(&rx);

    fs::create_dir(&dir).unwrap();
    assert!(wait_for_refresh(&rx).worktree);
    std::thread::sleep(Duration::from_millis(300));
    drain_events(&rx);

    fs::write(dir.join("file.txt"), "later").unwrap();
    assert!(
        wait_for_refresh(&rx).worktree,
        "recreating a removed path must install a fresh watch"
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
