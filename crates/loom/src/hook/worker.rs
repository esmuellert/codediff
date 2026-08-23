//! Addresses a worker answers to.

use std::cell::RefCell;
use std::rc::{Rc, Weak};

use crate::runtime::Runtime;
use crate::scope::ScopeId;

/// The address an answer is delivered to: which scope, which effect slot, and
/// which run of that effect.
#[derive(Clone, Copy)]
struct Address {
    scope: ScopeId,
    slot: u16,
    generation: u64,
}

impl Address {
    /// Whether the effect that opened this address is still the current one.
    fn wanted(self, runtime: &Weak<RefCell<Runtime>>) -> bool {
        let Some(runtime) = runtime.upgrade() else { return false };
        let Ok(rt) = runtime.try_borrow() else { return false };
        let Some(hooks) = rt.hooks.get(&self.scope) else { return false };
        match hooks.slots.get(self.slot as usize) {
            Some(crate::hook::Slot::Effect(e) | crate::hook::Slot::LayoutEffect(e)) => {
                e.generation == self.generation
            }
            _ => false,
        }
    }
}

/// The answer to one request, arriving later.
#[must_use = "a promise with no `then` throws its answer away"]
pub struct Promise<T: 'static> {
    shared: Rc<RefCell<Option<Box<dyn FnOnce(T)>>>>,
}

impl<T: 'static> Promise<T> {
    /// Runs `take` when the answer arrives, with the owning scope entered.
    pub fn then(self, take: impl FnOnce(T) + 'static) {
        *self.shared.borrow_mut() = Some(Box::new(take));
    }
}

/// The answering end, kept by whoever sent the request.
///
/// Carries the owning scope, the effect's slot and the effect's generation, so
/// an answer that arrives after the deps changed or the component went away is
/// refused rather than applied.
pub struct Resolver<T: 'static> {
    address: Address,
    runtime: Weak<RefCell<Runtime>>,
    shared: Rc<RefCell<Option<Box<dyn FnOnce(T)>>>>,
}

impl<T: 'static> Resolver<T> {
    /// Delivers. Returns whether it was taken.
    pub fn resolve(self, value: T) -> bool {
        if !self.is_wanted() {
            return false;
        }
        let Some(take) = self.shared.borrow_mut().take() else { return false };
        let Some(runtime) = self.runtime.upgrade() else { return false };
        crate::current::enter(&runtime, || take(value));
        true
    }

    pub fn is_wanted(&self) -> bool {
        self.address.wanted(&self.runtime)
    }
}

/// Answers that keep coming, for a worker that replies in pieces.
#[must_use = "an observable with no `subscribe` throws its answers away"]
pub struct Observable<T: 'static> {
    shared: Rc<RefCell<Option<Box<dyn FnMut(T)>>>>,
}

impl<T: 'static> Observable<T> {
    /// Runs `take` on every piece.
    pub fn subscribe(self, take: impl FnMut(T) + 'static) {
        *self.shared.borrow_mut() = Some(Box::new(take));
    }
}

/// The delivering end of an `Observable`.
pub struct Observer<T: 'static> {
    address: Address,
    runtime: Weak<RefCell<Runtime>>,
    shared: Rc<RefCell<Option<Box<dyn FnMut(T)>>>>,
}

impl<T: 'static> Clone for Observer<T> {
    fn clone(&self) -> Self {
        Self {
            address: self.address,
            runtime: self.runtime.clone(),
            shared: Rc::clone(&self.shared),
        }
    }
}

impl<T: 'static> Observer<T> {
    /// Delivers one piece. Returns whether it was taken.
    pub fn next(&self, value: T) -> bool {
        if !self.is_wanted() {
            return false;
        }
        let Some(runtime) = self.runtime.upgrade() else { return false };
        let mut held = self.shared.borrow_mut();
        let Some(take) = held.as_mut() else { return false };
        crate::current::enter(&runtime, || take(value));
        true
    }

    pub fn is_wanted(&self) -> bool {
        self.address.wanted(&self.runtime)
    }

    /// No more pieces, for every clone of this observer.
    pub fn complete(self) {
        *self.shared.borrow_mut() = None;
    }
}

/// Opens a one-shot address: the resolver the answerer keeps, and the promise
/// the asker attaches a handler to.
///
/// Legal inside an effect body, where the runtime knows which slot is asking.
pub fn promise<T: 'static>() -> (Resolver<T>, Promise<T>) {
    let (address, runtime) = open();
    let shared = Rc::new(RefCell::new(None));
    (
        Resolver { address, runtime, shared: Rc::clone(&shared) },
        Promise { shared },
    )
}

/// Opens a many-shot address. Same pair, same rule.
pub fn observable<T: 'static>() -> (Observer<T>, Observable<T>) {
    let (address, runtime) = open();
    let shared = Rc::new(RefCell::new(None));
    (
        Observer { address, runtime, shared: Rc::clone(&shared) },
        Observable { shared },
    )
}

/// P4.6 — which effect is running, and the runtime to reach back into.
fn open() -> (Address, Weak<RefCell<Runtime>>) {
    let running = crate::current::with(|rt| rt.running_effect).flatten();
    let Some((scope, slot, generation)) = running else {
        panic!("promise and observable open an address for the effect that is running, and no effect is");
    };
    let runtime = crate::current::handle().expect("an effect runs inside a runtime");
    (Address { scope, slot, generation }, runtime)
}
