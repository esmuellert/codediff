//! One render's value, and a stable setter for its next.

use std::any::Any;
use std::ops::Deref;

use super::{Slot, use_hook};
use crate::scope::{Scope, ScopeId};

pub type StateUpdate<T> = dyn Fn(T) -> T;
pub type StateWriter<T> = dyn Fn(&StateUpdate<T>);

/// A state slot the runtime can move forward without knowing its type.
pub(crate) trait PendingState {
    /// Moves the pending value into the committed one. Whether anything moved.
    fn commit(&mut self) -> bool;
    fn as_any(&mut self) -> &mut dyn Any;
}

pub(crate) struct StateSlot<T: 'static> {
    pub value: T,
    pub pending: Option<T>,
    /// Made once, when the component mounts, and held for the run of the
    /// program. Paying it once per slot is what keeps `SetState<T>` `Copy`.
    pub write: &'static StateWriter<T>,
}

impl<T: Clone + PartialEq + 'static> PendingState for StateSlot<T> {
    fn commit(&mut self) -> bool {
        match self.pending.take() {
            Some(next) if next != self.value => {
                self.value = next;
                true
            }
            _ => false,
        }
    }
    fn as_any(&mut self) -> &mut dyn Any {
        self
    }
}

/// Writes one state slot. Called like a function.
///
/// The closure is given the value the slot will hold when the next render
/// starts, and answers what to put there.
///
/// ```ignore
/// set_cursor(&|_| 5);
/// set_cursor(&|cursor| cursor + 1);
/// ```
pub struct SetState<T: 'static> {
    scope: ScopeId,
    slot: u16,
    write: &'static StateWriter<T>,
}

impl<T> Clone for SetState<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for SetState<T> {}
impl<T> PartialEq for SetState<T> {
    fn eq(&self, other: &Self) -> bool {
        self.scope == other.scope && self.slot == other.slot
    }
}
impl<T> Eq for SetState<T> {}
impl<T> std::fmt::Debug for SetState<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "SetState({:?}#{})", self.scope, self.slot)
    }
}

impl<T: 'static> SetState<T> {
    /// Whether the owning component is still mounted.
    pub fn is_mounted(self) -> bool {
        crate::current::with(|rt| rt.is_alive(self.scope)).unwrap_or(false)
    }
}

/// Taking the closure by reference is what lets a write borrow whatever is in
/// scope; nothing is boxed and nothing needs `'static`.
impl<T: 'static> Deref for SetState<T> {
    type Target = StateWriter<T>;
    fn deref(&self) -> &Self::Target {
        self.write
    }
}

/// Moves a slot's pending value forward, from outside any component.
fn write_slot<T: Clone + PartialEq + 'static>(
    scope: ScopeId,
    slot: usize,
    name: &'static str,
    next: &StateUpdate<T>,
) {
    super::require_runtime();

    let now = crate::current::with_mut(|rt| {
        let hooks = rt.hooks.get_mut(&scope)?;
        let Slot::State(state) = hooks.slots.get_mut(slot)? else {
            return None;
        };
        let cell = state.as_any().downcast_mut::<StateSlot<T>>()?;
        // The closure sees the value the slot will hold when the next render
        // starts, so two writes in one listener compose.
        Some(cell.pending.clone().unwrap_or_else(|| cell.value.clone()))
    })
    .flatten();

    // P4.3
    let Some(now) = now else {
        panic!("a SetState was used after {name} was removed");
    };

    let next = next(now);

    crate::current::with_mut(|rt| {
        let Some(hooks) = rt.hooks.get_mut(&scope) else {
            return;
        };
        let Some(Slot::State(state)) = hooks.slots.get_mut(slot) else {
            return;
        };
        let Some(cell) = state.as_any().downcast_mut::<StateSlot<T>>() else {
            return;
        };
        let changed = next != cell.value;
        cell.pending = Some(next);
        if changed {
            rt.mark(scope);
        }
    });
}

/// One render's value and a stable setter for its next value.
///
/// `first` runs once, when the component mounts. A write applies at once to
/// the pending value, and the returned `T` is this render's snapshot.
#[track_caller]
pub fn use_state<T: Clone + PartialEq + 'static>(
    scope: &mut Scope,
    first: impl FnOnce() -> T,
) -> (T, SetState<T>) {
    let id = scope.id;
    let name = scope.name();
    let index = crate::current::with(|rt| rt.hooks[&id].index).unwrap_or(0);
    let mut first = Some(first);

    let (value, write) = use_hook(
        scope,
        "State",
        || {
            let start = first
                .take()
                .expect("the first render builds the value once")();
            let write: &'static StateWriter<T> =
                Box::leak(Box::new(move |next: &StateUpdate<T>| {
                    write_slot::<T>(id, index, name, next)
                }));
            Slot::State(Box::new(StateSlot {
                value: start,
                pending: None,
                write,
            }))
        },
        |slot| {
            let Slot::State(state) = slot else {
                unreachable!("checked by shape")
            };
            let cell = state
                .as_any()
                .downcast_mut::<StateSlot<T>>()
                .expect("a state slot holds the type its first render put there");
            (cell.value.clone(), cell.write)
        },
    );

    (
        value,
        SetState {
            scope: id,
            slot: index as u16,
            write,
        },
    )
}
