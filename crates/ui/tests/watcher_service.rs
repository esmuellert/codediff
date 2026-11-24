use std::cell::Cell;
use std::rc::Rc;

use loom::testing::Harness;
use loom::{Column, Node, Scope, component, rsx, use_effect};
use ui::services::WatcherService;

#[component]
fn Subscribers(
    scope: &mut Scope,
    watcher: Rc<WatcherService>,
    first: Rc<Cell<u32>>,
    second: Rc<Cell<u32>>,
) -> Node {
    let watcher_for_first = Rc::clone(watcher);
    let first = Rc::clone(first);
    use_effect(scope, (), move || {
        watcher_for_first.subscribe().subscribe(move |_| {
            first.set(first.get() + 1);
        });
    });
    let watcher_for_second = Rc::clone(watcher);
    let second = Rc::clone(second);
    use_effect(scope, (), move || {
        watcher_for_second.subscribe().subscribe(move |_| {
            second.set(second.get() + 1);
        });
    });
    rsx! { Column {} }
}

#[test]
fn every_subscriber_receives_a_change() {
    let watcher = Rc::new(WatcherService::fake());
    let first = Rc::new(Cell::new(0));
    let second = Rc::new(Cell::new(0));
    let mut harness = Harness::new::<Subscribers>(
        SubscribersProps {
            watcher: Rc::clone(&watcher),
            first: Rc::clone(&first),
            second: Rc::clone(&second),
        },
        1,
        1,
    );
    harness.force_draw();

    watcher.notify(watcher::Refresh {
        worktree: true,
        ..watcher::Refresh::default()
    });

    assert_eq!(first.get(), 1);
    assert_eq!(second.get(), 1);
}
