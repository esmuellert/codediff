//! The context value must not churn across renders.
//!
//! `App` puts an `Rc` for each field into the context. If any of
//! those are rebuilt every render, `Context::same` returns false, every
//! reader is marked dirty, and `needs_draw()` is true after every draw.
//! The loop then hits its 4-extra-draw cap every single frame.

use std::path::Path;
use std::rc::Rc;

use loom::testing::Harness;
use ui::RevisionRange;
use ui::components::{App, AppProps};
use ui::services::{DiffService, SyntaxService, VcsService, WatcherService};

fn idle_app() -> Harness {
    let vcs_service = VcsService::new(|| {}, |_, _| Ok(Vec::new()), |_, _| Ok(Vec::new()));
    let diff_service = DiffService::new(|| {}, |_| Err("no files".to_owned()));
    let syntax_service = SyntaxService::start(|| {});
    Harness::new::<App>(
        AppProps {
            cwd: Rc::from(Path::new("/tmp")),
            revision_range: RevisionRange::Worktree,
            config: Rc::new(config::Config::default()),
            keybindings: Rc::new(ui::KeyMap::default()),
            vcs_service: Rc::new(vcs_service),
            diff_service: Rc::new(diff_service),
            syntax_service: Rc::new(syntax_service),
            watcher_service: Rc::new(WatcherService::new()),
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
        "the tree still wants a draw after settling, so every frame would          redraw until the loop's bound"
    );
}
