//! External store subscriptions.

use std::any::Any;
use std::rc::Rc;

use super::{Slot, use_hook};
use crate::scope::Scope;

pub(crate) struct StoreSlot {
    /// Released on unmount.
    pub subscription: Subscription,
    /// Last snapshot, compared by `Rc` identity.
    pub snapshot: Box<dyn Any>,
}

/// A value source outside the component tree.
///
/// `snapshot` returns the same snapshot until the value changes.
pub trait ExternalStore {
    type Value: ?Sized + 'static;

    /// Starts telling `notify` about changes. Dropping the `Subscription`
    /// stops it.
    fn subscribe(&self, notify: Notify) -> Subscription;

    /// What the value is now.
    fn snapshot(&self) -> Snapshot<Self::Value>;
}

/// A store snapshot compared by `Rc` identity.
pub struct Snapshot<T: ?Sized>(Rc<T>);

impl<T: ?Sized> Clone for Snapshot<T> {
    fn clone(&self) -> Self {
        Self(Rc::clone(&self.0))
    }
}
impl<T: ?Sized> PartialEq for Snapshot<T> {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}
impl<T: ?Sized> Eq for Snapshot<T> {}
impl<T: ?Sized> std::ops::Deref for Snapshot<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.0
    }
}
/// Converts an `Rc` into a snapshot.
impl<T: ?Sized> From<Rc<T>> for Snapshot<T> {
    fn from(value: Rc<T>) -> Self {
        Self(value)
    }
}

/// What a store calls to say it changed. `Clone`, so a store keeps one per
/// reader.
#[derive(Clone)]
pub struct Notify(Rc<dyn Fn()>);

impl Notify {
    /// Marks the subscribed component for redraw.
    pub fn changed(&self) {
        (self.0)();
    }
}

/// Ends a subscription when it is dropped.
pub struct Subscription(Option<Box<dyn FnOnce()>>);

impl Subscription {
    pub fn new(stop: impl FnOnce() + 'static) -> Self {
        Self(Some(Box::new(stop)))
    }
}

impl Drop for Subscription {
    fn drop(&mut self) {
        if let Some(stop) = self.0.take() {
            stop();
        }
    }
}

/// Subscribes to a store and returns its current snapshot.
///
/// The store must remain the same for the component's lifetime.
#[track_caller]
pub fn use_sync_external_store<S: ExternalStore>(
    scope: &mut Scope,
    store: &S,
) -> Snapshot<S::Value> {
    let id = scope.id;
    let runtime = crate::current::handle();

    let now = store.snapshot();
    let mut first = Some(now.clone());

    use_hook(
        scope,
        "Store",
        || {
            let notify = Notify(Rc::new(move || {
                if let Some(runtime) = runtime.as_ref().and_then(std::rc::Weak::upgrade)
                    && let Ok(mut rt) = runtime.try_borrow_mut()
                {
                    rt.mark(id);
                }
            }));
            Slot::Store(StoreSlot {
                subscription: store.subscribe(notify),
                snapshot: Box::new(first.take().expect("the first render reads once")),
            })
        },
        |slot| {
            let Slot::Store(held) = slot else {
                unreachable!("checked by shape")
            };
            let last = held
                .snapshot
                .downcast_ref::<Snapshot<S::Value>>()
                .expect("a store slot holds the type its first render put there");
            if *last == now {
                last.clone()
            } else {
                held.snapshot = Box::new(now.clone());
                now.clone()
            }
        },
    )
}
