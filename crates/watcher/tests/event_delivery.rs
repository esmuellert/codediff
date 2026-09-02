mod repository;

use std::fs;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use repository::{EVENT_TIMEOUT, drain_events, wait_for_refresh, watched};

#[test]
fn same_size_overwrite_is_seen() {
    let (repo, _watcher, rx) = watched();
    fs::write(repo.path().join("file.txt"), "bbbb\n").unwrap();
    assert!(
        wait_for_refresh(&rx).worktree,
        "same-size writes must be visible"
    );
}

#[test]
fn two_thousand_overwrites_remain_worktree_only() {
    let (repo, _watcher, rx) = watched();
    drain_events(&rx);

    for n in 0..2_000 {
        fs::write(repo.path().join("file.txt"), format!("edit {n}")).unwrap();
    }
    std::thread::sleep(Duration::from_secs(1));

    let refreshes: Vec<_> = rx.try_iter().collect();
    assert!(!refreshes.is_empty(), "the writes must produce a refresh");
    assert!(
        refreshes
            .iter()
            .all(|refresh| refresh.worktree && !refresh.index && !refresh.head && !refresh.refs),
        "ordinary worktree writes must not require overflow recovery"
    );
}

#[test]
fn continuous_edits_emit_without_waiting_for_quiet() {
    let (repo, _watcher, rx) = watched();
    let path = repo.path().join("file.txt");
    let stop = Arc::new(AtomicBool::new(false));
    let writer_stop = Arc::clone(&stop);

    let writer = std::thread::spawn(move || {
        let mut n = 0;
        while !writer_stop.load(Ordering::Acquire) {
            fs::write(&path, format!("edit {n}")).unwrap();
            n += 1;
            std::thread::sleep(Duration::from_millis(1));
        }
    });

    let first = rx.recv_timeout(EVENT_TIMEOUT);
    stop.store(true, Ordering::Release);
    writer.join().unwrap();
    assert!(
        first.is_ok(),
        "continuous events must not postpone refresh until the writer stops"
    );
}
