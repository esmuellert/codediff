//! The file list a real repository produces, with the numbers the lines draw.

use std::path::PathBuf;

use diff_service::files::{self as files, Request};
use file_types::Stats;

/// A fixture repository in a temporary directory, removed on drop.
struct Fixture {
    dir: PathBuf,
}

impl Fixture {
    fn new(name: &str) -> Self {
        let dir =
            std::env::temp_dir().join(format!("codediff-pipeline-{name}-{}", std::process::id()));
        fixtures::repo(&dir).expect("building the fixture repository");
        Self { dir }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

#[test]
fn untracked_and_deleted_files_carry_one_sided_counts() {
    let fixture = Fixture::new("one-sided-counts");
    let files = files::get_files(&Request::worktree(&fixture.dir)).expect("listing");

    let untracked = files
        .iter()
        .find(|file| file.path().as_str() == "untracked.txt")
        .expect("untracked file is listed");
    assert_eq!(untracked.get_stats(), Some(Stats::new(1, 0)));

    let deleted = files
        .iter()
        .find(|file| file.path().as_str() == "deleted.txt")
        .expect("deleted file is listed");
    assert_eq!(deleted.get_stats(), Some(Stats::new(0, 1)));
}

#[test]
fn a_file_staged_and_edited_again_carries_a_count_per_comparison() {
    // Each comparison has its own line counts.
    let fixture = Fixture::new("counts");
    let files = files::get_files(&Request::worktree(&fixture.dir)).expect("listing");

    let found: Vec<(&'static str, Option<Stats>)> = files
        .iter()
        .filter(|file| file.path().as_str() == "staged-then-edited.txt")
        .map(|file| (file.revs().heading(), file.get_stats()))
        .collect();
    assert_eq!(
        found,
        vec![
            ("Changes", Some(Stats::new(1, 0))),
            ("Staged Changes", Some(Stats::new(1, 1))),
        ],
        "{found:?}"
    );
}

#[test]
fn commit_to_commit_listing_and_open_produce_aligned_diffs_and_colours() {
    use std::process::Command;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::mpsc;
    use std::time::{Duration, Instant};

    use diff_service::DiffService;
    use diff_service::diff::Settings;
    use file_types::{ChangeType, DiffVersion};
    use vcs::RevisionRange;

    static NEXT_ID: AtomicUsize = AtomicUsize::new(0);
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!(
        "codediff-pipeline-between-{}-{id}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("src")).expect("creating fixture dir");

    struct Cleanup(PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let _cleanup = Cleanup(dir.clone());

    let git = |args: &[&str], date: &str| -> String {
        let out = Command::new("git")
            .args(args)
            .current_dir(&dir)
            .env("GIT_AUTHOR_NAME", "codediff fixtures")
            .env("GIT_AUTHOR_EMAIL", "fixture@codediff.test")
            .env("GIT_AUTHOR_DATE", date)
            .env("GIT_COMMITTER_NAME", "codediff fixtures")
            .env("GIT_COMMITTER_EMAIL", "fixture@codediff.test")
            .env("GIT_COMMITTER_DATE", date)
            .output()
            .expect("running git");
        assert!(
            out.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout).trim().to_owned()
    };

    git(&["init", "-q", "-b", "main"], "2026-01-01T00:00:00Z");
    git(
        &["config", "core.autocrlf", "false"],
        "2026-01-01T00:00:00Z",
    );
    git(&["config", "diff.renames", "true"], "2026-01-01T00:00:00Z");

    std::fs::write(
        dir.join("src/engine.rs"),
        "pub fn compute(limit: usize) -> usize {\n    let base = 1;\n    limit + base\n}\n",
    )
    .unwrap();
    std::fs::write(dir.join("src/obsolete.rs"), "pub fn old() {}\n").unwrap();
    git(&["add", "-A"], "2026-01-01T00:00:00Z");
    git(&["commit", "-qm", "first"], "2026-01-01T00:00:00Z");
    let first = git(&["rev-parse", "HEAD"], "2026-01-01T00:00:00Z");

    std::fs::write(
        dir.join("src/engine.rs"),
        "pub fn compute(limit: usize) -> usize {\n    let step = 2;\n    limit.saturating_add(step)\n}\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("src/added.rs"),
        "pub fn added() -> bool {\n    true\n}\n",
    )
    .unwrap();
    git(&["rm", "-q", "src/obsolete.rs"], "2026-01-02T00:00:00Z");
    git(&["add", "-A"], "2026-01-02T00:00:00Z");
    git(&["commit", "-qm", "second"], "2026-01-02T00:00:00Z");
    let second = git(&["rev-parse", "HEAD"], "2026-01-02T00:00:00Z");

    // Leave dirty working-tree noise to prove Between ignores the working tree.
    std::fs::write(dir.join("dirty.rs"), "pub fn dirty() {}\n").unwrap();

    let (redraw_tx, redraw_rx) = mpsc::channel();
    let service = DiffService::start(
        move || {
            let _ = redraw_tx.send(());
        },
        Settings::default(),
    );

    let wait_idle = || {
        let deadline = Instant::now() + Duration::from_secs(10);
        while service.is_busy() {
            assert!(Instant::now() < deadline, "diff-service timed out");
            let _ = redraw_rx.recv_timeout(Duration::from_millis(20));
        }
    };

    let slot = service.list_files(&dir, RevisionRange::Between(first, second));
    wait_idle();
    let files = slot
        .lock()
        .unwrap()
        .clone()
        .expect("slot filled")
        .expect("listing succeeded");

    let summary: Vec<(&str, ChangeType, Option<Stats>)> = files
        .iter()
        .map(|file| {
            (
                file.path().as_str(),
                file.get_change_type(),
                file.get_stats(),
            )
        })
        .collect();
    assert_eq!(
        summary,
        vec![
            ("src/added.rs", ChangeType::Added, Some(Stats::new(3, 0))),
            (
                "src/engine.rs",
                ChangeType::Modified,
                Some(Stats::new(2, 2))
            ),
            (
                "src/obsolete.rs",
                ChangeType::Deleted,
                Some(Stats::new(0, 1))
            ),
        ]
    );

    let engine = files
        .into_iter()
        .find(|file| file.path().as_str() == "src/engine.rs")
        .expect("engine.rs listed");
    let (diff_slot, colours_slot) = service.open(engine);
    wait_idle();

    let content = diff_slot
        .lock()
        .unwrap()
        .clone()
        .expect("diff filled")
        .expect("diff computed");
    let alignment = content.alignment().expect("two-sided alignment");
    assert_eq!(
        alignment.line(DiffVersion::Original, 2),
        Some("    let base = 1;")
    );
    assert_eq!(
        alignment.line(DiffVersion::Modified, 2),
        Some("    let step = 2;")
    );

    let colours = colours_slot.lock().unwrap();
    assert!(!colours[DiffVersion::Original as usize].is_empty());
    assert!(!colours[DiffVersion::Modified as usize].is_empty());
    assert!(
        colours[DiffVersion::Original as usize]
            .iter()
            .any(|spans| !spans.is_empty()),
        "original commit lines received syntax spans"
    );
    assert!(
        colours[DiffVersion::Modified as usize]
            .iter()
            .any(|spans| !spans.is_empty()),
        "modified commit lines received syntax spans"
    );
}
