use std::cell::Cell;
use std::rc::Rc;

use loom::testing::Harness;
use loom::{Column, Node, Scope, component, rsx, use_effect};
use ui::services::watcher::WatcherService;

#[component]
fn Subscribers(
    scope: &mut Scope,
    watcher_service: Rc<WatcherService>,
    first: Rc<Cell<u32>>,
    second: Rc<Cell<u32>>,
) -> Node {
    let watcher_service_for_first = Rc::clone(watcher_service);
    let first = Rc::clone(first);
    use_effect(scope, (), move || {
        watcher_service_for_first.changes().subscribe(move |_| {
            first.set(first.get() + 1);
        });
    });
    let watcher_service_for_second = Rc::clone(watcher_service);
    let second = Rc::clone(second);
    use_effect(scope, (), move || {
        watcher_service_for_second.changes().subscribe(move |_| {
            second.set(second.get() + 1);
        });
    });
    rsx! { Column {} }
}

#[test]
fn every_subscriber_receives_a_change() {
    let watcher_service = Rc::new(WatcherService::new());
    let first = Rc::new(Cell::new(0));
    let second = Rc::new(Cell::new(0));
    let mut harness = Harness::new::<Subscribers>(
        SubscribersProps {
            watcher_service: Rc::clone(&watcher_service),
            first: Rc::clone(&first),
            second: Rc::clone(&second),
        },
        1,
        1,
    );
    harness.force_draw();

    watcher_service.deliver(watcher::Refresh {
        worktree: true,
        ..watcher::Refresh::default()
    });

    assert_eq!(first.get(), 1);
    assert_eq!(second.get(), 1);
}
