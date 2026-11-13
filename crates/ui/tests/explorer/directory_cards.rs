use std::path::Path;
use std::rc::Rc;
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex};

use file_types::{File, Oid, RepoPath, Revs, Stats};
use loom::testing::Harness;
use loom::{Basis, Layout, Row, RowProps, Scope, component, rsx, use_state};
use pipeline::Pipeline;
use ui::Theme;
use ui::components::border::{Border, BorderProps};
use ui::components::diff_viewer::DiffViewer;
use ui::components::explorer::Explorer;
use ui::components::{Context, DirectorySelection, Ui, UiProps};

use super::common::{fake_pipeline, wait};

fn file(path: &str) -> File {
    File::unchanged_path(
        RepoPath::new(path, Path::new("/repo")),
        Revs::worktree_against(Oid::new("base")),
    )
    .set_stats(Stats::new(1, 1))
}

fn diff(file: File, original: &str, modified: &str) -> pipeline::diff::DiffContent {
    let original: Vec<_> = original.lines().collect();
    let modified: Vec<_> = modified.lines().collect();
    let changed =
        pipeline::diff::compute(&original, &modified, pipeline::diff::Settings::default())
            .expect("diff computes");
    let alignment = pipeline::diff::align(changed, &original, &modified).expect("alignment");
    pipeline::diff::DiffContent::Diff(pipeline::diff::Diff { file, alignment })
}

#[component]
fn SelectedDiffHost(_scope: &mut Scope, context: Context) -> loom::Node {
    rsx! {
        Ui {
            value: context.clone(),
            DiffViewer {}
        }
    }
}

#[component]
fn ExplorerDiffHost(scope: &mut Scope, pipeline: Rc<Pipeline>) -> loom::Node {
    let (file, set_file) = use_state(scope, || None::<Rc<File>>);
    let (directory, set_directory) = use_state(scope, || None::<Rc<DirectorySelection>>);
    rsx! {
        Ui {
            value: Context {
                theme: Rc::new(Theme::DARK),
                repo: Rc::from(Path::new("/repo")),
                file,
                set_file: Some(set_file),
                directory,
                set_directory: Some(set_directory),
                diff_service: Some(Rc::clone(pipeline)),
                ..Context::default()
            },
            Row {
                layout: Layout { grow: 1, ..Default::default() },
                ..,
                Border {
                    layout: Layout { basis: Basis::Length(32), shrink: 1, ..Default::default() },
                    Explorer {}
                }
                Border {
                    layout: Layout { grow: 1, ..Default::default() },
                    DiffViewer {}
                }
            }
        }
    }
}

fn setup(line_count: usize) -> (Harness, Rc<Pipeline>, Receiver<()>) {
    let first = file("src/a.rs");
    let second = file("src/b.rs");
    let added = File::added(
        RepoPath::new("src/new.rs", Path::new("/repo")),
        Revs::worktree_against(Oid::new("base")),
    );
    let outside = file("z-readme.rs");
    let files = vec![first.clone(), second.clone(), added, outside];
    let text = |marker: &str| {
        (0..line_count)
            .map(|line| format!("{marker}_{line:02}"))
            .collect::<Vec<_>>()
            .join("\n")
    };
    let (pipeline, redraws) = fake_pipeline(
        vec![files],
        vec![
            diff(first, &text("old_a_marker"), &text("new_a_marker")),
            diff(second, &text("old_b_marker"), &text("new_b_marker")),
        ],
    );
    let mut harness = Harness::new::<ExplorerDiffHost>(
        ExplorerDiffHostProps {
            pipeline: Rc::clone(&pipeline),
        },
        100,
        18,
    );
    wait(&pipeline, &redraws, &mut harness);
    (harness, pipeline, redraws)
}

#[test]
fn folding_a_selected_directory_keeps_its_loaded_cards_and_scroll_position() {
    let (mut harness, pipeline, redraws) = setup(32);
    let directory_row = harness
        .screen()
        .iter()
        .position(|line| line.chars().take(32).collect::<String>().contains("src"))
        .expect("src directory");
    harness.click(2, directory_row as u16).force_draw();
    wait(&pipeline, &redraws, &mut harness);
    let initial = harness.screen();
    harness.wheel(80, 6, 1).force_draw();
    assert_ne!(
        harness.screen(),
        initial,
        "the cards must be scrolled before folding"
    );
    let expected = harness.cells().clone();
    let area = harness.area_of("DiffViewer").expect("diff viewer area");
    for mouse in [true, false] {
        for expanded in [false, true] {
            if mouse {
                harness.click(2, directory_row as u16).force_draw();
            } else {
                harness.press(loom::crokey::key!(enter)).force_draw();
            }
            let screen = harness.screen();
            let explorer = screen
                .iter()
                .map(|line| line.chars().take(32).collect::<String>())
                .collect::<Vec<_>>()
                .join("\n");
            assert_eq!(
                explorer.contains("a.rs"),
                expanded,
                "directory did not toggle"
            );
            assert!(
                !screen.join("\n").contains("Loading directory diffs…"),
                "folding reloaded the cards"
            );
            let actual = harness.cells().clone();
            for y in area.y..area.bottom() {
                for x in area.x..area.right() {
                    assert_eq!(
                        actual.cell((x, y)),
                        expected.cell((x, y)),
                        "card cell changed at {x},{y} after folding"
                    );
                }
            }
        }
    }
    assert!(!pipeline.is_busy(), "folding opened the files again");
}

#[test]
fn changing_directories_clears_old_cards_and_resets_scrolling() {
    let first = file("first/a.rs");
    let second = file("second/b.rs");
    let content = |file: &File, marker: &str| {
        let original = (0..32)
            .map(|line| format!("old_{marker}_{line:02}"))
            .collect::<Vec<_>>()
            .join("\n");
        let modified = (0..32)
            .map(|line| format!("new_{marker}_{line:02}"))
            .collect::<Vec<_>>()
            .join("\n");
        diff(file.clone(), &original, &modified)
    };
    let (pipeline, redraws) = fake_pipeline(
        Vec::new(),
        vec![content(&first, "a"), content(&second, "b")],
    );
    let directory = |path: &str, file: &File| {
        Rc::new(DirectorySelection {
            path: RepoPath::new(path, Path::new("/repo")),
            revs: file.revs().clone(),
            files: Rc::new(vec![file.clone()]),
        })
    };
    let first_context = Context {
        directory: Some(directory("first", &first)),
        diff_service: Some(Rc::clone(&pipeline)),
        ..Context::default()
    };
    let second_context = Context {
        directory: Some(directory("second", &second)),
        ..first_context.clone()
    };
    let mut harness = Harness::new::<SelectedDiffHost>(
        SelectedDiffHostProps {
            context: first_context.clone(),
        },
        80,
        10,
    );
    wait(&pipeline, &redraws, &mut harness);
    assert!(harness.screen().join("\n").contains("first/a.rs"));
    harness.wheel(50, 5, 1).force_draw();
    assert!(!harness.screen().join("\n").contains("first/a.rs"));

    harness
        .set_props::<SelectedDiffHost>(SelectedDiffHostProps {
            context: second_context.clone(),
        })
        .force_draw();
    let text = harness.screen().join("\n");
    assert!(!text.contains("new_a_"), "old cards stayed: {text}");
    wait(&pipeline, &redraws, &mut harness);
    let second_screen = harness.screen();
    let text = second_screen.join("\n");
    assert!(
        text.contains("second/b.rs") && text.contains("new_b_00"),
        "{text}"
    );
    assert!(!text.contains("new_a_"), "{text}");

    harness.set_props::<SelectedDiffHost>(SelectedDiffHostProps {
        context: Context {
            directory: None,
            file: Some(Rc::new(first.clone())),
            ..first_context
        },
    });
    wait(&pipeline, &redraws, &mut harness);
    assert!(harness.screen().join("\n").contains("new_a_00"));
    harness.set_props::<SelectedDiffHost>(SelectedDiffHostProps {
        context: second_context,
    });
    wait(&pipeline, &redraws, &mut harness);
    assert_eq!(harness.screen(), second_screen);
}

#[test]
fn selecting_a_directory_shows_its_two_sided_diffs_and_a_file_keeps_the_regular_viewer() {
    let (mut harness, pipeline, redraws) = setup(1);

    harness.press(loom::crokey::key!(j)).force_draw();
    wait(&pipeline, &redraws, &mut harness);
    let cards = harness
        .screen()
        .iter()
        .map(|line| line.chars().skip(35).collect::<String>())
        .collect::<Vec<_>>()
        .join("\n");
    assert!(cards.contains("src/a.rs"), "{cards}");
    assert!(cards.contains("src/b.rs"), "{cards}");
    assert!(cards.contains("new_a_marker"), "{cards}");
    assert!(
        !cards.contains("src/new.rs"),
        "one-sided files are not cards: {cards}"
    );
    assert!(
        !cards.contains("z-readme.rs"),
        "files outside the directory are omitted: {cards}"
    );

    let selected_file_row = harness
        .screen()
        .iter()
        .position(|line| line.chars().take(32).collect::<String>().contains("a.rs"))
        .expect("Explorer lists src/a.rs");
    harness.click(2, selected_file_row as u16).force_draw();
    wait(&pipeline, &redraws, &mut harness);

    let file_view = harness.screen().join("\n");
    assert!(file_view.contains("new_a_marker"), "{file_view}");
    assert!(
        !file_view.contains("new_b_marker"),
        "directory cards remain visible: {file_view}"
    );
}

/// The paths a pipeline has read, in order.
type Reads = Arc<Mutex<Vec<String>>>;

/// A directory of `count` files with `lines` changed lines each, shown in a
/// viewer of `height` rows. Returns the paths read so far.
fn directory_viewer(
    count: usize,
    lines: usize,
    height: u16,
) -> (Harness, Rc<Pipeline>, Receiver<()>, Reads) {
    let files: Vec<File> = (0..count)
        .map(|index| file(&format!("dir/{index}.rs")))
        .collect();
    let text = |marker: &str| {
        (0..lines)
            .map(|line| format!("{marker}_{line:02}"))
            .collect::<Vec<_>>()
            .join("\n")
    };
    let contents: Vec<_> = files
        .iter()
        .map(|file| diff(file.clone(), &text("old"), &text("new")))
        .collect();
    let reads = Arc::new(Mutex::new(Vec::new()));
    let read_paths = Arc::clone(&reads);
    let (redraw, redraws) = mpsc::channel();
    let pipeline = Rc::new(Pipeline::new(
        move || {
            let _ = redraw.send(());
        },
        |_| Ok(Vec::new()),
        move |file| {
            read_paths
                .lock()
                .unwrap()
                .push(file.path().as_str().to_owned());
            contents
                .iter()
                .find(|content| content.file() == file)
                .cloned()
                .ok_or_else(|| format!("cannot read {}", file.path()))
        },
    ));
    let context = Context {
        directory: Some(Rc::new(DirectorySelection {
            path: RepoPath::new("dir", Path::new("/repo")),
            revs: files[0].revs().clone(),
            files: Rc::new(files),
        })),
        diff_service: Some(Rc::clone(&pipeline)),
        ..Context::default()
    };
    let mut harness =
        Harness::new::<SelectedDiffHost>(SelectedDiffHostProps { context }, 80, height);
    wait(&pipeline, &redraws, &mut harness);
    (harness, pipeline, redraws, reads)
}

#[test]
fn a_file_is_read_only_when_the_end_of_the_cards_comes_into_view() {
    let (mut harness, pipeline, redraws, reads) = directory_viewer(3, 20, 10);
    assert_eq!(*reads.lock().unwrap(), ["dir/0.rs"]);

    // Scroll near the end of the first card; the end line is still below.
    for _ in 0..3 {
        harness.wheel(40, 5, 1).force_draw();
    }
    wait(&pipeline, &redraws, &mut harness);
    assert_eq!(*reads.lock().unwrap(), ["dir/0.rs"]);

    for _ in 0..2 {
        harness.wheel(40, 5, 1).force_draw();
    }
    let before = harness.screen();
    wait(&pipeline, &redraws, &mut harness);
    assert_eq!(*reads.lock().unwrap(), ["dir/0.rs", "dir/1.rs"]);
    let after = harness.screen();
    assert_eq!(
        after[..before.len() - 1],
        before[..before.len() - 1],
        "the appended card moved what was on screen"
    );
}

#[test]
fn short_files_keep_loading_until_the_view_is_full() {
    let (mut harness, _pipeline, _redraws, reads) = directory_viewer(3, 1, 30);
    assert_eq!(*reads.lock().unwrap(), ["dir/0.rs", "dir/1.rs", "dir/2.rs"]);
    let screen = harness.screen().join("\n");
    for index in 0..3 {
        assert!(screen.contains(&format!("dir/{index}.rs")), "{screen}");
    }
    assert!(!screen.contains("Loading directory diffs…"), "{screen}");
}

#[test]
fn a_file_that_cannot_be_read_is_reported_and_the_next_one_still_loads() {
    let good = file("dir/good.rs");
    let bad = file("dir/bad.rs");
    let (pipeline, redraws) = fake_pipeline(Vec::new(), vec![diff(good.clone(), "a", "b")]);
    let context = Context {
        directory: Some(Rc::new(DirectorySelection {
            path: RepoPath::new("dir", Path::new("/repo")),
            revs: good.revs().clone(),
            files: Rc::new(vec![bad, good]),
        })),
        diff_service: Some(Rc::clone(&pipeline)),
        ..Context::default()
    };
    let mut harness = Harness::new::<SelectedDiffHost>(SelectedDiffHostProps { context }, 80, 20);
    wait(&pipeline, &redraws, &mut harness);

    let screen = harness.screen().join("\n");
    assert!(screen.contains("cannot read dir/bad.rs"), "{screen}");
    assert!(screen.contains("dir/good.rs"), "{screen}");
}

#[test]
fn only_one_file_is_read_at_a_time_while_the_end_line_comes_and_goes() {
    let files: Vec<File> = (0..3)
        .map(|index| file(&format!("dir/{index}.rs")))
        .collect();
    let text = |marker: &str| {
        (0..20)
            .map(|line| format!("{marker}_{line:02}"))
            .collect::<Vec<_>>()
            .join("\n")
    };
    let contents: Vec<_> = files
        .iter()
        .map(|file| diff(file.clone(), &text("old"), &text("new")))
        .collect();
    let reads = Arc::new(Mutex::new(Vec::new()));
    let read_paths = Arc::clone(&reads);
    let (release, released) = mpsc::channel::<()>();
    let (redraw, redraws) = mpsc::channel();
    let pipeline = Rc::new(Pipeline::new(
        move || {
            let _ = redraw.send(());
        },
        |_| Ok(Vec::new()),
        move |file| {
            read_paths
                .lock()
                .unwrap()
                .push(file.path().as_str().to_owned());
            if file.path().as_str() == "dir/1.rs" {
                released.recv().unwrap();
            }
            Ok(contents
                .iter()
                .find(|content| content.file() == file)
                .cloned()
                .unwrap())
        },
    ));
    let context = Context {
        directory: Some(Rc::new(DirectorySelection {
            path: RepoPath::new("dir", Path::new("/repo")),
            revs: files[0].revs().clone(),
            files: Rc::new(files),
        })),
        diff_service: Some(Rc::clone(&pipeline)),
        ..Context::default()
    };
    let mut harness = Harness::new::<SelectedDiffHost>(SelectedDiffHostProps { context }, 80, 10);
    wait(&pipeline, &redraws, &mut harness);
    for _ in 0..5 {
        harness.wheel(40, 5, 1).force_draw();
    }
    // The second file is being read. Move the end line out of view and back.
    for lines in [-1, -1, 1, 1] {
        harness.wheel(40, 5, lines).force_draw();
    }
    release.send(()).unwrap();
    wait(&pipeline, &redraws, &mut harness);

    // The second card pushed the end line out of view, so nothing else is read.
    assert_eq!(*reads.lock().unwrap(), ["dir/0.rs", "dir/1.rs"]);
}
