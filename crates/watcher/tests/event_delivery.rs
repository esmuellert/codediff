mod repository;

use std::fs;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use repository::{drain_events, wait_for_refresh, watched};

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
fn two_thousand_rapid_overwrites_coalesce() {
    let (repo, _watcher, rx) = watched();
    drain_events(&rx);

    for n in 0..2_000 {
        fs::write(repo.path().join("file.txt"), format!("edit {n}")).unwrap();
    }
    std::thread::sleep(Duration::from_secs(1));

    let refreshes: Vec<_> = rx.try_iter().collect();
    let refresh_count = refreshes.len();
    assert!(refresh_count >= 1, "the writes must produce a refresh");
    assert!(
        refresh_count <= 10,
        "2,000 rapid writes should coalesce to at most 10 refreshes, got {refresh_count}"
    );
    assert!(
        refreshes
            .iter()
            .all(|refresh| refresh.worktree && !refresh.index && !refresh.head && !refresh.refs),
        "ordinary worktree writes must not require overflow recovery"
    );
}

#[test]
fn sustained_edits_emit_before_the_writer_stops() {
    let (repo, _watcher, rx) = watched();
    let path = repo.path().join("file.txt");
    let done = Arc::new(AtomicBool::new(false));
    let writer_done = Arc::clone(&done);

    let writer = std::thread::spawn(move || {
        let deadline = std::time::Instant::now() + Duration::from_secs(2);
        let mut n = 0;
        while std::time::Instant::now() < deadline {
            fs::write(&path, format!("edit {n}")).unwrap();
            n += 1;
            std::thread::sleep(Duration::from_millis(1));
        }
        writer_done.store(true, Ordering::Release);
    });

    let first = rx.recv_timeout(Duration::from_millis(500));
    let was_still_writing = !done.load(Ordering::Acquire);
    writer.join().unwrap();
    assert!(
        was_still_writing,
        "the writer must outlive the observation window"
    );
    assert!(
        first.is_ok(),
        "continuous events must not postpone refresh until the writer stops"
    );
}
