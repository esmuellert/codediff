//! Test services that return given file lists and diffs instead of calling Git.

// Each test binary uses part of this file.
#![allow(dead_code)]
// The opened files are spelled out in full rather than hidden behind a name.
#![allow(clippy::type_complexity)]

use std::rc::Rc;
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use diff::DiffContent;
use file_types::File;
use loom::testing::Harness;
use syntax::Span;
use ui::services::{DiffService, SyntaxService, VcsService};

pub struct FakeServices {
    pub vcs_service: Rc<VcsService>,
    pub diff_service: Rc<DiffService>,
    pub syntax_service: Rc<SyntaxService>,
}

impl FakeServices {
    pub fn is_busy(&self) -> bool {
        self.vcs_service.is_busy() || self.diff_service.is_busy() || self.syntax_service.is_busy()
    }
}

/// Each listing yields the next list, then no files.
///
/// Each file reads as the first diff given for it. When several are given, each
/// read consumes one until the last, which keeps being returned.
pub fn fake_services(
    lists: Vec<Vec<File>>,
    diffs: Vec<DiffContent>,
) -> (FakeServices, Receiver<()>) {
    let (redraw_tx, redraws) = mpsc::channel();
    let mut lists = lists.into_iter();
    let mut diffs = diffs;

    let vcs_redraw = redraw_tx.clone();
    let vcs_service = Rc::new(VcsService::new(
        move || {
            let _ = vcs_redraw.send(());
        },
        move |_, _| Ok(lists.next().unwrap_or_default()),
        |_, _| Ok(Vec::new()),
    ));

    let diff_redraw = redraw_tx.clone();
    let diff_service = Rc::new(DiffService::new(
        move || {
            let _ = diff_redraw.send(());
        },
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
    ));

    let syntax_service = Rc::new(SyntaxService::start(move || {
        let _ = redraw_tx.send(());
    }));

    (
        FakeServices {
            vcs_service,
            diff_service,
            syntax_service,
        },
        redraws,
    )
}

pub fn fake_vcs_service(lists: Vec<Vec<File>>) -> (Rc<VcsService>, Receiver<()>) {
    let (redraw_tx, redraws) = mpsc::channel();
    let mut lists = lists.into_iter();
    let vcs_service = Rc::new(VcsService::new(
        move || {
            let _ = redraw_tx.send(());
        },
        move |_, _| Ok(lists.next().unwrap_or_default()),
        |_, _| Ok(Vec::new()),
    ));
    (vcs_service, redraws)
}

pub fn fake_diff_service(diffs: Vec<DiffContent>) -> (Rc<DiffService>, Receiver<()>) {
    let (redraw_tx, redraws) = mpsc::channel();
    let mut diffs = diffs;
    let diff_service = Rc::new(DiffService::new(
        move || {
            let _ = redraw_tx.send(());
        },
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
    ));
    (diff_service, redraws)
}

/// Draws until `is_busy()` is false and the last drawing saw every update.
pub fn wait_while(
    mut is_busy: impl FnMut() -> bool,
    redraws: &Receiver<()>,
    harness: &mut Harness,
) {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        let idle = !is_busy();
        while redraws.try_recv().is_ok() {}
        harness.redraw_all();
        // Effects may set state that needs further frames.
        for _ in 0..4 {
            harness.force_draw();
        }
        if idle && !is_busy() && redraws.try_recv().is_err() {
            return;
        }
        assert!(Instant::now() < deadline, "services never finished");
        let _ = redraws.recv_timeout(Duration::from_millis(20));
    }
}

pub fn wait(services: &FakeServices, redraws: &Receiver<()>, harness: &mut Harness) {
    wait_while(|| services.is_busy(), redraws, harness);
}

pub fn wait_vcs(vcs_service: &VcsService, redraws: &Receiver<()>, harness: &mut Harness) {
    wait_while(|| vcs_service.is_busy(), redraws, harness);
}

pub fn wait_diff(diff_service: &DiffService, redraws: &Receiver<()>, harness: &mut Harness) {
    wait_while(|| diff_service.is_busy(), redraws, harness);
}

/// Each content opened and diffed into its own slot.
pub fn opened(
    contents: Vec<DiffContent>,
) -> Vec<Arc<Mutex<Option<Result<Arc<DiffContent>, String>>>>> {
    let files: Vec<File> = contents
        .iter()
        .map(|content| content.file().clone())
        .collect();
    let (diff_service, redraws) = fake_diff_service(contents);
    let opened = files
        .into_iter()
        .map(|file| diff_service.open(file))
        .collect();
    while diff_service.is_busy() {
        let _ = redraws.recv_timeout(Duration::from_millis(20));
    }
    opened
}

/// The colours `SyntaxService` produces for `content`.
pub fn coloured(content: DiffContent) -> Arc<Mutex<[Vec<Vec<Span>>; 2]>> {
    let (redraw_tx, redraws) = mpsc::channel();
    let syntax_service = SyntaxService::start(move || {
        let _ = redraw_tx.send(());
    });
    let colours = syntax_service.colour(Arc::new(content));
    while syntax_service.is_busy() {
        let _ = redraws.recv_timeout(Duration::from_millis(20));
    }
    colours
}
