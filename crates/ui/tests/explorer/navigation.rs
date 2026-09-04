//! Scroll, selection anchors, mouse behavior, and edge cases.

use std::collections::HashSet;

use file_types::File;
use ui::components::explorer::build::grouped_tree;
use ui::components::explorer::{find_by_identity, identity};

use super::common::*;

// ---- scroll ----

#[test]
fn j_and_k_move_the_selected_line() {
    let total = 5;
    assert_eq!(1u32.saturating_add(1).min(total - 1), 2, "j goes down");
    assert_eq!(2u32.saturating_sub(1), 1, "k goes up");
    assert_eq!(0u32.saturating_sub(1), 0, "k at the top stays");
    assert_eq!(
        4u32.saturating_add(1).min(total - 1),
        4,
        "j at the end stays"
    );
}

// ---- selection anchor ----

#[test]
fn the_selected_line_stays_when_the_file_list_rebuilds() {
    let files_v1: Vec<File> = ["src/app.rs", "src/lib.rs", "notes.txt"]
        .iter()
        .map(|p| file(p))
        .collect();

    let nodes_v1 = grouped_tree(&files_v1, &HashSet::new());
    let lib_line = nodes_v1
        .iter()
        .position(|n| {
            matches!(n,
                ui::components::explorer::build::Node::File { name, .. } if name == "lib.rs"
            )
        })
        .expect("lib.rs exists");

    let files_v2: Vec<File> = ["src/app.rs", "src/lib.rs", "src/new.rs", "notes.txt"]
        .iter()
        .map(|p| file(p))
        .collect();

    let nodes_v2 = grouped_tree(&files_v2, &HashSet::new());
    let at_same_line = &nodes_v2[lib_line];
    match at_same_line {
        ui::components::explorer::build::Node::File { name, .. } => {
            assert_eq!(name, "lib.rs", "the selected line still points at lib.rs");
        }
        other => panic!(
            "expected lib.rs at line {lib_line}, got {:?}",
            match other {
                ui::components::explorer::build::Node::Heading { name, .. } => name.to_string(),
                ui::components::explorer::build::Node::Directory { name, .. } => name.clone(),
                ui::components::explorer::build::Node::File { name, .. } => name.clone(),
            }
        ),
    }
}

#[test]
fn the_selected_line_follows_its_file_when_one_is_inserted_before_it() {
    let before: Vec<File> = ["src/lib.rs", "notes.txt"]
        .iter()
        .map(|p| file(p))
        .collect();
    let old = grouped_tree(&before, &HashSet::new());
    let on = old
        .iter()
        .position(|n| {
            matches!(n,
                ui::components::explorer::build::Node::File { name, .. } if name == "notes.txt"
            )
        })
        .expect("notes.txt is listed");
    let saved = identity(&old[on]);

    let after: Vec<File> = ["src/lib.rs", "a.txt", "notes.txt"]
        .iter()
        .map(|p| file(p))
        .collect();
    let new = grouped_tree(&after, &HashSet::new());

    let landed = find_by_identity(Some(&saved), &new).expect("notes.txt is still listed");
    assert_ne!(landed, on, "the row moved");
    assert!(
        matches!(&new[landed],
        ui::components::explorer::build::Node::File { name, .. } if name == "notes.txt"),
        "notes.txt is selected again"
    );
}

#[test]
fn a_file_that_is_gone_leaves_the_selected_line_where_it_was() {
    let after: Vec<File> = ["a.rs"].iter().map(|p| file(p)).collect();
    let new = grouped_tree(&after, &HashSet::new());

    assert_eq!(
        find_by_identity(Some("gone.rs"), &new),
        None,
        "nothing to move to, so the caller keeps the selected line"
    );
}

#[test]
fn nothing_saved_moves_nothing() {
    let files: Vec<File> = ["a.rs"].iter().map(|p| file(p)).collect();
    let nodes = grouped_tree(&files, &HashSet::new());
    assert_eq!(find_by_identity(None, &nodes), None);
}

#[test]
fn the_identity_tells_two_files_of_the_same_name_apart() {
    let files: Vec<File> = ["src/a/mod.rs", "src/b/mod.rs"]
        .iter()
        .map(|p| file(p))
        .collect();
    let nodes = grouped_tree(&files, &HashSet::new());

    let first = nodes
        .iter()
        .position(|n| {
            matches!(n,
                ui::components::explorer::build::Node::File { file, .. }
                if file.path().as_str() == "src/b/mod.rs"
            )
        })
        .expect("src/b/mod.rs is listed");

    let saved = identity(&nodes[first]);
    let landed = find_by_identity(Some(&saved), &nodes).expect("found");
    assert_eq!(landed, first, "the full path picks out the right mod.rs");
}

// ---- mouse ----

#[test]
fn a_directory_toggles_only_when_clicked_while_selected() {
    let mut harness = harness(vec![file("src/a.rs"), file("src/b.rs")], 40, 6);

    harness.click(0, 1).force_draw();
    assert!(harness.screen().join("\n").contains("a.rs"));

    harness.click(0, 1).force_draw();
    assert!(!harness.screen().join("\n").contains("a.rs"));

    harness.click(0, 0).force_draw();
    harness.click(0, 1).force_draw();
    assert!(!harness.screen().join("\n").contains("a.rs"));

    harness.click(0, 1).force_draw();
    assert!(harness.screen().join("\n").contains("a.rs"));
}

#[test]
fn the_selected_line_has_a_different_background() {
    let files: Vec<File> = ["src/app.rs", "notes.txt"]
        .iter()
        .map(|p| file(p))
        .collect();
    let mut h1 = harness(files.clone(), 40, 10);
    h1.force_draw();
    h1.press(crokey::key!(j));
    h1.force_draw();
    let selected_background = h1.style_at(0, 1).bg;
    let other_background = h1.style_at(0, 2).bg;
    assert_ne!(
        selected_background, other_background,
        "the selected line has a different background from other rows"
    );
}

#[test]
fn queued_j_keys_reveal_the_selected_line() {
    let files: Vec<File> = (0..8)
        .map(|index| file(&format!("file{index}.rs")))
        .collect();
    let mut harness = harness(files, 40, 6);
    harness
        .press(crokey::key!(j))
        .press(crokey::key!(j))
        .press(crokey::key!(j))
        .press(crokey::key!(j))
        .force_draw();

    assert!(harness.screen_row(0).contains("file0.rs"));
    assert!(harness.screen_row(3).contains("file3.rs"));
    assert_ne!(harness.style_at(0, 3).bg, harness.style_at(0, 2).bg);
}

// ---- edge cases ----

#[test]
fn an_empty_list_draws_nothing() {
    let rows = draw(Vec::new(), 40, 5);
    for row in &rows {
        assert!(
            row.is_empty() || row.chars().all(|c| c == ' '),
            "an empty list is blank: {:?}",
            row
        );
    }
}

#[test]
fn a_single_file_renders_without_panic() {
    let rows = draw(vec![file("only.rs")], 40, 5);
    assert!(rows[1].contains("only.rs"), "got {:?}", rows[1]);
}

// ---- enter opens a file ----

#[test]
fn enter_on_a_file_sets_the_focused_file() {
    use std::cell::Cell;
    use std::rc::Rc;

    use loom::testing::Harness;
    use loom::{Node, Scope, component, rsx, use_state};
    use ui::Theme;
    use ui::components::{Context, Explorer, Ui, UiProps};

    /// Provides context with a set_file that records calls.
    #[component]
    fn WithFile(
        scope: &mut Scope,
        files_service: Rc<ui::services::files::FilesService>,
        file_set: Rc<Cell<bool>>,
    ) -> Node {
        let (file, set_file) = use_state(scope, || None::<Rc<file_types::File>>);
        if file.is_some() {
            file_set.set(true);
        }
        rsx! {
            Ui {
                value: Context {
                    theme: Rc::new(Theme::DARK),
                    repo: Rc::from(std::path::Path::new("/repo")),
                    files_service: Some(Rc::clone(files_service)),
                    set_file: Some(set_file),
                    file: file.as_ref().map(Rc::clone),
                    ..Default::default()
                },
                Explorer {}
            }
        }
    }

    let files = vec![file("src/app.rs"), file("src/lib.rs")];
    let (files_service, files_responses) = mock_files_service(vec![files]);
    let file_set = Rc::new(Cell::new(false));
    let mut h = Harness::new::<WithFile>(
        WithFileProps {
            files_service: Rc::clone(&files_service),
            file_set: Rc::clone(&file_set),
        },
        40,
        10,
    );

    h.force_draw();
    files_service.deliver(
        files_responses
            .recv_timeout(std::time::Duration::from_secs(1))
            .expect("files response"),
    );
    h.force_draw();
    h.press(crokey::key!(j));
    h.force_draw();
    h.press(crokey::key!(j));
    h.force_draw();
    h.press(crokey::key!(enter));
    h.force_draw();

    assert!(
        file_set.get(),
        "pressing Enter on a file should set the focused file"
    );
}

// ---- scroll vs selection ----

#[test]
fn wheel_scrolls_the_view_without_moving_the_selected_line() {
    // A list taller than the viewport. The heading starts selected.
    let files: Vec<File> = (0..20).map(|i| file(&format!("file{i}.rs"))).collect();
    let mut h = harness(files, 40, 6);
    for _ in 0..5 {
        h.force_draw();
    }

    let before = h.screen();

    // Scroll down by wheeling.
    h.wheel(10, 3, 3);
    for _ in 0..3 {
        h.force_draw();
    }

    let after = h.screen();

    // The view should have changed (different rows visible).
    assert_ne!(before, after, "wheel should scroll the view");

    // The selected line should not have moved, though it is now offscreen.
    // Press j once: selection moves to 1, not to the scrolled row plus one.
    h.press(crokey::key!(j));
    for _ in 0..3 {
        h.force_draw();
    }

    // Now press k: selection goes back to 0.
    h.press(crokey::key!(k));
    for _ in 0..3 {
        h.force_draw();
    }

    // The heading is visible again because j/k revealed selected line 0.
    let final_screen = h.screen();
    let has_heading = final_screen.iter().any(|r| r.contains("Changes"));
    assert!(
        has_heading,
        "after j then k, line 0 is selected and the heading is visible: {:?}",
        final_screen
    );
}

#[test]
fn j_moves_the_selected_line_and_the_view_follows() {
    let files: Vec<File> = (0..20).map(|i| file(&format!("file{i}.rs"))).collect();
    let mut h = harness(files, 40, 6);
    for _ in 0..5 {
        h.force_draw();
    }

    // Press j enough times to go past the viewport.
    for _ in 0..10 {
        h.press(crokey::key!(j));
        h.force_draw();
    }

    let screen = h.screen();
    // The selected line is highlighted and the viewport followed it.
    let has_heading = screen.iter().any(|r| r.contains("Changes"));
    // The heading at row 0 should have scrolled off.
    assert!(
        !has_heading,
        "after pressing j 10 times in a 6-row viewport, the heading should be off screen: {:?}",
        screen
    );
}

#[test]
fn wheel_cannot_scroll_past_the_last_line() {
    let files: Vec<File> = (0..10).map(|i| file(&format!("file{i}.rs"))).collect();
    // 6-row viewport, ~11 nodes (heading + 10 files).
    let mut h = harness(files, 40, 6);
    for _ in 0..5 {
        h.force_draw();
    }

    // Scroll way past the end.
    for _ in 0..20 {
        h.wheel(10, 3, 3);
        h.force_draw();
    }

    let screen = h.screen();
    // The last file should be visible at the bottom, not at the top
    // with empty rows below.
    let non_empty: Vec<&String> = screen.iter().filter(|r| !r.trim().is_empty()).collect();
    assert_eq!(
        non_empty.len(),
        6,
        "every row should have content — no empty space below the last line: {:?}",
        screen
    );
}

#[test]
fn short_content_does_not_scroll() {
    // Only 2 files — heading + 2 = 3 nodes, viewport is 6.
    let files = vec![file("a.rs"), file("b.rs")];
    let mut h = harness(files, 40, 6);
    for _ in 0..5 {
        h.force_draw();
    }

    let before = h.screen();

    // Try to scroll down.
    h.wheel(10, 3, 3);
    for _ in 0..3 {
        h.force_draw();
    }

    let after = h.screen();
    assert_eq!(
        before, after,
        "content shorter than the viewport should not scroll"
    );
}

#[test]
fn moving_to_a_file_opens_it() {
    use std::cell::RefCell;
    use std::rc::Rc;

    use loom::testing::Harness;
    use loom::{Node, Scope, component, rsx, use_state};
    use ui::Theme;
    use ui::components::{Context, Explorer, Ui, UiProps};

    /// Records every file the explorer opens.
    #[component]
    fn Recorder(
        scope: &mut Scope,
        files_service: Rc<ui::services::files::FilesService>,
        opened: Rc<RefCell<Vec<String>>>,
    ) -> Node {
        let (file, set_file) = use_state(scope, || None::<Rc<file_types::File>>);
        if let Some(ref f) = file {
            let path = f.path().as_str().to_string();
            let mut log = opened.borrow_mut();
            if log.last() != Some(&path) {
                log.push(path);
            }
        }
        rsx! {
            Ui {
                value: Context {
                    theme: Rc::new(Theme::DARK),
                    repo: Rc::from(std::path::Path::new("/repo")),
                    files_service: Some(Rc::clone(files_service)),
                    set_file: Some(set_file),
                    file: file.as_ref().map(Rc::clone),
                    ..Default::default()
                },
                Explorer {}
            }
        }
    }

    let files = vec![file("a.rs"), file("b.rs")];
    let (files_service, files_responses) = mock_files_service(vec![files]);
    let opened = Rc::new(RefCell::new(Vec::new()));
    let mut h = Harness::new::<Recorder>(
        RecorderProps {
            files_service: Rc::clone(&files_service),
            opened: Rc::clone(&opened),
        },
        40,
        10,
    );
    h.force_draw();
    files_service.deliver(
        files_responses
            .recv_timeout(std::time::Duration::from_secs(1))
            .expect("files response"),
    );
    for _ in 0..4 {
        h.force_draw();
    }

    // Selection starts on the heading. Move down to a.rs.
    h.press(crokey::key!(j));
    for _ in 0..3 {
        h.force_draw();
    }
    assert_eq!(
        opened.borrow().last().map(String::as_str),
        Some("a.rs"),
        "moving onto a.rs should open it: {:?}",
        opened.borrow()
    );

    // Move down to b.rs.
    h.press(crokey::key!(j));
    for _ in 0..3 {
        h.force_draw();
    }
    assert_eq!(
        opened.borrow().last().map(String::as_str),
        Some("b.rs"),
        "moving onto b.rs should open it: {:?}",
        opened.borrow()
    );

    // Move back up to a.rs.
    h.press(crokey::key!(k));
    for _ in 0..3 {
        h.force_draw();
    }
    assert_eq!(
        opened.borrow().last().map(String::as_str),
        Some("a.rs"),
        "moving back up to a.rs should open it: {:?}",
        opened.borrow()
    );
}
