//! The context value must not churn across renders.
//!
//! `UiProvider` puts an `Rc` for each field into the context. If any of
//! those are rebuilt every render, `Context::same` returns false, every
//! reader is marked dirty, and `needs_draw()` is true after every draw.
//! The loop then hits its 4-extra-draw cap every single frame.

use std::path::Path;
use std::rc::Rc;

use loom::testing::Harness;
use pipeline::Pipeline;
use ui::components::{App, AppProps};
use ui::services::version_control::VersionControlService;
use ui::services::watcher::WatcherService;

fn idle_app() -> Harness {
    let pipeline = Pipeline::new(|| {}, |_| Ok(Vec::new()), |_| Err("no files".to_owned()));
    Harness::new::<App>(
        AppProps {
            cwd: Rc::from(Path::new("/tmp")),
            config: Rc::new(config::Config::default()),
            keybindings: Rc::new(ui::KeyMap::default()),
            diff_service: Rc::new(pipeline),
            version_control_service: Rc::new(VersionControlService::new()),
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
        "the tree still wants a draw after settling, so every frame would \
         redraw until the loop's bound"
    );
}
