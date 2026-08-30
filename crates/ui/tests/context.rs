//! The context value must not churn across renders.
//!
//! `UiProvider` puts an `Rc` for each field into the context. If any of
//! those are rebuilt every render, `Context::same` returns false, every
//! reader is marked dirty, and `needs_draw()` is true after every draw.
//! The loop then hits its 4-extra-draw cap every single frame.

use std::cell::RefCell;
use std::path::Path;
use std::rc::Rc;
use std::sync::mpsc;

use loom::testing::Harness;
use ui::components::{App, AppProps};
use ui::services::diff::DiffService;
use ui::services::files::FilesService;
use ui::services::syntax::SyntaxService;

fn idle_app() -> Harness {
    let (tx, _rx) = mpsc::channel();
    let files = FilesService::new(
        Rc::new(RefCell::new(pipeline::files::FilesWorker::start(
            channel::Emitter::new(tx, |v| v),
        ))),
        Vec::new(),
    );
    let (dtx, _drx) = mpsc::channel();
    let diff = DiffService::new(Rc::new(RefCell::new(pipeline::diff::DiffWorker::start(
        channel::Emitter::new(dtx, |v| v),
    ))));
    let (stx, _srx) = mpsc::channel();
    let syntax = SyntaxService::new(Rc::new(RefCell::new(syntax::Syntax::start(
        channel::Emitter::new(stx, |v| v),
    ))));

    Harness::new::<App>(
        AppProps {
            cwd: Rc::from(Path::new("/tmp")),
            file_service: Rc::new(files),
            diff_service: Rc::new(diff),
            syntax_service: Rc::new(syntax),
        },
        80,
        24,
    )
}

#[test]
fn the_tree_settles_after_a_draw() {
    let mut h = idle_app();

    for _ in 0..5 {
        h.force_draw();
    }

    assert!(
        !h.needs_draw(),
        "the tree still wants a draw after settling, so every frame would \
         redraw until the loop's bound"
    );
}
