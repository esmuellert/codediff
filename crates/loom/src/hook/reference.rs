//! Mutable storage that survives a render without scheduling one.

use std::cell::{RefCell, RefMut};

use super::{Slot, use_hook};
use crate::scope::{Scope, ScopeId};

/// A mutable value that survives renders without causing one.
///
/// The Rust form of React's `useRef`. `current()` is `ref.current`: read
/// through it, call methods on it, or assign over it.
///
/// ```ignore
/// view.current().scroll(3);
/// let top = view.current().top();
/// *view.current() = Viewport::new();
/// ```
pub struct Ref<T: 'static> {
    scope: ScopeId,
    slot: u16,
    /// Made when the component mounts and kept for the run of the program,
    /// which is what keeps `Ref<T>` `Copy`.
    cell: &'static RefCell<T>,
}

impl<T> Clone for Ref<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for Ref<T> {}
impl<T> PartialEq for Ref<T> {
    fn eq(&self, other: &Self) -> bool {
        self.scope == other.scope && self.slot == other.slot
    }
}
impl<T> Eq for Ref<T> {}
impl<T> std::fmt::Debug for Ref<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Ref({:?}#{})", self.scope, self.slot)
    }
}

impl<T: 'static> Ref<T> {
    /// The value in the slot. Writing through it is silent.
    ///
    /// The guard lasts to the end of the statement. Two of them on the same
    /// ref at once panic (P4.5).
    pub fn current(self) -> RefMut<'static, T> {
        super::require_runtime();
        self.cell
            .try_borrow_mut()
            .unwrap_or_else(|_| panic!("one ref was read twice in one statement \u{2014} finish one before starting the next"))
    }

    /// Whether the owning component is still mounted.
    pub fn is_mounted(self) -> bool {
        crate::current::with(|rt| rt.is_alive(self.scope)).unwrap_or(false)
    }
}

/// Mutable storage that survives a render without scheduling one.
///
/// `first` runs once. Reads and writes both go through `Ref::current`.
#[track_caller]
pub fn use_ref<T: 'static>(scope: &mut Scope, first: impl FnOnce() -> T) -> Ref<T> {
    let id = scope.id;
    let index = crate::current::with(|rt| rt.hooks[&id].index).unwrap_or(0);
    let mut first = Some(first);

    let cell = use_hook(
        scope,
        "Ref",
        || {
            let start = first.take().expect("the first render builds the value once")();
            // Leaked once per slot, like a state slot's writer, which is what
            // keeps the handle `Copy`.
            let leaked: &'static RefCell<T> = Box::leak(Box::new(RefCell::new(start)));
            Slot::Ref(Box::new(leaked))
        },
        |slot| {
            let Slot::Ref(held) = slot else { unreachable!("checked by shape") };
            *held
                .downcast_ref::<&'static RefCell<T>>()
                .expect("a ref slot holds the type its first render put there")
        },
    );

    Ref { scope: id, slot: index as u16, cell }
}
