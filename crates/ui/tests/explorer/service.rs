use std::path::Path;
use std::rc::Rc;
use std::time::Duration;

use loom::testing::Harness;
use loom::{Node, Scope, component, rsx};
use ui::Theme;
use ui::components::{Context, Explorer, Ui, UiProps};
use ui::services::files::FilesService;
use ui::services::watcher::WatcherService;

use super::common::{file, mock_files_service};

#[component]
fn Host(
    scope: &mut Scope,
    repo: Rc<Path>,
    files_service: Rc<FilesService>,
    watcher_service: Rc<WatcherService>,
) -> Node {
    let _ = scope;
    rsx! {
        Ui {
            value: Context {
                theme: Rc::new(Theme::DARK),
                repo: Rc::clone(repo),
                files_service: Some(Rc::clone(files_service)),
                watcher_service: Some(Rc::clone(watcher_service)),
                ..Context::default()
            },
            Explorer {}
        }
    }
}

fn receive(
    harness: &mut Harness,
    files_service: &FilesService,
    responses: &std::sync::mpsc::Receiver<pipeline::files::Response>,
) {
    files_service.deliver(
        responses
            .recv_timeout(Duration::from_secs(1))
            .expect("files response"),
    );
    harness.force_draw().force_draw();
}

#[test]
fn explorer_requests_its_file_list() {
    let (files_service, responses) = mock_files_service(vec![vec![file("one.rs")]]);
    let watcher_service = Rc::new(WatcherService::new());
    let mut harness = Harness::new::<Host>(
        HostProps {
            repo: Rc::from(Path::new("/one")),
            files_service: Rc::clone(&files_service),
            watcher_service,
        },
        40,
        5,
    );
    harness.force_draw();
    receive(&mut harness, &files_service, &responses);

    assert!(harness.screen().join("\n").contains("one.rs"));
}

#[test]
fn changing_repository_requests_a_new_file_list() {
    let (files_service, responses) =
        mock_files_service(vec![vec![file("one.rs")], vec![file("two.rs")]]);
    let watcher_service = Rc::new(WatcherService::new());
    let mut harness = Harness::new::<Host>(
        HostProps {
            repo: Rc::from(Path::new("/one")),
            files_service: Rc::clone(&files_service),
            watcher_service: Rc::clone(&watcher_service),
        },
        40,
        5,
    );
    harness.force_draw();
    receive(&mut harness, &files_service, &responses);

    harness.set_props::<Host>(HostProps {
        repo: Rc::from(Path::new("/two")),
        files_service: Rc::clone(&files_service),
        watcher_service,
    });
    harness.force_draw();
    receive(&mut harness, &files_service, &responses);

    let screen = harness.screen().join("\n");
    assert!(screen.contains("two.rs"), "got {screen:?}");
    assert!(!screen.contains("one.rs"), "got {screen:?}");
}

#[test]
fn a_late_response_from_the_previous_repository_is_ignored() {
    let (files_service, responses) =
        mock_files_service(vec![vec![file("one.rs")], vec![file("two.rs")]]);
    let watcher_service = Rc::new(WatcherService::new());
    let mut harness = Harness::new::<Host>(
        HostProps {
            repo: Rc::from(Path::new("/one")),
            files_service: Rc::clone(&files_service),
            watcher_service: Rc::clone(&watcher_service),
        },
        40,
        5,
    );
    harness.force_draw();
    receive(&mut harness, &files_service, &responses);

    harness.set_props::<Host>(HostProps {
        repo: Rc::from(Path::new("/two")),
        files_service: Rc::clone(&files_service),
        watcher_service,
    });
    harness.force_draw();
    files_service.deliver(pipeline::files::Response {
        repo: "/one".into(),
        files: vec![file("stale.rs")],
    });
    harness.force_draw();

    let screen = harness.screen().join("\n");
    assert!(screen.contains("one.rs"), "got {screen:?}");
    assert!(!screen.contains("stale.rs"), "got {screen:?}");

    receive(&mut harness, &files_service, &responses);
    assert!(harness.screen().join("\n").contains("two.rs"));
}

#[test]
fn refresh_keeps_the_same_file_selected() {
    let (files_service, responses) = mock_files_service(vec![
        vec![file("a.rs"), file("b.rs"), file("c.rs")],
        vec![file("0.rs"), file("a.rs"), file("b.rs"), file("c.rs")],
    ]);
    let watcher_service = Rc::new(WatcherService::new());
    let mut harness = Harness::new::<Host>(
        HostProps {
            repo: Rc::from(Path::new("/repo")),
            files_service: Rc::clone(&files_service),
            watcher_service: Rc::clone(&watcher_service),
        },
        40,
        6,
    );
    harness.force_draw();
    receive(&mut harness, &files_service, &responses);
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
    receive(&mut harness, &files_service, &responses);

    assert!(harness.screen_row(3).contains("b.rs"));
    assert_eq!(harness.style_at(0, 3).bg, selected_background);
    assert_ne!(harness.style_at(0, 2).bg, selected_background);
}

#[test]
fn filesystem_changes_refresh_the_current_repository() {
    let (files_service, responses) =
        mock_files_service(vec![vec![file("before.rs")], vec![file("after.rs")]]);
    let watcher_service = Rc::new(WatcherService::new());
    let mut harness = Harness::new::<Host>(
        HostProps {
            repo: Rc::from(Path::new("/repo")),
            files_service: Rc::clone(&files_service),
            watcher_service: Rc::clone(&watcher_service),
        },
        40,
        5,
    );
    harness.force_draw();
    receive(&mut harness, &files_service, &responses);

    watcher_service.deliver(watcher::Refresh {
        worktree: true,
        ..watcher::Refresh::default()
    });
    receive(&mut harness, &files_service, &responses);

    let screen = harness.screen().join("\n");
    assert!(screen.contains("after.rs"), "got {screen:?}");
}
