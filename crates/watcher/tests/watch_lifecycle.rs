mod repository;

use std::fs;

use repository::{drain_events, wait_for_refresh, watched};

#[test]
fn a_non_repository_is_rejected_instead_of_starting_partially() {
    let dir = tempfile::tempdir().unwrap();
    let (tx, _rx) = std::sync::mpsc::channel();
    let emitter = channel::Emitter::new(tx, std::convert::identity);

    assert!(
        watcher::start(dir.path(), emitter).is_err(),
        "missing git roots must make startup fail"
    );
}

#[test]
fn one_hundred_start_stop_cycles_leave_a_usable_watcher() {
    let (repo, first, first_rx) = watched();
    drop(first);
    drop(first_rx);

    for _ in 0..100 {
        let (tx, rx) = std::sync::mpsc::channel();
        let emitter = channel::Emitter::new(tx, std::convert::identity);
        let watcher = watcher::start(repo.path(), emitter).unwrap();
        drop(watcher);
        drop(rx);
    }

    let (tx, rx) = std::sync::mpsc::channel();
    let emitter = channel::Emitter::new(tx, std::convert::identity);
    let final_watcher = watcher::start(repo.path(), emitter).unwrap();
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
