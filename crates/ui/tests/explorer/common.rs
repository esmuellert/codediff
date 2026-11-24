use std::path::Path;
use std::rc::Rc;

use file_types::{CommitHash, File, RepoPath, Revs, Stats};
use loom::testing::Harness;
use ui::Theme;
use ui::components::{Context, Explorer, ExplorerProps, Ui};

pub use crate::fake_services::{
    FakeServices, fake_diff_service, fake_services, fake_vcs_service, wait, wait_diff, wait_vcs,
};

pub fn file(path: &str) -> File {
    File::unchanged_path(
        RepoPath::new(path, Path::new("/repo")),
        Revs::worktree_against(CommitHash::new("abc")),
    )
}

pub fn file_with_stats(path: &str, added: u32, removed: u32) -> File {
    file(path).set_stats(Stats::new(added, removed))
}

pub fn moved(from: &str, to: &str) -> File {
    File::new(
        Some(RepoPath::new(from, Path::new("/repo"))),
        Some(RepoPath::new(to, Path::new("/repo"))),
        Revs::worktree_against(CommitHash::new("abc")),
    )
    .expect("a file on both sides")
}

pub fn draw(files: Vec<File>, width: u16, height: u16) -> Vec<String> {
    let mut h = harness(files, width, height);
    for _ in 0..5 {
        h.force_draw();
    }
    (0..height).map(|y| h.screen_line(y)).collect()
}

pub fn harness(files: Vec<File>, width: u16, height: u16) -> Harness {
    let (vcs_service, redraws) = fake_vcs_service(vec![files]);
    let mut harness =
        Harness::new::<Explorer>(ExplorerProps {}, width, height).provide::<Ui>(Context {
            theme: Rc::new(Theme::DARK),
            repo: Rc::from(Path::new("/repo")),
            vcs_service: Some(Rc::clone(&vcs_service)),
            ..Default::default()
        });
    wait_vcs(&vcs_service, &redraws, &mut harness);
    harness
}

pub fn screen(paths: &[&str], width: u16, height: u16) -> Vec<String> {
    draw(paths.iter().map(|p| file(p)).collect(), width, height)
}
