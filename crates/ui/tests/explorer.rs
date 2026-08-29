//! The explorer, drawn into a buffer.

use std::path::Path;
use std::rc::Rc;

use file_types::{File, Oid, RepoPath, Revs};
use loom::testing::Harness;
use ui::Theme;
use ui::components::{Context, Explorer, ExplorerProps, Ui};

fn file(path: &str) -> File {
    File::unchanged_path(
        RepoPath::new(path, Path::new("/repo")),
        Revs::worktree_against(Oid::new("abc")),
    )
}

fn screen(paths: &[&str], width: u16, height: u16) -> Vec<String> {
    let files: Vec<File> = paths.iter().map(|p| file(p)).collect();
    let rows = height as u32;
    let mut harness = Harness::new::<Explorer>(
        ExplorerProps { on_open: Rc::new(|_| {}) },
        width,
        height,
    )
    .provide::<Ui>(Context {
        theme: Rc::new(Theme::DARK),
        repo: Rc::from(Path::new("/repo")),
        files: Rc::new(files),
        cursor: 0,
        view_lines: 0..rows,
        set_repo: None,
        set_cursor: None,
    });
    harness.screen()
}

#[test]
fn a_file_at_the_root_is_one_row() {
    let rows = screen(&["README.md"], 40, 2);
    assert!(rows[0].contains("README.md"), "got {:?}", rows[0]);
}

#[test]
fn files_in_a_directory_hang_below_it() {
    let rows = screen(&["src/app.rs", "src/lib.rs"], 40, 4);
    assert!(rows[0].contains("src"), "got {:?}", rows[0]);
    assert!(rows[1].contains("app.rs"), "got {:?}", rows[1]);
    assert!(rows[2].contains("lib.rs"), "got {:?}", rows[2]);
}

#[test]
fn the_last_of_its_siblings_gets_a_corner() {
    let rows = screen(&["src/app.rs", "src/lib.rs"], 40, 4);
    assert!(rows[1].contains('├'), "app.rs has a sibling below: {:?}", rows[1]);
    assert!(rows[2].contains('└'), "lib.rs is the last: {:?}", rows[2]);
}

#[test]
fn a_deeper_file_carries_its_ancestors_line() {
    let rows = screen(&["src/view/tab.rs", "notes.txt"], 40, 4);
    // src is not the last of its siblings, so the line through it continues.
    assert!(rows[2].starts_with('│'), "tab.rs sits under src: {:?}", rows[2]);
}
