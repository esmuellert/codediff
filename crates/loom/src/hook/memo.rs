//! A value recomputed only when its dependencies change.

use std::any::Any;
use std::rc::Rc;

use super::{Slot, use_hook};
use crate::scope::Scope;

pub(crate) struct MemoSlot {
    pub deps: Box<dyn Any>,
    pub value: Rc<dyn Any>,
}

/// A value recomputed only when `deps` changes.
///
/// Returns the same `Rc` otherwise, for as long as the component lives, so
/// the identity is something you may rely on.
#[track_caller]
pub fn use_memo<D, T>(scope: &mut Scope, deps: D, compute: impl FnOnce() -> T) -> Rc<T>
where
    D: PartialEq + 'static,
    T: 'static,
{
    let input = std::cell::Cell::new(Some((deps, compute)));

    let value = use_hook(
        scope,
        "Memo",
        || {
            let (deps, compute) = input.take().expect("the first render computes once");
            Slot::Memo(MemoSlot { deps: Box::new(deps), value: Rc::new(compute()) })
        },
        |slot| {
            let Slot::Memo(memo) = slot else { unreachable!("checked by shape") };
            if let Some((deps, compute)) = input.take() {
                let same = memo
                    .deps
                    .downcast_ref::<D>()
                    .is_some_and(|last| *last == deps);
                if !same {
                    memo.deps = Box::new(deps);
                    memo.value = Rc::new(compute());
                }
            }
            Rc::clone(&memo.value)
        },
    );

    value
        .downcast::<T>()
        .unwrap_or_else(|_| panic!("a memo slot holds the type its first render put there"))
}
