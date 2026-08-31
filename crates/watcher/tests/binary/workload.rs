use std::fs;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use crate::process::{WatcherProcess, committed, git};

#[test]
fn mixed_repository_workload_stays_responsive() {
    let repo = committed();
    let mut process = WatcherProcess::start(repo.path());
    assert_eq!(process.next_message()["type"].as_str(), Some("ready"));

    // Keep the byte count unchanged so metadata shortcuts cannot hide the edit.
    fs::write(repo.path().join("file.txt"), "after!\n").unwrap();
    process.wait_for_refresh(|refresh| refresh["worktree"].as_bool() == Some(true));
    process.drain_until_quiet();

    let dynamic_dir = repo.path().join("dynamic");
    fs::create_dir(&dynamic_dir).unwrap();
    process.wait_for_refresh(|refresh| refresh["worktree"].as_bool() == Some(true));
    process.drain_until_quiet();
    fs::write(dynamic_dir.join("file.txt"), "visible\n").unwrap();
    process.wait_for_refresh(|refresh| refresh["worktree"].as_bool() == Some(true));
    process.drain_until_quiet();

    fs::write(repo.path().join(".gitignore"), "target/\ndynamic/\n").unwrap();
    process.wait_for_refresh(|refresh| refresh["worktree"].as_bool() == Some(true));
    process.drain_until_quiet();
    fs::write(dynamic_dir.join("file.txt"), "ignored\n").unwrap();
    process.assert_quiet();
    process.assert_running();

    fs::write(repo.path().join(".gitignore"), "target/\n").unwrap();
    process.wait_for_refresh(|refresh| refresh["worktree"].as_bool() == Some(true));
    process.drain_until_quiet();
    fs::write(dynamic_dir.join("file.txt"), "visible again\n").unwrap();
    process.wait_for_refresh(|refresh| refresh["worktree"].as_bool() == Some(true));
    process.drain_until_quiet();

    let ignored_dir = repo.path().join("target/build");
    fs::create_dir_all(&ignored_dir).unwrap();
    for number in 0..200 {
        fs::write(ignored_dir.join(format!("object-{number}.o")), "object").unwrap();
    }
    process.assert_quiet();
    process.assert_running();

    let edited_file = repo.path().join("file.txt");
    let writer_finished = Arc::new(AtomicBool::new(false));
    let writer_finished_flag = Arc::clone(&writer_finished);
    let writer_thread = std::thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(1);
        let mut number = 0;
        while Instant::now() < deadline {
            fs::write(&edited_file, format!("edit {number}")).unwrap();
            number += 1;
            std::thread::sleep(Duration::from_millis(1));
        }
        writer_finished_flag.store(true, Ordering::Release);
    });
    process.wait_for_refresh(|refresh| refresh["worktree"].as_bool() == Some(true));
    let refresh_arrived_while_writing = !writer_finished.load(Ordering::Acquire);
    writer_thread.join().unwrap();
    assert!(
        refresh_arrived_while_writing,
        "continuous writes must not postpone process output"
    );
    process.drain_until_quiet();

    assert!(git(repo.path(), &["add", "file.txt"]));
    process.wait_for_refresh(|refresh| refresh["index"].as_bool() == Some(true));
    process.drain_until_quiet();
    assert!(git(repo.path(), &["branch", "workload-topic"]));
    process.wait_for_refresh(|refresh| refresh["refs"].as_bool() == Some(true));
    process.drain_until_quiet();
    assert!(git(repo.path(), &["switch", "workload-topic"]));
    process.wait_for_refresh(|refresh| refresh["head"].as_bool() == Some(true));
    process.drain_until_quiet();

    fs::write(repo.path().join("after-workload.txt"), "still alive\n").unwrap();
    process.wait_for_refresh(|refresh| refresh["worktree"].as_bool() == Some(true));
    process.assert_running();
}
