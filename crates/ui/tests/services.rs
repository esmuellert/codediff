//! Unit and integration tests for `VcsService`, `DiffService`, and `SyntaxService`.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use diff::{Diff, DiffContent, Settings, SingleFile};
use file_types::{ChangeType, CommitHash, DiffVersion, File, RepoPath, Rev, Revs, Stats};
use ui::services::{DiffService, SyntaxService, VcsService};
use vcs::RevisionRange;

fn file(path: &str) -> File {
    File::unchanged_path(
        RepoPath::new(path, Path::new("/repo")),
        Revs::worktree_against(CommitHash::new("base")),
    )
}

fn content(path: &str, after: &str) -> DiffContent {
    let before = ["fn main() { let value = 1; }"];
    let after = [after];
    let changed = diff::compute(&before, &after, Settings::default()).unwrap();
    DiffContent::Diff(Diff {
        file: file(path),
        alignment: diff::align(changed, &before, &after).unwrap(),
    })
}

fn diff_and_syntax_services(
    contents: Vec<DiffContent>,
) -> (DiffService, SyntaxService, Receiver<()>) {
    let (redraw_tx, redraws) = mpsc::channel();
    let diff_redraw = redraw_tx.clone();
    let diff_service = DiffService::new(
        move || {
            let _ = diff_redraw.send(());
        },
        move |file| {
            contents
                .iter()
                .find(|content| content.file() == file)
                .cloned()
                .ok_or_else(|| format!("cannot read {}", file.path()))
        },
    );
    let syntax_service = SyntaxService::start(move || {
        let _ = redraw_tx.send(());
    });
    (diff_service, syntax_service, redraws)
}

fn wait_until_idle(mut is_busy: impl FnMut() -> bool, redraws: &Receiver<()>) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while is_busy() {
        assert!(Instant::now() < deadline, "service never finished");
        let _ = redraws.recv_timeout(Duration::from_millis(20));
    }
}

#[test]
fn every_open_file_gets_its_own_diff_and_colours() {
    let (diff_service, syntax_service, redraws) = diff_and_syntax_services(vec![
        content("a.rs", "fn main() { let value = 2; }"),
        content("b.rs", "fn main() { let other = 3; }"),
    ]);
    let a = diff_service.open(file("a.rs"));
    let b = diff_service.open(file("b.rs"));
    wait_until_idle(|| diff_service.is_busy(), &redraws);

    let a_content = a.lock().unwrap().clone().unwrap().unwrap();
    let b_content = b.lock().unwrap().clone().unwrap().unwrap();
    let a_colours = syntax_service.colour(Arc::clone(&a_content));
    let b_colours = syntax_service.colour(Arc::clone(&b_content));
    wait_until_idle(|| syntax_service.is_busy(), &redraws);

    for ((content, colours), expected) in [(&a_content, &a_colours), (&b_content, &b_colours)]
        .into_iter()
        .zip(["let value = 2", "let other = 3"])
    {
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
    let (diff_service, _syntax_service, redraws) =
        diff_and_syntax_services(vec![content("good.rs", "fn good() {}")]);
    let bad = diff_service.open(file("bad.rs"));
    let good = diff_service.open(file("good.rs"));
    wait_until_idle(|| diff_service.is_busy(), &redraws);

    assert_eq!(
        bad.lock().unwrap().clone().unwrap().err().as_deref(),
        Some("cannot read bad.rs")
    );
    assert!(matches!(*good.lock().unwrap(), Some(Ok(_))));
}

#[test]
fn a_file_nobody_holds_any_more_is_not_read() {
    let (release, wait) = mpsc::channel::<()>();
    let wait = Mutex::new(wait);
    let (started, reading) = mpsc::channel::<()>();
    let reads = Arc::new(Mutex::new(Vec::new()));
    let read_paths = Arc::clone(&reads);
    let (redraw, redraws) = mpsc::channel();
    let diff_service = DiffService::new(
        move || {
            let _ = redraw.send(());
        },
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
    let first_a = diff_service.open(file("a.txt"));
    reading.recv_timeout(Duration::from_secs(10)).unwrap();
    let b = diff_service.open(file("b.txt"));
    drop(b);
    drop(first_a);
    let second_a = diff_service.open(file("a.txt"));
    release.send(()).unwrap();
    wait_until_idle(|| diff_service.is_busy(), &redraws);

    assert_eq!(*reads.lock().unwrap(), ["a.txt", "a.txt"]);
    assert!(matches!(*second_a.lock().unwrap(), Some(Ok(_))));
}

#[test]
fn a_listed_repository_reports_its_files_or_why_it_could_not() {
    let (redraw, redraws) = mpsc::channel();
    let mut responses = vec![Err("not a repository".to_owned()), Ok(vec![file("a.rs")])];
    let vcs_service = VcsService::new(
        move || {
            let _ = redraw.send(());
        },
        move |_, _| responses.pop().unwrap(),
        |_, _| Ok(Vec::new()),
    );

    let listed = vcs_service.list_files(Path::new("/repo"), RevisionRange::Worktree);
    wait_until_idle(|| vcs_service.is_busy(), &redraws);
    assert_eq!(*listed.lock().unwrap(), Some(Ok(vec![file("a.rs")])));

    let failed = vcs_service.list_files(Path::new("/elsewhere"), RevisionRange::Worktree);
    wait_until_idle(|| vcs_service.is_busy(), &redraws);
    assert_eq!(
        *failed.lock().unwrap(),
        Some(Err("not a repository".to_owned()))
    );
}

#[test]
fn refreshing_a_file_whose_text_is_the_same_keeps_its_arc_identity() {
    let text = content("a.rs", "fn main() { let value = 2; }");
    let (redraw, redraws) = mpsc::channel();
    let diff_service = DiffService::new(
        move || {
            let _ = redraw.send(());
        },
        move |_| Ok(text.clone()),
    );
    let slot = diff_service.open(file("a.rs"));
    wait_until_idle(|| diff_service.is_busy(), &redraws);
    let diff_before = slot.lock().unwrap().clone().unwrap().unwrap();

    diff_service.refresh(file("a.rs"), &slot);
    wait_until_idle(|| diff_service.is_busy(), &redraws);

    let diff_after = slot.lock().unwrap().clone().unwrap().unwrap();
    assert!(
        Arc::ptr_eq(&diff_before, &diff_after),
        "unchanged text must preserve Arc<DiffContent> pointer identity"
    );
}

#[test]
fn refreshing_a_file_whose_text_changed_replaces_its_diff_and_stops_dropped_syntax_job() {
    // Colouring 3000 lines takes long enough for dropping the old colours slot to abort chunking.
    let lines: Vec<String> = (0..3000)
        .map(|line| format!("let old{line} = 1;"))
        .collect();
    let lines: Vec<&str> = lines.iter().map(String::as_str).collect();
    let unchanged = diff::compute(&lines, &lines, Settings::default()).unwrap();
    let long_text = DiffContent::Diff(Diff {
        file: file("a.rs"),
        alignment: diff::align(unchanged, &lines, &lines).unwrap(),
    });
    let short_text = content("a.rs", "fn main() { let value = 3; }");
    let mut reads = vec![short_text, long_text];
    let (redraw_tx, redraws) = mpsc::channel();
    let diff_redraw = redraw_tx.clone();
    let diff_service = DiffService::new(
        move || {
            let _ = diff_redraw.send(());
        },
        move |_| Ok(reads.pop().unwrap()),
    );
    let syntax_service = SyntaxService::start(move || {
        let _ = redraw_tx.send(());
    });

    let slot = diff_service.open(file("a.rs"));
    wait_until_idle(|| diff_service.is_busy(), &redraws);
    let old_content = slot.lock().unwrap().clone().unwrap().unwrap();
    let old_colours = syntax_service.colour(old_content);

    diff_service.refresh(file("a.rs"), &slot);
    wait_until_idle(|| diff_service.is_busy(), &redraws);
    drop(old_colours);

    let new_content = slot.lock().unwrap().clone().unwrap().unwrap();
    let new_colours = syntax_service.colour(Arc::clone(&new_content));
    wait_until_idle(|| syntax_service.is_busy(), &redraws);

    assert_eq!(
        new_content
            .alignment()
            .unwrap()
            .line(DiffVersion::Modified, 1),
        Some("fn main() { let value = 3; }")
    );
    let colours = new_colours.lock().unwrap();
    assert_eq!(colours[DiffVersion::Original as usize].len(), 1);
    assert_eq!(colours[DiffVersion::Modified as usize].len(), 1);
}

struct FixtureRepo {
    dir: PathBuf,
}

impl FixtureRepo {
    fn new(name: &str) -> Self {
        let dir =
            std::env::temp_dir().join(format!("codediff-services-{name}-{}", std::process::id()));
        fixtures::repo(&dir).expect("building the fixture repository");
        Self { dir }
    }
}

impl Drop for FixtureRepo {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

#[test]
fn untracked_and_deleted_files_carry_one_sided_counts() {
    let fixture = FixtureRepo::new("one-sided-counts");
    let (redraw_tx, redraws) = mpsc::channel();
    let vcs_service = VcsService::start(move || {
        let _ = redraw_tx.send(());
    });
    let slot = vcs_service.list_files(&fixture.dir, RevisionRange::Worktree);
    wait_until_idle(|| vcs_service.is_busy(), &redraws);
    let files = slot.lock().unwrap().take().unwrap().unwrap();

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

    let binary = files
        .into_iter()
        .find(|file| file.path().as_str() == "picture.png")
        .expect("binary file is listed");
    let (diff_tx, diff_rx) = mpsc::channel();
    let diff_service = DiffService::start(
        move || {
            let _ = diff_tx.send(());
        },
        Settings::default(),
    );
    let binary_slot = diff_service.open(binary);
    wait_until_idle(|| diff_service.is_busy(), &diff_rx);
    assert_eq!(
        binary_slot.lock().unwrap().take().unwrap().err().as_deref(),
        Some("picture.png is binary — there are no lines to review")
    );
}

#[test]
fn a_file_staged_and_edited_again_carries_a_count_per_revision_pair() {
    let fixture = FixtureRepo::new("counts");
    let (redraw_tx, redraws) = mpsc::channel();
    let vcs_service = VcsService::start(move || {
        let _ = redraw_tx.send(());
    });
    let slot = vcs_service.list_files(&fixture.dir, RevisionRange::Worktree);
    wait_until_idle(|| vcs_service.is_busy(), &redraws);
    let files = slot.lock().unwrap().take().unwrap().unwrap();

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
fn commit_to_commit_listing_open_and_colour_produce_aligned_diffs_and_spans() {
    static NEXT_ID: AtomicUsize = AtomicUsize::new(0);
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!(
        "codediff-services-between-{}-{id}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("src")).expect("creating fixture dir");

    struct Cleanup(PathBuf);
    impl Drop for Cleanup {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
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

    fs::write(
        dir.join("src/engine.rs"),
        "pub fn compute(limit: usize) -> usize {
    let base = 1;
    limit + base
}
",
    )
    .unwrap();
    fs::write(
        dir.join("src/obsolete.rs"),
        "pub fn old() {}
",
    )
    .unwrap();
    git(&["add", "-A"], "2026-01-01T00:00:00Z");
    git(&["commit", "-qm", "first"], "2026-01-01T00:00:00Z");
    let first = git(&["rev-parse", "HEAD"], "2026-01-01T00:00:00Z");

    fs::write(
        dir.join("src/engine.rs"),
        "pub fn compute(limit: usize) -> usize {
    let step = 2;
    limit.saturating_add(step)
}
",
    )
    .unwrap();
    fs::write(
        dir.join("src/added.rs"),
        "pub fn added() -> bool {
    true
}
",
    )
    .unwrap();
    git(&["rm", "-q", "src/obsolete.rs"], "2026-01-02T00:00:00Z");
    git(&["add", "-A"], "2026-01-02T00:00:00Z");
    git(&["commit", "-qm", "second"], "2026-01-02T00:00:00Z");
    let second = git(&["rev-parse", "HEAD"], "2026-01-02T00:00:00Z");

    // Leave dirty working-tree noise to prove Between ignores the working tree.
    fs::write(
        dir.join("dirty.rs"),
        "pub fn dirty() {}
",
    )
    .unwrap();

    let (redraw_tx, redraw_rx) = mpsc::channel();
    let vcs_redraw = redraw_tx.clone();
    let vcs_service = VcsService::start(move || {
        let _ = vcs_redraw.send(());
    });
    let diff_redraw = redraw_tx.clone();
    let diff_service = DiffService::start(
        move || {
            let _ = diff_redraw.send(());
        },
        Settings::default(),
    );
    let syntax_service = SyntaxService::start(move || {
        let _ = redraw_tx.send(());
    });

    let slot = vcs_service.list_files(&dir, RevisionRange::Between(first, second));
    wait_until_idle(|| vcs_service.is_busy(), &redraw_rx);
    let files = slot
        .lock()
        .unwrap()
        .take()
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
    let diff_slot = diff_service.open(engine);
    wait_until_idle(|| diff_service.is_busy(), &redraw_rx);

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

    let colours_slot = syntax_service.colour(content);
    wait_until_idle(|| syntax_service.is_busy(), &redraw_rx);
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

#[test]
fn list_commits_and_toggle_stage_serialize_cleanly_on_vcs_service() {
    let dir = std::env::temp_dir().join(format!(
        "codediff-vcs-service-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&dir).expect("create tempdir");
    struct TempRepo(PathBuf);
    impl Drop for TempRepo {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    let repo = TempRepo(dir);
    let root = repo.0.as_path();

    let git = |args: &[&str]| {
        let status = Command::new("git")
            .args(args)
            .current_dir(root)
            .env("GIT_AUTHOR_NAME", "Tester")
            .env("GIT_AUTHOR_EMAIL", "tester@example.com")
            .env("GIT_COMMITTER_NAME", "Tester")
            .env("GIT_COMMITTER_EMAIL", "tester@example.com")
            .status()
            .expect("running git");
        assert!(status.success(), "git {args:?} failed");
    };

    git(&["init", "-b", "main", "--quiet"]);
    fs::write(
        root.join("a.txt"),
        "one
",
    )
    .unwrap();
    git(&["add", "."]);
    git(&["commit", "-m", "first commit", "--quiet"]);
    fs::write(
        root.join("a.txt"),
        "two
",
    )
    .unwrap();
    git(&["add", "."]);
    git(&["commit", "-m", "second commit", "--quiet"]);

    // Edit a.txt in worktree so toggle_stage can stage and unstage it sequentially.
    fs::write(
        root.join("a.txt"),
        "three
",
    )
    .unwrap();

    let (tx, rx) = mpsc::channel();
    let vcs_service = VcsService::start(move || {
        let _ = tx.send(());
    });
    let commit_slot = vcs_service.list_commits(root, 10);
    rx.recv_timeout(Duration::from_secs(5))
        .expect("redraw notification after list_commits");
    wait_until_idle(|| vcs_service.is_busy(), &rx);

    let commits = commit_slot
        .lock()
        .unwrap()
        .take()
        .expect("commits populated")
        .expect("commits ok");
    assert_eq!(commits.len(), 2);
    assert_eq!(commits[0].subject, "second commit");
    assert_eq!(commits[1].subject, "first commit");

    // Stage via VcsService::toggle_stage and immediately list_files on the same worker thread!
    let repo_path = RepoPath::new("a.txt", root);
    let worktree_revs = Revs::worktree_against(commits[0].hash.clone());
    vcs_service.toggle_stage(&repo_path, &worktree_revs);
    let staged_slot = vcs_service.list_files(root, RevisionRange::Worktree);
    wait_until_idle(|| vcs_service.is_busy(), &rx);

    let staged_files = staged_slot.lock().unwrap().take().unwrap().unwrap();
    assert_eq!(staged_files.len(), 1);
    assert_eq!(staged_files[0].revs().after, Rev::Index);

    // Unstage via VcsService::toggle_stage and list_files again!
    let index_revs = Revs::new(Rev::Commit(commits[0].hash.clone()), Rev::Index);
    vcs_service.toggle_stage(&repo_path, &index_revs);
    let unstaged_slot = vcs_service.list_files(root, RevisionRange::Worktree);
    wait_until_idle(|| vcs_service.is_busy(), &rx);

    let unstaged_files = unstaged_slot.lock().unwrap().take().unwrap().unwrap();
    assert_eq!(unstaged_files.len(), 1);
    assert_eq!(unstaged_files[0].revs().after, Rev::Worktree);
}
