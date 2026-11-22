use std::path::Path;
use std::rc::Rc;
use std::sync::mpsc;

use diff_service::DiffService;
use loom::testing::Harness;
use loom::{Node, Scope, component, rsx};
use ui::Theme;
use ui::components::{Context, Explorer, Ui, UiProps};
use ui::services::watcher::WatcherService;

use super::common::{fake_pipeline, file, wait};

#[component]
fn Host(
    scope: &mut Scope,
    repo: Rc<Path>,
    pipeline: Rc<DiffService>,
    watcher_service: Rc<WatcherService>,
) -> Node {
    let _ = scope;
    rsx! {
        Ui {
            value: Context {
                theme: Rc::new(Theme::DARK),
                repo: Rc::clone(repo),
                diff_service: Some(Rc::clone(pipeline)),
                watcher_service: Some(Rc::clone(watcher_service)),
                ..Context::default()
            },
            Explorer {}
        }
    }
}

#[component]
fn DiffTypeHost(
    scope: &mut Scope,
    repo: Rc<Path>,
    diff_type: vcs::DiffType,
    pipeline: Rc<DiffService>,
    watcher_service: Rc<WatcherService>,
    version_control_service: Option<Rc<ui::services::version_control::VersionControlService>>,
) -> Node {
    let _ = scope;
    rsx! {
        Ui {
            value: Context {
                theme: Rc::new(Theme::DARK),
                repo: Rc::clone(repo),
                diff_type: diff_type.clone(),
                diff_service: Some(Rc::clone(pipeline)),
                watcher_service: Some(Rc::clone(watcher_service)),
                version_control_service: version_control_service.as_ref().map(Rc::clone),
                ..Context::default()
            },
            Explorer {}
        }
    }
}

#[test]
fn explorer_requests_its_file_list() {
    let (pipeline, redraws) = fake_pipeline(vec![vec![file("one.rs")]], Vec::new());
    let watcher_service = Rc::new(WatcherService::new());
    let mut harness = Harness::new::<Host>(
        HostProps {
            repo: Rc::from(Path::new("/one")),
            pipeline: Rc::clone(&pipeline),
            watcher_service,
        },
        40,
        5,
    );
    harness.force_draw();
    wait(&pipeline, &redraws, &mut harness);

    assert!(harness.screen().join("\n").contains("one.rs"));
}

#[test]
fn changing_repository_requests_a_new_file_list() {
    let (pipeline, redraws) =
        fake_pipeline(vec![vec![file("one.rs")], vec![file("two.rs")]], Vec::new());
    let watcher_service = Rc::new(WatcherService::new());
    let mut harness = Harness::new::<Host>(
        HostProps {
            repo: Rc::from(Path::new("/one")),
            pipeline: Rc::clone(&pipeline),
            watcher_service: Rc::clone(&watcher_service),
        },
        40,
        5,
    );
    harness.force_draw();
    wait(&pipeline, &redraws, &mut harness);

    harness.set_props::<Host>(HostProps {
        repo: Rc::from(Path::new("/two")),
        pipeline: Rc::clone(&pipeline),
        watcher_service,
    });
    harness.force_draw();
    wait(&pipeline, &redraws, &mut harness);

    let screen = harness.screen().join("\n");
    assert!(screen.contains("two.rs"), "got {screen:?}");
    assert!(!screen.contains("one.rs"), "got {screen:?}");
}

#[test]
fn a_list_still_being_read_for_the_previous_repository_never_appears() {
    let (release, released) = mpsc::channel::<()>();
    let (redraw, redraws) = mpsc::channel();
    let pipeline = Rc::new(DiffService::new(
        move || {
            let _ = redraw.send(());
        },
        move |request| {
            if request.repo == Path::new("/one") {
                released.recv().unwrap();
                Ok(vec![file("one.rs")])
            } else {
                Ok(vec![file("two.rs")])
            }
        },
        |_| unreachable!("nothing is opened"),
    ));
    let watcher_service = Rc::new(WatcherService::new());
    let mut harness = Harness::new::<Host>(
        HostProps {
            repo: Rc::from(Path::new("/one")),
            pipeline: Rc::clone(&pipeline),
            watcher_service: Rc::clone(&watcher_service),
        },
        40,
        5,
    );
    harness.force_draw();
    harness.set_props::<Host>(HostProps {
        repo: Rc::from(Path::new("/two")),
        pipeline: Rc::clone(&pipeline),
        watcher_service,
    });
    harness.force_draw();
    release.send(()).unwrap();
    wait(&pipeline, &redraws, &mut harness);

    let screen = harness.screen().join("\n");
    assert!(screen.contains("two.rs"), "got {screen:?}");
    assert!(!screen.contains("one.rs"), "got {screen:?}");
}

#[test]
fn a_list_that_cannot_be_read_says_why() {
    let (redraw, redraws) = mpsc::channel();
    let pipeline = Rc::new(DiffService::new(
        move || {
            let _ = redraw.send(());
        },
        |_| Err("not a git repository".to_owned()),
        |_| unreachable!("nothing is opened"),
    ));
    let mut harness = Harness::new::<Host>(
        HostProps {
            repo: Rc::from(Path::new("/repo")),
            pipeline: Rc::clone(&pipeline),
            watcher_service: Rc::new(WatcherService::new()),
        },
        60,
        5,
    );
    harness.force_draw();
    wait(&pipeline, &redraws, &mut harness);

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
    let (pipeline, redraws) = fake_pipeline(
        vec![
            vec![file("a.rs"), file("b.rs"), file("c.rs")],
            vec![file("0.rs"), file("a.rs"), file("b.rs"), file("c.rs")],
        ],
        Vec::new(),
    );
    let watcher_service = Rc::new(WatcherService::new());
    let mut harness = Harness::new::<Host>(
        HostProps {
            repo: Rc::from(Path::new("/repo")),
            pipeline: Rc::clone(&pipeline),
            watcher_service: Rc::clone(&watcher_service),
        },
        40,
        6,
    );
    harness.force_draw();
    wait(&pipeline, &redraws, &mut harness);
    harness.press(crokey::key!(j));
    harness.force_draw();
    harness.press(crokey::key!(j));
    harness.force_draw();
    let selected_background = harness.style_at(0, 2).bg;
    assert_ne!(selected_background, harness.style_at(0, 1).bg);

    watcher_service.deliver(watcher::Refresh {
        worktree: true,
        ..watcher::Refresh::default()
    });
    wait(&pipeline, &redraws, &mut harness);

    assert!(harness.screen_line(3).contains("b.rs"));
    assert_eq!(harness.style_at(0, 3).bg, selected_background);
    assert_ne!(harness.style_at(0, 2).bg, selected_background);
}

#[test]
fn filesystem_changes_refresh_the_current_repository() {
    let (pipeline, redraws) = fake_pipeline(
        vec![vec![file("before.rs")], vec![file("after.rs")]],
        Vec::new(),
    );
    let watcher_service = Rc::new(WatcherService::new());
    let mut harness = Harness::new::<Host>(
        HostProps {
            repo: Rc::from(Path::new("/repo")),
            pipeline: Rc::clone(&pipeline),
            watcher_service: Rc::clone(&watcher_service),
        },
        40,
        5,
    );
    harness.force_draw();
    wait(&pipeline, &redraws, &mut harness);

    watcher_service.deliver(watcher::Refresh {
        worktree: true,
        ..watcher::Refresh::default()
    });
    wait(&pipeline, &redraws, &mut harness);

    let screen = harness.screen().join("\n");
    assert!(screen.contains("after.rs"), "got {screen:?}");
}

#[test]
fn changing_diff_type_requests_the_new_revision_range_and_drops_stale_in_flight_lists() {
    let (release, released) = mpsc::channel::<()>();
    let (redraw, redraws) = mpsc::channel();
    let seen = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let seen_in_worker = std::sync::Arc::clone(&seen);
    let pipeline = Rc::new(DiffService::new(
        move || {
            let _ = redraw.send(());
        },
        move |request| {
            seen_in_worker
                .lock()
                .unwrap()
                .push(request.diff_type.clone());
            match &request.diff_type {
                vcs::DiffType::Worktree => {
                    released.recv().unwrap();
                    Ok(vec![file("worktree.rs")])
                }
                vcs::DiffType::Between(first, second) if first == "a" && second == "b" => {
                    Ok(vec![file("between.rs")])
                }
                other => panic!("unexpected diff_type {other:?}"),
            }
        },
        |_| unreachable!("nothing is opened"),
    ));
    let watcher_service = Rc::new(WatcherService::new());
    let mut harness = Harness::new::<DiffTypeHost>(
        DiffTypeHostProps {
            repo: Rc::from(Path::new("/repo")),
            diff_type: vcs::DiffType::Worktree,
            pipeline: Rc::clone(&pipeline),
            watcher_service: Rc::clone(&watcher_service),
            version_control_service: None,
        },
        40,
        5,
    );
    harness.force_draw();

    harness.set_props::<DiffTypeHost>(DiffTypeHostProps {
        repo: Rc::from(Path::new("/repo")),
        diff_type: vcs::DiffType::Between("a".into(), "b".into()),
        pipeline: Rc::clone(&pipeline),
        watcher_service: Rc::clone(&watcher_service),
        version_control_service: None,
    });
    harness.force_draw();
    release.send(()).unwrap();
    wait(&pipeline, &redraws, &mut harness);

    let screen = harness.screen().join("\n");
    assert!(screen.contains("between.rs"), "got {screen:?}");
    assert!(!screen.contains("worktree.rs"), "got {screen:?}");
    assert_eq!(
        *seen.lock().unwrap(),
        vec![
            vcs::DiffType::Worktree,
            vcs::DiffType::Between("a".into(), "b".into()),
        ]
    );
}

#[test]
fn worktree_only_refresh_does_not_relist_commit_to_commit_range() {
    let calls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let calls_in_worker = std::sync::Arc::clone(&calls);
    let (redraw, redraws) = mpsc::channel();
    let pipeline = Rc::new(DiffService::new(
        move || {
            let _ = redraw.send(());
        },
        move |_| {
            let call = calls_in_worker.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            Ok(vec![file(&format!("call-{call}.rs"))])
        },
        |_| unreachable!("nothing is opened"),
    ));
    let watcher_service = Rc::new(WatcherService::new());
    let mut harness = Harness::new::<DiffTypeHost>(
        DiffTypeHostProps {
            repo: Rc::from(Path::new("/repo")),
            diff_type: vcs::DiffType::Between("a".into(), "b".into()),
            pipeline: Rc::clone(&pipeline),
            watcher_service: Rc::clone(&watcher_service),
            version_control_service: None,
        },
        40,
        5,
    );
    harness.force_draw();
    wait(&pipeline, &redraws, &mut harness);
    assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 1);

    watcher_service.deliver(watcher::Refresh {
        worktree: true,
        index: true,
        ..watcher::Refresh::default()
    });
    wait(&pipeline, &redraws, &mut harness);
    assert_eq!(
        calls.load(std::sync::atomic::Ordering::SeqCst),
        1,
        "worktree/index edits do not re-run commit-to-commit diff"
    );

    watcher_service.deliver(watcher::Refresh {
        refs: true,
        ..watcher::Refresh::default()
    });
    wait(&pipeline, &redraws, &mut harness);
    assert_eq!(
        calls.load(std::sync::atomic::Ordering::SeqCst),
        2,
        "ref updates re-list revision ranges that may name a branch"
    );
}

#[test]
fn pressing_space_on_a_commit_to_commit_file_does_not_stage_worktree_changes() {
    use std::process::Command;

    use file_types::{File, Oid, RepoPath, Rev, Revs};
    use ui::services::version_control::VersionControlService;

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
    std::fs::write(repo_dir.path().join("src/lib.rs"), "pub fn v1() {}\n").unwrap();
    git(&["add", "-A"]);
    git(&["commit", "-qm", "first"]);
    let first = git(&["rev-parse", "HEAD"]);

    std::fs::write(repo_dir.path().join("src/lib.rs"), "pub fn v2() {}\n").unwrap();
    git(&["add", "-A"]);
    git(&["commit", "-qm", "second"]);
    let second = git(&["rev-parse", "HEAD"]);

    // Modify the same path in the working tree so an accidental `git add` would stage it.
    std::fs::write(repo_dir.path().join("src/lib.rs"), "pub fn dirty() {}\n").unwrap();

    let commit_file = File::unchanged_path(
        RepoPath::new("src/lib.rs", repo_dir.path()),
        Revs::new(
            Rev::Commit(Oid::new(&first)),
            Rev::Commit(Oid::new(&second)),
        ),
    );
    let (pipeline, redraws) = fake_pipeline(vec![vec![commit_file]], Vec::new());
    let mut harness = Harness::new::<DiffTypeHost>(
        DiffTypeHostProps {
            repo: Rc::from(repo_dir.path()),
            diff_type: vcs::DiffType::Between(first, second),
            pipeline: Rc::clone(&pipeline),
            watcher_service: Rc::new(WatcherService::new()),
            version_control_service: Some(Rc::new(VersionControlService::new())),
        },
        50,
        6,
    );
    harness.force_draw();
    wait(&pipeline, &redraws, &mut harness);

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

    // Give any background thread plenty of time if one had been spawned.
    std::thread::sleep(std::time::Duration::from_millis(120));

    let staged = git(&["diff", "--cached", "--name-only"]);
    assert!(
        staged.is_empty(),
        "commit-to-commit Space must not stage working-tree edits, got staged={staged:?}"
    );
}
