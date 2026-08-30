//! Hook slots, and the one primitive every hook is built on.

mod context;
mod effect;
mod exit;
mod measure;
mod memo;
mod reference;
mod state;
mod store;
mod worker;

use std::any::Any;

pub use context::{Context, offer, use_context};
pub use effect::{Always, Cleanup, use_effect, use_layout_effect};
pub use exit::use_exit;
pub use measure::{Size, use_measure};
pub use memo::use_memo;
pub use reference::{Ref, use_ref};
pub use state::{SetState, use_state};
pub use store::{ExternalStore, Notify, Snapshot, Subscription, use_sync_external_store};
pub use worker::{Observable, Observer, Promise, Resolver, observable, promise};

pub(crate) use effect::EffectRun;

use crate::scope::Scope;

/// One hook's storage.
pub(crate) enum Slot {
    State(Box<dyn state::PendingState>),
    Ref(Box<dyn Any>),
    Memo(memo::MemoSlot),
    Effect(effect::EffectSlot),
    LayoutEffect(effect::EffectSlot),
    Store(store::StoreSlot),
}

impl Slot {
    fn shape(&self) -> &'static str {
        match self {
            Slot::State(_) => "State",
            Slot::Ref(_) => "Ref",
            Slot::Memo(_) => "Memo",
            Slot::Effect(_) => "Effect",
            Slot::LayoutEffect(_) => "LayoutEffect",
            Slot::Store(_) => "Store",
        }
    }
}

pub(crate) struct Hooks {
    pub slots: Vec<Slot>,
    /// Reset to 0 at the top of each render.
    pub index: usize,
    /// How many slots the last completed render used. `None` before the
    /// first one finishes.
    pub used: Option<usize>,
    #[cfg(debug_assertions)]
    pub sites: Vec<&'static std::panic::Location<'static>>,
}

impl Hooks {
    pub fn new() -> Self {
        Self {
            slots: Vec::new(),
            index: 0,
            used: None,
            #[cfg(debug_assertions)]
            sites: Vec::new(),
        }
    }

    /// Runs every cleanup this scope is holding, deepest slot last.
    pub fn cleanup(self) {
        for slot in self.slots {
            match slot {
                Slot::Effect(e) | Slot::LayoutEffect(e) => {
                    if let Some(undo) = e.cleanup {
                        undo();
                    }
                }
                Slot::Store(s) => drop(s.subscription),
                _ => {}
            }
        }
    }
}

/// Bumps the index, pushes on the first render, checks the discriminant
/// otherwise, and panics naming both call sites when they disagree (P4.1).
#[track_caller]
pub(crate) fn use_hook<H>(
    scope: &mut Scope,
    kind: &'static str,
    first: impl FnOnce() -> Slot,
    read: impl FnOnce(&mut Slot) -> H,
) -> H {
    let id = scope.id;
    let site = std::panic::Location::caller();

    let (index, fresh) = crate::current::with_mut(|rt| {
        let hooks = rt.hooks.get_mut(&id).expect("a running scope has hook storage");
        let index = hooks.index;
        hooks.index += 1;
        (index, index >= hooks.slots.len())
    })
    .expect("hooks run inside a component");

    if fresh {
        let slot = first();
        crate::current::with_mut(|rt| {
            let hooks = rt.hooks.get_mut(&id).expect("a running scope has hook storage");
            hooks.slots.push(slot);
            #[cfg(debug_assertions)]
            hooks.sites.push(site);
        });
    }

    crate::current::with_mut(|rt| {
        let name = rt.name_of(id);
        let hooks = rt.hooks.get_mut(&id).expect("a running scope has hook storage");
        let current = hooks.slots[index].shape();
        if current != kind {
            #[cfg(debug_assertions)]
            let was = hooks.sites[index];
            #[cfg(debug_assertions)]
            panic!(
                "{name}: hook {index} was a {current} at {was}, and is a {kind} here at {site}. \
                 Hooks must run in the same order every render \u{2014} none inside an if, a loop, \
                 or after an early return."
            );
            #[cfg(not(debug_assertions))]
            panic!(
                "{name}: hook {index} was a {current} and is a {kind}. Hooks must run in the \
                 same order every render."
            );
        }
        read(&mut hooks.slots[index])
    })
    .expect("hooks run inside a component")
}

/// The count check at the end of a render. P4.2.
pub(crate) fn finish_render(name: &'static str, hooks: &mut Hooks) {
    let called = hooks.index;
    if let Some(last) = hooks.used
        && called != last
    {
        panic!(
            "{name}: this render called {called} hooks and the last one called {last}. \
             Hooks must run in the same order every render."
        );
    }
    hooks.used = Some(called);
}

/// P4.4 — a setter or ref used with no runtime entered.
pub(crate) fn require_runtime() {
    assert!(
        crate::current::inside(),
        "state setters and refs may only be used while loom is running a \
         component, listener, effect, worker reply or painter"
    );
}
