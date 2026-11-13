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
