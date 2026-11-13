//! A pipeline that answers from given file lists and diffs instead of Git.

// Each test binary uses part of this file.
#![allow(dead_code)]
// The opened files are spelled out in full rather than hidden behind a name.
#![allow(clippy::type_complexity)]

use std::rc::Rc;
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use diff_service::DiffService;
use diff_service::diff::DiffContent;
use file_types::File;
use loom::testing::Harness;
use syntax::Span;

/// Each listing answers with the next list, then with no files.
///
/// A file reads as the first diff given for it. When several are given, each
/// read uses up one, until the last, which keeps being returned.
pub fn fake_pipeline(
    lists: Vec<Vec<File>>,
    diffs: Vec<DiffContent>,
) -> (Rc<DiffService>, Receiver<()>) {
    let (redraw, redraws) = mpsc::channel();
    let mut lists = lists.into_iter();
    let mut diffs = diffs;
    let pipeline = DiffService::new(
        move || {
            let _ = redraw.send(());
        },
        move |_| Ok(lists.next().unwrap_or_default()),
        move |file| {
            let matching: Vec<usize> = (0..diffs.len())
                .filter(|&index| diffs[index].file() == file)
                .collect();
            match matching.as_slice() {
                [] => Err(format!("cannot read {}", file.path())),
                [only] => Ok(diffs[*only].clone()),
                [first, ..] => Ok(diffs.remove(*first)),
            }
        },
    );
    (Rc::new(pipeline), redraws)
}

/// Draws until the threads have nothing left to do and the last drawing saw
/// everything they wrote.
pub fn wait(pipeline: &DiffService, redraws: &Receiver<()>, harness: &mut Harness) {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let idle = !pipeline.is_busy();
        while redraws.try_recv().is_ok() {}
        harness.redraw_all();
        // Effects may set state that needs further frames.
        for _ in 0..4 {
            harness.force_draw();
        }
        // A job asks for a redraw before it stops counting as busy, so work
        // that finished during the drawing has left a request behind.
        if idle && !pipeline.is_busy() && redraws.try_recv().is_err() {
            return;
        }
        assert!(Instant::now() < deadline, "the pipeline never finished");
        let _ = redraws.recv_timeout(Duration::from_millis(20));
    }
}

/// Each content opened, diffed and coloured.
pub fn opened(
    contents: Vec<DiffContent>,
) -> Vec<(
    Arc<Mutex<Option<Result<Arc<DiffContent>, String>>>>,
    Arc<Mutex<[Vec<Vec<Span>>; 2]>>,
)> {
    let files: Vec<File> = contents
        .iter()
        .map(|content| content.file().clone())
        .collect();
    let (pipeline, redraws) = fake_pipeline(Vec::new(), contents);
    let opened = files.into_iter().map(|file| pipeline.open(file)).collect();
    while pipeline.is_busy() {
        let _ = redraws.recv_timeout(Duration::from_millis(20));
    }
    opened
}

/// The colours the pipeline produces for `content`.
pub fn coloured(content: DiffContent) -> Arc<Mutex<[Vec<Vec<Span>>; 2]>> {
    Arc::clone(&opened(vec![content])[0].1)
}
