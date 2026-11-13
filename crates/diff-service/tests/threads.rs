//! The background threads fill in each file's own result.

use std::path::Path;
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use diff_service::DiffService;
use diff_service::diff::{self, Diff, DiffContent, SingleFile};
use file_types::{DiffVersion, File, Oid, RepoPath, Revs};

fn file(path: &str) -> File {
    File::unchanged_path(
        RepoPath::new(path, Path::new("/repo")),
        Revs::worktree_against(Oid::new("base")),
    )
}

fn content(path: &str, after: &str) -> DiffContent {
    let before = ["fn main() { let value = 1; }"];
    let after = [after];
    let changed = diff::compute(&before, &after, diff::Settings::default()).unwrap();
    DiffContent::Diff(Diff {
        file: file(path),
        alignment: diff::align(changed, &before, &after).unwrap(),
    })
}

/// A pipeline that reads files by name from `contents`.
fn pipeline(contents: Vec<DiffContent>) -> (DiffService, Receiver<()>) {
    let (redraw, redraws) = mpsc::channel();
    let pipeline = DiffService::new(
        move || {
            let _ = redraw.send(());
        },
        |_| Ok(Vec::new()),
        move |file| {
            contents
                .iter()
                .find(|content| content.file() == file)
                .cloned()
                .ok_or_else(|| format!("cannot read {}", file.path()))
        },
    );
    (pipeline, redraws)
}

fn wait_until_idle(pipeline: &DiffService, redraws: &Receiver<()>) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while pipeline.is_busy() {
        assert!(Instant::now() < deadline, "the pipeline never finished");
        let _ = redraws.recv_timeout(Duration::from_millis(20));
    }
}

#[test]
fn every_open_file_gets_its_own_diff_and_colours() {
    let (pipeline, redraws) = pipeline(vec![
        content("a.rs", "fn main() { let value = 2; }"),
        content("b.rs", "fn main() { let other = 3; }"),
    ]);
    let a = pipeline.open(file("a.rs"));
    let b = pipeline.open(file("b.rs"));
    wait_until_idle(&pipeline, &redraws);

    for ((diff, colours), expected) in [(&a, "let value = 2"), (&b, "let other = 3")] {
        let content = diff.lock().unwrap().clone().unwrap().unwrap();
        let line = content
            .alignment()
            .unwrap()
            .line(DiffVersion::Modified, 1)
            .unwrap();
        assert!(line.contains(expected), "{line}");
        let colours = colours.lock().unwrap();
        assert!(!colours[DiffVersion::Original as usize][0].is_empty());
        assert!(!colours[DiffVersion::Modified as usize][0].is_empty());
    }
}

#[test]
fn a_file_that_cannot_be_read_reports_why_and_the_next_one_still_loads() {
    let (pipeline, redraws) = pipeline(vec![content("good.rs", "fn good() {}")]);
    let bad = pipeline.open(file("bad.rs"));
    let good = pipeline.open(file("good.rs"));
    wait_until_idle(&pipeline, &redraws);

    assert_eq!(
        bad.0.lock().unwrap().clone().unwrap().err().as_deref(),
        Some("cannot read bad.rs")
    );
    assert!(matches!(*good.0.lock().unwrap(), Some(Ok(_))));
}

#[test]
fn a_file_nobody_holds_any_more_is_not_read() {
    let (release, wait) = mpsc::channel::<()>();
    let wait = Mutex::new(wait);
    let (started, reading) = mpsc::channel::<()>();
    let reads = Arc::new(Mutex::new(Vec::new()));
    let read_paths = Arc::clone(&reads);
    let (redraw, redraws) = mpsc::channel();
    let pipeline = DiffService::new(
        move || {
            let _ = redraw.send(());
        },
        |_| Ok(Vec::new()),
        move |file| {
            read_paths
                .lock()
                .unwrap()
                .push(file.path().as_str().to_owned());
            if read_paths.lock().unwrap().len() == 1 {
                started.send(()).unwrap();
                wait.lock().unwrap().recv().unwrap();
            }
            Ok(DiffContent::SingleFile(SingleFile {
                file: file.clone(),
                lines: Arc::new(vec!["text".to_owned()]),
            }))
        },
    );

    // A is being read while B is queued; the user then moves back to A.
    let first_a = pipeline.open(file("a.txt"));
    reading.recv_timeout(Duration::from_secs(10)).unwrap();
    let b = pipeline.open(file("b.txt"));
    drop(b);
    drop(first_a);
    let second_a = pipeline.open(file("a.txt"));
    release.send(()).unwrap();
    wait_until_idle(&pipeline, &redraws);

    assert_eq!(*reads.lock().unwrap(), ["a.txt", "a.txt"]);
    assert!(matches!(*second_a.0.lock().unwrap(), Some(Ok(_))));
}

#[test]
fn a_listed_repository_reports_its_files_or_why_it_could_not() {
    let (redraw, redraws) = mpsc::channel();
    let mut answers = vec![Err("not a repository".to_owned()), Ok(vec![file("a.rs")])];
    let pipeline = DiffService::new(
        move || {
            let _ = redraw.send(());
        },
        move |_| answers.pop().unwrap(),
        |_| unreachable!("nothing is opened"),
    );

    let listed = pipeline.list_files(Path::new("/repo"));
    wait_until_idle(&pipeline, &redraws);
    assert_eq!(*listed.lock().unwrap(), Some(Ok(vec![file("a.rs")])));

    let failed = pipeline.list_files(Path::new("/elsewhere"));
    wait_until_idle(&pipeline, &redraws);
    assert_eq!(
        *failed.lock().unwrap(),
        Some(Err("not a repository".to_owned()))
    );
}
