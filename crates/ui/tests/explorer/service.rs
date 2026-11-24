use std::path::Path;
use std::rc::Rc;
use std::sync::mpsc;

use loom::testing::Harness;
use loom::{Node, Scope, component, rsx};
use ui::Theme;
use ui::components::{Context, Explorer, Ui, UiProps};
use ui::services::{VcsService, WatcherService};

use super::common::{fake_vcs, file, wait_vcs};

#[component]
fn Host(
    scope: &mut Scope,
    repo: Rc<Path>,
    vcs: Rc<VcsService>,
    watcher: Rc<WatcherService>,
) -> Node {
    let _ = scope;
    rsx! {
        Ui {
            value: Context {
                theme: Rc::new(Theme::DARK),
                repo: Rc::clone(repo),
                vcs: Some(Rc::clone(vcs)),
                watcher: Some(Rc::clone(watcher)),
                ..Context::default()
            },
            Explorer {}
        }
    }
}

#[component]
fn RevisionRangeHost(
    scope: &mut Scope,
    repo: Rc<Path>,
    revision_range: vcs::RevisionRange,
    vcs: Rc<VcsService>,
    watcher: Rc<WatcherService>,
) -> Node {
    let _ = scope;
    rsx! {
        Ui {
            value: Context {
                theme: Rc::new(Theme::DARK),
                repo: Rc::clone(repo),
                revision_range: revision_range.clone(),
                vcs: Some(Rc::clone(vcs)),
                watcher: Some(Rc::clone(watcher)),
                ..Context::default()
            },
            Explorer {}
        }
    }
}

#[test]
fn explorer_requests_its_file_list() {
    let (vcs, redraws) = fake_vcs(vec![vec![file("one.rs")]]);
    let watcher = Rc::new(WatcherService::fake());
    let mut harness = Harness::new::<Host>(
        HostProps {
            repo: Rc::from(Path::new("/one")),
            vcs: Rc::clone(&vcs),
            watcher,
        },
        40,
        5,
    );
    harness.force_draw();
    wait_vcs(&vcs, &redraws, &mut harness);

    assert!(
        harness
            .screen()
            .join(
                "
"
            )
            .contains("one.rs")
    );
}

#[test]
fn changing_repository_requests_a_new_file_list() {
    let (vcs, redraws) = fake_vcs(vec![vec![file("one.rs")], vec![file("two.rs")]]);
    let watcher = Rc::new(WatcherService::fake());
    let mut harness = Harness::new::<Host>(
        HostProps {
            repo: Rc::from(Path::new("/one")),
            vcs: Rc::clone(&vcs),
            watcher: Rc::clone(&watcher),
        },
        40,
        5,
    );
    harness.force_draw();
    wait_vcs(&vcs, &redraws, &mut harness);

    harness.set_props::<Host>(HostProps {
        repo: Rc::from(Path::new("/two")),
        vcs: Rc::clone(&vcs),
        watcher,
    });
    harness.force_draw();
    wait_vcs(&vcs, &redraws, &mut harness);

    let screen = harness.screen().join(
        "
",
    );
    assert!(screen.contains("two.rs"), "got {screen:?}");
    assert!(!screen.contains("one.rs"), "got {screen:?}");
}

#[test]
fn a_list_still_being_read_for_the_previous_repository_never_appears() {
    let (release, released) = mpsc::channel::<()>();
    let (redraw, redraws) = mpsc::channel();
    let vcs = Rc::new(VcsService::fake(
        move || {
            let _ = redraw.send(());
        },
        move |repo, _| {
            if repo == Path::new("/one") {
                released.recv().unwrap();
                Ok(vec![file("one.rs")])
            } else {
                Ok(vec![file("two.rs")])
            }
        },
        |_, _| Ok(Vec::new()),
    ));
    let watcher = Rc::new(WatcherService::fake());
    let mut harness = Harness::new::<Host>(
        HostProps {
            repo: Rc::from(Path::new("/one")),
            vcs: Rc::clone(&vcs),
            watcher: Rc::clone(&watcher),
        },
        40,
        5,
    );
    harness.force_draw();
    harness.set_props::<Host>(HostProps {
        repo: Rc::from(Path::new("/two")),
        vcs: Rc::clone(&vcs),
        watcher,
    });
    harness.force_draw();
    release.send(()).unwrap();
    wait_vcs(&vcs, &redraws, &mut harness);

    let screen = harness.screen().join(
        "
",
    );
    assert!(screen.contains("two.rs"), "got {screen:?}");
    assert!(!screen.contains("one.rs"), "got {screen:?}");
}

#[test]
fn a_list_that_cannot_be_read_says_why() {
    let (redraw, redraws) = mpsc::channel();
    let vcs = Rc::new(VcsService::fake(
        move || {
            let _ = redraw.send(());
        },
        |_, _| Err("not a git repository".to_owned()),
        |_, _| Ok(Vec::new()),
    ));
    let mut harness = Harness::new::<Host>(
        HostProps {
            repo: Rc::from(Path::new("/repo")),
            vcs: Rc::clone(&vcs),
            watcher: Rc::new(WatcherService::fake()),
        },
        60,
        5,
    );
    harness.force_draw();
    wait_vcs(&vcs, &redraws, &mut harness);

    assert!(
        harness
            .screen_line(0)
            .contains("Cannot list files: not a git repository"),
        "{:?}",
        harness.screen()
    );
}

#[test]
fn refresh_keeps_the_same_file_selected() {
    let (vcs, redraws) = fake_vcs(vec![
        vec![file("a.rs"), file("b.rs"), file("c.rs")],
        vec![file("0.rs"), file("a.rs"), file("b.rs"), file("c.rs")],
    ]);
    let watcher = Rc::new(WatcherService::fake());
    let mut harness = Harness::new::<Host>(
        HostProps {
            repo: Rc::from(Path::new("/repo")),
            vcs: Rc::clone(&vcs),
            watcher: Rc::clone(&watcher),
        },
        40,
        6,
    );
    harness.force_draw();
    wait_vcs(&vcs, &redraws, &mut harness);
    harness.press(crokey::key!(j));
    harness.force_draw();
    harness.press(crokey::key!(j));
    harness.force_draw();
    let selected_background = harness.style_at(0, 2).bg;
    assert_ne!(selected_background, harness.style_at(0, 1).bg);

    watcher.notify(watcher::Refresh {
        worktree: true,
        ..watcher::Refresh::default()
    });
    wait_vcs(&vcs, &redraws, &mut harness);

    assert!(harness.screen_line(3).contains("b.rs"));
    assert_eq!(harness.style_at(0, 3).bg, selected_background);
    assert_ne!(harness.style_at(0, 2).bg, selected_background);
}

#[test]
fn filesystem_changes_refresh_the_current_repository() {
    let (vcs, redraws) = fake_vcs(vec![vec![file("before.rs")], vec![file("after.rs")]]);
    let watcher = Rc::new(WatcherService::fake());
    let mut harness = Harness::new::<Host>(
        HostProps {
            repo: Rc::from(Path::new("/repo")),
            vcs: Rc::clone(&vcs),
            watcher: Rc::clone(&watcher),
        },
        40,
        5,
    );
    harness.force_draw();
    wait_vcs(&vcs, &redraws, &mut harness);

    watcher.notify(watcher::Refresh {
        worktree: true,
        ..watcher::Refresh::default()
    });
    wait_vcs(&vcs, &redraws, &mut harness);

    let screen = harness.screen().join(
        "
",
    );
    assert!(screen.contains("after.rs"), "got {screen:?}");
}

#[test]
fn changing_revision_range_requests_the_new_revision_range_and_drops_stale_in_flight_lists() {
    let (started, listing_worktree) = mpsc::channel::<()>();
    let (release, released) = mpsc::channel::<()>();
    let (redraw, redraws) = mpsc::channel();
    let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let seen_in_worker = std::sync::Arc::clone(&seen);
    let vcs = Rc::new(VcsService::fake(
        move || {
            let _ = redraw.send(());
        },
        move |_, revision_range| {
            seen_in_worker.lock().unwrap().push(revision_range.clone());
            match revision_range {
                vcs::RevisionRange::Worktree => {
                    started.send(()).unwrap();
                    released.recv().unwrap();
                    Ok(vec![file("worktree.rs")])
                }
                vcs::RevisionRange::Between(first, second) if first == "a" && second == "b" => {
                    Ok(vec![file("between.rs")])
                }
                other => panic!("unexpected revision_range {other:?}"),
            }
        },
        |_, _| Ok(Vec::new()),
    ));
    let watcher = Rc::new(WatcherService::fake());
    let mut harness = Harness::new::<RevisionRangeHost>(
        RevisionRangeHostProps {
            repo: Rc::from(Path::new("/repo")),
            revision_range: vcs::RevisionRange::Worktree,
            vcs: Rc::clone(&vcs),
            watcher: Rc::clone(&watcher),
        },
        40,
        5,
    );
    harness.force_draw();
    listing_worktree
        .recv_timeout(std::time::Duration::from_secs(10))
        .expect("the vcs worker started listing Worktree");

    harness.set_props::<RevisionRangeHost>(RevisionRangeHostProps {
        repo: Rc::from(Path::new("/repo")),
        revision_range: vcs::RevisionRange::Between("a".into(), "b".into()),
        vcs: Rc::clone(&vcs),
        watcher: Rc::clone(&watcher),
    });
    harness.force_draw();
    release.send(()).unwrap();
    wait_vcs(&vcs, &redraws, &mut harness);

    let screen = harness.screen().join(
        "
",
    );
    assert!(screen.contains("between.rs"), "got {screen:?}");
    assert!(!screen.contains("worktree.rs"), "got {screen:?}");
    assert_eq!(
        *seen.lock().unwrap(),
        vec![
            vcs::RevisionRange::Worktree,
            vcs::RevisionRange::Between("a".into(), "b".into()),
        ]
    );
}

#[test]
fn worktree_only_refresh_does_not_relist_commit_to_commit_range() {
    let calls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let calls_in_worker = std::sync::Arc::clone(&calls);
    let (redraw, redraws) = mpsc::channel();
    let vcs = Rc::new(VcsService::fake(
        move || {
            let _ = redraw.send(());
        },
        move |_, _| {
            let call = calls_in_worker.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(vec![file(&format!("call-{call}.rs"))])
        },
        |_, _| Ok(Vec::new()),
    ));
    let watcher = Rc::new(WatcherService::fake());
    let mut harness = Harness::new::<RevisionRangeHost>(
        RevisionRangeHostProps {
            repo: Rc::from(Path::new("/repo")),
            revision_range: vcs::RevisionRange::Between("a".into(), "b".into()),
            vcs: Rc::clone(&vcs),
            watcher: Rc::clone(&watcher),
        },
        40,
        5,
    );
    harness.force_draw();
    wait_vcs(&vcs, &redraws, &mut harness);
    assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 1);

    watcher.notify(watcher::Refresh {
        worktree: true,
        index: true,
        ..watcher::Refresh::default()
    });
    wait_vcs(&vcs, &redraws, &mut harness);
    assert_eq!(
        calls.load(std::sync::atomic::Ordering::SeqCst),
        1,
        "worktree/index edits do not re-run commit-to-commit diff"
    );

    watcher.notify(watcher::Refresh {
        refs: true,
        ..watcher::Refresh::default()
    });
    wait_vcs(&vcs, &redraws, &mut harness);
    assert_eq!(
        calls.load(std::sync::atomic::Ordering::SeqCst),
        2,
        "ref updates re-list revision ranges that may name a branch"
    );
}

#[test]
fn pressing_space_on_a_commit_to_commit_file_does_not_stage_worktree_changes() {
    use std::process::Command;

    let repo_dir = tempfile::tempdir().expect("creating temp git repo");
    let git = |args: &[&str]| {
        let out = Command::new("git")
            .args(args)
            .current_dir(repo_dir.path())
            .env("GIT_AUTHOR_NAME", "codediff")
            .env("GIT_AUTHOR_EMAIL", "test@codediff.test")
            .env("GIT_AUTHOR_DATE", "2026-01-01T00:00:00Z")
            .env("GIT_COMMITTER_NAME", "codediff")
            .env("GIT_COMMITTER_EMAIL", "test@codediff.test")
            .env("GIT_COMMITTER_DATE", "2026-01-01T00:00:00Z")
            .output()
            .expect("running git");
        assert!(
            out.status.success(),
            "{:?}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout).trim().to_owned()
    };

    git(&["init", "-q", "-b", "main"]);
    git(&["config", "core.autocrlf", "false"]);
    std::fs::create_dir_all(repo_dir.path().join("src")).unwrap();
    std::fs::write(
        repo_dir.path().join("src/lib.rs"),
        "pub fn v1() {}
",
    )
    .unwrap();
    git(&["add", "-A"]);
    git(&["commit", "-qm", "first"]);
    let first = git(&["rev-parse", "HEAD"]);

    std::fs::write(
        repo_dir.path().join("src/lib.rs"),
        "pub fn v2() {}
",
    )
    .unwrap();
    git(&["add", "-A"]);
    git(&["commit", "-qm", "second"]);
    let second = git(&["rev-parse", "HEAD"]);

    // Modify the same path in the working tree so an accidental `git add` would stage it.
    std::fs::write(
        repo_dir.path().join("src/lib.rs"),
        "pub fn dirty() {}
",
    )
    .unwrap();

    let (redraw_tx, redraws) = mpsc::channel();
    let vcs = Rc::new(VcsService::start(move || {
        let _ = redraw_tx.send(());
    }));
    let mut harness = Harness::new::<RevisionRangeHost>(
        RevisionRangeHostProps {
            repo: Rc::from(repo_dir.path()),
            revision_range: vcs::RevisionRange::Between(first, second),
            vcs: Rc::clone(&vcs),
            watcher: Rc::new(WatcherService::fake()),
        },
        50,
        6,
    );
    harness.force_draw();
    wait_vcs(&vcs, &redraws, &mut harness);

    // Line 0: Heading ("Changes"), Line 1: Directory ("src"), Line 2: File ("lib.rs").
    // Press Space on both the directory and the file.
    harness.press(crokey::key!(j));
    harness.force_draw();
    assert!(harness.screen_line(1).contains("src"));
    harness.press(crokey::key!(space));
    harness.force_draw();

    harness.press(crokey::key!(j));
    harness.force_draw();
    assert!(harness.screen_line(2).contains("lib.rs"));
    harness.press(crokey::key!(space));
    harness.force_draw();

    wait_vcs(&vcs, &redraws, &mut harness);

    let staged = git(&["diff", "--cached", "--name-only"]);
    assert!(
        staged.is_empty(),
        "commit-to-commit Space must not stage working-tree edits, got staged={staged:?}"
    );
}
