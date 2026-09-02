use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;
use std::sync::mpsc;
use std::time::Duration;

use file_types::{File, Oid, RepoPath, Revs, Stats};
use loom::testing::Harness;
use ui::Theme;
use ui::components::{Context, Explorer, ExplorerProps, Ui};
use ui::services::files::FilesService;

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
    let mut h = harness(files, width, height);
    for _ in 0..5 {
        h.force_draw();
    }
    (0..height).map(|y| h.screen_row(y)).collect()
}

pub fn mock_files_service(
    responses: Vec<Vec<File>>,
) -> (Rc<FilesService>, mpsc::Receiver<pipeline::files::Response>) {
    let (files_tx, files_responses) = mpsc::channel();
    let files_worker = pipeline::files::FilesWorker::mock(
        responses,
        channel::Emitter::new(files_tx, |response| response),
    );
    (
        Rc::new(FilesService::new(
            Rc::new(RefCell::new(files_worker)),
            Vec::new(),
        )),
        files_responses,
    )
}

pub fn harness(files: Vec<File>, width: u16, height: u16) -> Harness {
    let (files_service, files_responses) = mock_files_service(vec![files]);
    let mut harness =
        Harness::new::<Explorer>(ExplorerProps {}, width, height).provide::<Ui>(Context {
            theme: Rc::new(Theme::DARK),
            repo: Rc::from(Path::new("/repo")),
            files_service: Some(Rc::clone(&files_service)),
            ..Default::default()
        });
    harness.force_draw();
    files_service.deliver(
        files_responses
            .recv_timeout(Duration::from_secs(1))
            .expect("files response"),
    );
    harness.force_draw();
    harness
}

pub fn screen(paths: &[&str], width: u16, height: u16) -> Vec<String> {
    draw(paths.iter().map(|p| file(p)).collect(), width, height)
}
