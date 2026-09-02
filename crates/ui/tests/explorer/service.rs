use std::path::Path;
use std::rc::Rc;
use std::time::Duration;

use loom::testing::Harness;
use loom::{Node, Scope, component, rsx};
use ui::Theme;
use ui::components::{Context, Explorer, Ui, UiProps};
use ui::services::files::FilesService;

use super::common::{file, mock_file_service};

#[component]
fn Host(scope: &mut Scope, repo: Rc<Path>, file_service: Rc<FilesService>) -> Node {
    let _ = scope;
    rsx! {
        Ui {
            value: Context {
                theme: Rc::new(Theme::DARK),
                repo: Rc::clone(repo),
                file_service: Some(Rc::clone(file_service)),
                ..Context::default()
            },
            Explorer {}
        }
    }
}

fn receive(
    harness: &mut Harness,
    file_service: &FilesService,
    responses: &std::sync::mpsc::Receiver<pipeline::files::Response>,
) {
    file_service.deliver(
        responses
            .recv_timeout(Duration::from_secs(1))
            .expect("file list response"),
    );
    harness.force_draw().force_draw();
}

#[test]
fn explorer_requests_its_file_list() {
    let (file_service, responses) = mock_file_service(vec![vec![file("one.rs")]]);
    let mut harness = Harness::new::<Host>(
        HostProps {
            repo: Rc::from(Path::new("/one")),
            file_service: Rc::clone(&file_service),
        },
        40,
        5,
    );
    harness.force_draw();
    receive(&mut harness, &file_service, &responses);

    assert!(harness.screen().join("\n").contains("one.rs"));
}

#[test]
fn changing_repository_requests_a_new_file_list() {
    let (file_service, responses) =
        mock_file_service(vec![vec![file("one.rs")], vec![file("two.rs")]]);
    let mut harness = Harness::new::<Host>(
        HostProps {
            repo: Rc::from(Path::new("/one")),
            file_service: Rc::clone(&file_service),
        },
        40,
        5,
    );
    harness.force_draw();
    receive(&mut harness, &file_service, &responses);

    harness.set_props::<Host>(HostProps {
        repo: Rc::from(Path::new("/two")),
        file_service: Rc::clone(&file_service),
    });
    harness.force_draw();
    receive(&mut harness, &file_service, &responses);

    let screen = harness.screen().join("\n");
    assert!(screen.contains("two.rs"), "got {screen:?}");
    assert!(!screen.contains("one.rs"), "got {screen:?}");
}

#[test]
fn a_late_response_from_the_previous_repository_is_ignored() {
    let (file_service, responses) =
        mock_file_service(vec![vec![file("one.rs")], vec![file("two.rs")]]);
    let mut harness = Harness::new::<Host>(
        HostProps {
            repo: Rc::from(Path::new("/one")),
            file_service: Rc::clone(&file_service),
        },
        40,
        5,
    );
    harness.force_draw();
    receive(&mut harness, &file_service, &responses);

    harness.set_props::<Host>(HostProps {
        repo: Rc::from(Path::new("/two")),
        file_service: Rc::clone(&file_service),
    });
    harness.force_draw();
    file_service.deliver(pipeline::files::Response {
        repo: "/one".into(),
        files: vec![file("stale.rs")],
    });
    harness.force_draw();

    let screen = harness.screen().join("\n");
    assert!(screen.contains("one.rs"), "got {screen:?}");
    assert!(!screen.contains("stale.rs"), "got {screen:?}");

    receive(&mut harness, &file_service, &responses);
    assert!(harness.screen().join("\n").contains("two.rs"));
}

#[test]
fn refresh_keeps_the_cursor_on_the_same_file() {
    let (file_service, responses) = mock_file_service(vec![
        vec![file("a.rs"), file("b.rs"), file("c.rs")],
        vec![file("0.rs"), file("a.rs"), file("b.rs"), file("c.rs")],
    ]);
    let mut harness = Harness::new::<Host>(
        HostProps {
            repo: Rc::from(Path::new("/repo")),
            file_service: Rc::clone(&file_service),
        },
        40,
        6,
    );
    harness.force_draw();
    receive(&mut harness, &file_service, &responses);
    harness.press(crokey::key!(j));
    harness.force_draw();
    harness.press(crokey::key!(j));
    harness.force_draw();
    let selected_background = harness.style_at(0, 2).bg;
    assert_ne!(selected_background, harness.style_at(0, 1).bg);

    file_service.fs_changed(watcher::Refresh {
        worktree: true,
        ..watcher::Refresh::default()
    });
    receive(&mut harness, &file_service, &responses);

    assert!(harness.screen_row(3).contains("b.rs"));
    assert_eq!(harness.style_at(0, 3).bg, selected_background);
    assert_ne!(harness.style_at(0, 2).bg, selected_background);
}

#[test]
fn filesystem_changes_refresh_the_current_repository() {
    let (file_service, responses) =
        mock_file_service(vec![vec![file("before.rs")], vec![file("after.rs")]]);
    let mut harness = Harness::new::<Host>(
        HostProps {
            repo: Rc::from(Path::new("/repo")),
            file_service: Rc::clone(&file_service),
        },
        40,
        5,
    );
    harness.force_draw();
    receive(&mut harness, &file_service, &responses);

    file_service.fs_changed(watcher::Refresh {
        worktree: true,
        ..watcher::Refresh::default()
    });
    receive(&mut harness, &file_service, &responses);

    let screen = harness.screen().join("\n");
    assert!(screen.contains("after.rs"), "got {screen:?}");
}
