use std::path::Path;
use std::rc::Rc;

use file_types::{File, Oid, RepoPath, Revs, Stats};
use loom::testing::Harness;
use ui::Theme;
use ui::components::{Context, Explorer, ExplorerProps, Ui};

pub fn file(path: &str) -> File {
    File::unchanged_path(
        RepoPath::new(path, Path::new("/repo")),
        Revs::worktree_against(Oid::new("abc")),
    )
}

pub fn file_with_stats(path: &str, added: u32, removed: u32) -> File {
    file(path).set_stats(Stats::new(added, removed))
}

pub fn moved(from: &str, to: &str) -> File {
    File::new(
        Some(RepoPath::new(from, Path::new("/repo"))),
        Some(RepoPath::new(to, Path::new("/repo"))),
        Revs::worktree_against(Oid::new("abc")),
    )
    .expect("a file on both sides")
}

pub fn draw(files: Vec<File>, width: u16, height: u16) -> Vec<String> {
    harness(files, width, height, 0).screen()
}

pub fn harness(files: Vec<File>, width: u16, height: u16, cursor: u32) -> Harness {
    let rows = height as u32;
    Harness::new::<Explorer>(ExplorerProps {}, width, height)
        .provide::<Ui>(Context {
            theme: Rc::new(Theme::DARK),
            repo: Rc::from(Path::new("/repo")),
            files: Rc::new(files),
            cursor,
            view_lines: 0..rows,
            set_repo: None,
            set_cursor: None,
            diff: None,
            file: None,
            set_file: None,
            syntax: None,
        })
}

pub fn screen(paths: &[&str], width: u16, height: u16) -> Vec<String> {
    draw(paths.iter().map(|p| file(p)).collect(), width, height + 1)
}
