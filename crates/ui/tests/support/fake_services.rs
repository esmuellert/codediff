//! Test services that return given file lists and diffs instead of calling Git.

// Each test binary uses part of this file.
#![allow(dead_code)]

use std::rc::Rc;
use std::sync::mpsc::{self, Receiver};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use diff::DiffContent;
use file_types::File;
use loom::testing::Harness;
use syntax::Span;
use ui::services::{DiffService, Resource, SyntaxService, VcsService};

pub struct FakeServices {
    pub vcs: Rc<VcsService>,
    pub diff: Rc<DiffService>,
    pub syntax: Rc<SyntaxService>,
}

impl FakeServices {
    pub fn is_busy(&self) -> bool {
        self.vcs.is_busy() || self.diff.is_busy() || self.syntax.is_busy()
    }
}

/// Each listing yields the next list, then no files.
///
/// Each file loads as the first diff given for it. When several are given, each
/// load consumes one until the last, which keeps being returned.
pub fn fake_services(
    lists: Vec<Vec<File>>,
    diffs: Vec<DiffContent>,
) -> (FakeServices, Receiver<()>) {
    let (redraw_tx, redraws) = mpsc::channel();
    let mut lists = lists.into_iter();
    let mut diffs = diffs;

    let vcs_redraw = redraw_tx.clone();
    let vcs = Rc::new(VcsService::fake(
        move || {
            let _ = vcs_redraw.send(());
        },
        move |_, _| Ok(lists.next().unwrap_or_default()),
        |_, _| Ok(Vec::new()),
    ));

    let diff_redraw = redraw_tx.clone();
    let diff = Rc::new(DiffService::fake(
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

    let syntax = Rc::new(SyntaxService::start(move || {
        let _ = redraw_tx.send(());
    }));

    (FakeServices { vcs, diff, syntax }, redraws)
}

pub fn fake_vcs(lists: Vec<Vec<File>>) -> (Rc<VcsService>, Receiver<()>) {
    let (redraw_tx, redraws) = mpsc::channel();
    let mut lists = lists.into_iter();
    let vcs = Rc::new(VcsService::fake(
        move || {
            let _ = redraw_tx.send(());
        },
        move |_, _| Ok(lists.next().unwrap_or_default()),
        |_, _| Ok(Vec::new()),
    ));
    (vcs, redraws)
}

pub fn fake_diff(diffs: Vec<DiffContent>) -> (Rc<DiffService>, Receiver<()>) {
    let (redraw_tx, redraws) = mpsc::channel();
    let mut diffs = diffs;
    let diff = Rc::new(DiffService::fake(
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
    (diff, redraws)
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

pub fn wait_vcs(vcs: &VcsService, redraws: &Receiver<()>, harness: &mut Harness) {
    wait_while(|| vcs.is_busy(), redraws, harness);
}

pub fn wait_diff(diff: &DiffService, redraws: &Receiver<()>, harness: &mut Harness) {
    wait_while(|| diff.is_busy(), redraws, harness);
}

/// Each content loaded and diffed into its own resource.
pub fn loaded(contents: Vec<DiffContent>) -> Vec<Resource<Arc<DiffContent>>> {
    let files: Vec<File> = contents
        .iter()
        .map(|content| content.file().clone())
        .collect();
    let (diff, redraws) = fake_diff(contents);
    let loaded = files.into_iter().map(|file| diff.load(file)).collect();
    while diff.is_busy() {
        let _ = redraws.recv_timeout(Duration::from_millis(20));
    }
    loaded
}

/// The spans `SyntaxService` produces for `content`.
pub fn highlighted(content: DiffContent) -> Arc<Mutex<[Vec<Vec<Span>>; 2]>> {
    let (redraw_tx, redraws) = mpsc::channel();
    let syntax = SyntaxService::start(move || {
        let _ = redraw_tx.send(());
    });
    let spans = syntax.highlight(Arc::new(content));
    while syntax.is_busy() {
        let _ = redraws.recv_timeout(Duration::from_millis(20));
    }
    spans
}
