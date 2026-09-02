//! The thread-local the runtime lives in while a frame is being built, and
//! the guard that puts it there.

use std::cell::RefCell;
use std::rc::Rc;

use crate::runtime::Runtime;

thread_local! {
    static CURRENT: RefCell<Option<Rc<RefCell<Runtime>>>> = const { RefCell::new(None) };
}

/// Runs `body` with `runtime` reachable from every hook, setter and ref
/// inside it.
pub(crate) fn enter<T>(runtime: &Rc<RefCell<Runtime>>, body: impl FnOnce() -> T) -> T {
    let previous = CURRENT.with(|slot| slot.borrow_mut().replace(Rc::clone(runtime)));
    let out = body();
    CURRENT.with(|slot| *slot.borrow_mut() = previous);
    out
}

/// Reads the runtime this thread is inside, if it is inside one.
pub(crate) fn with<T>(read: impl FnOnce(&Runtime) -> T) -> Option<T> {
    let runtime = CURRENT.with(|slot| slot.borrow().clone())?;
    let borrowed = runtime.try_borrow().ok()?;
    Some(read(&borrowed))
}

/// The same, with the runtime borrowed mutably.
pub(crate) fn with_mut<T>(write: impl FnOnce(&mut Runtime) -> T) -> Option<T> {
    let runtime = CURRENT.with(|slot| slot.borrow().clone())?;
    let mut borrowed = runtime.try_borrow_mut().ok()?;
    Some(write(&mut borrowed))
}

/// Whether this thread is inside a runtime at all. What P4.4 tests.
pub(crate) fn inside() -> bool {
    CURRENT.with(|slot| slot.borrow().is_some())
}

/// The runtime this thread is inside, as an owned handle.
pub(crate) fn held() -> Option<crate::reconcile::RuntimeRef> {
    CURRENT.with(|slot| slot.borrow().clone())
}

/// A handle held by a setter or a ref, so it can reach the runtime that owns
/// it from outside a frame.
pub(crate) fn handle() -> Option<std::rc::Weak<RefCell<Runtime>>> {
    CURRENT.with(|slot| slot.borrow().as_ref().map(Rc::downgrade))
}
