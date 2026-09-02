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
use ui::services::version_control::VersionControlService;

fn idle_app() -> Harness {
    let (files_tx, _files_responses) = mpsc::channel();
    let files_service = FilesService::new(
        Rc::new(RefCell::new(pipeline::files::FilesWorker::start(
            channel::Emitter::new(files_tx, |response| response),
        ))),
        Vec::new(),
    );
    let (diff_tx, _diff_responses) = mpsc::channel();
    let diff_service = DiffService::new(Rc::new(RefCell::new(pipeline::diff::DiffWorker::start(
        channel::Emitter::new(diff_tx, |response| response),
    ))));
    let (syntax_tx, _syntax_responses) = mpsc::channel();
    let syntax_service = SyntaxService::new(Rc::new(RefCell::new(syntax::Syntax::start(
        channel::Emitter::new(syntax_tx, |response| response),
    ))));

    Harness::new::<App>(
        AppProps {
            cwd: Rc::from(Path::new("/tmp")),
            files_service: Rc::new(files_service),
            diff_service: Rc::new(diff_service),
            syntax_service: Rc::new(syntax_service),
            version_control_service: Rc::new(VersionControlService::new()),
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
