//! Work to do after the frame, and work to do after layout but before paint.

use std::any::Any;

use super::{Slot, use_hook};
use crate::scope::{Scope, ScopeId};

pub(crate) struct EffectSlot {
    pub deps: Box<dyn Any>,
    pub cleanup: Option<Box<dyn FnOnce()>>,
    /// Bumped each time the effect runs, so a reply from the previous run is
    /// refused.
    pub generation: u64,
}

/// One effect the frame has queued but not yet run.
pub(crate) struct EffectRun {
    pub scope: ScopeId,
    pub slot: u16,
    pub generation: u64,
    pub run: Box<dyn FnOnce() -> Option<Box<dyn FnOnce()>>>,
}

/// What `run` may return: a function that undoes the work, or `()` for
/// nothing to undo.
pub trait Cleanup: 'static {
    fn into_cleanup(self) -> Option<Box<dyn FnOnce()>>;
}

impl Cleanup for () {
    fn into_cleanup(self) -> Option<Box<dyn FnOnce()>> {
        None
    }
}

impl<F: FnOnce() + 'static> Cleanup for F {
    fn into_cleanup(self) -> Option<Box<dyn FnOnce()>> {
        Some(Box::new(self))
    }
}

/// Deps that never compare equal, so the effect runs after every paint. This
/// is what React means by leaving the dependency array out.
#[derive(Clone, Copy, Debug)]
pub struct Always;

impl PartialEq for Always {
    fn eq(&self, _: &Self) -> bool {
        false
    }
}

/// Work to do after the frame is painted.
///
/// Re-runs after the paint of the first frame in which `deps != previous
/// deps`. What `run` returns is the cleanup: it is called before the next run
/// and again when the component goes away.
///
/// `()` as deps runs once. `Always` runs after every paint.
#[track_caller]
pub fn use_effect<D, C>(scope: &mut Scope, deps: D, run: impl FnOnce() -> C + 'static)
where
    D: PartialEq + 'static,
    C: Cleanup,
{
    queue(scope, deps, run, false);
}

/// The same, run before the frame is painted rather than after.
///
/// Layout has finished, so every `ref` holds its node and `NodeHandle::area`
/// answers this frame's rectangle. A state write here re-renders and re-lays
/// out before anything reaches the screen.
///
/// Prefer `use_effect`. This one holds the frame up.
#[track_caller]
pub fn use_layout_effect<D, C>(scope: &mut Scope, deps: D, run: impl FnOnce() -> C + 'static)
where
    D: PartialEq + 'static,
    C: Cleanup,
{
    queue(scope, deps, run, true);
}

#[track_caller]
fn queue<D, C>(scope: &mut Scope, deps: D, run: impl FnOnce() -> C + 'static, before_paint: bool)
where
    D: PartialEq + 'static,
    C: Cleanup,
{
    let id = scope.id;
    let index = crate::current::with(|rt| rt.hooks[&id].index).unwrap_or(0) as u16;
    let kind = if before_paint { "LayoutEffect" } else { "Effect" };
    let deps_cell = std::cell::Cell::new(Some(deps));

    let generation = use_hook(
        scope,
        kind,
        || {
            let deps = deps_cell.take().expect("the first render stores its deps");
            let slot = EffectSlot { deps: Box::new(deps), cleanup: None, generation: 0 };
            if before_paint { Slot::LayoutEffect(slot) } else { Slot::Effect(slot) }
        },
        |slot| {
            let effect = match slot {
                Slot::Effect(e) | Slot::LayoutEffect(e) => e,
                _ => unreachable!("checked by shape"),
            };
            match deps_cell.take() {
                // The slot was made just above, so this is the first render
                // and the effect runs.
                None => {
                    effect.generation += 1;
                    Some(effect.generation)
                }
                Some(next) => {
                    let same = effect.deps.downcast_ref::<D>().is_some_and(|last| *last == next);
                    if same {
                        None
                    } else {
                        effect.deps = Box::new(next);
                        effect.generation += 1;
                        Some(effect.generation)
                    }
                }
            }
        },
    );

    let Some(generation) = generation else { return };

    let queued = EffectRun {
        scope: id,
        slot: index,
        generation,
        run: Box::new(move || run().into_cleanup()),
    };

    crate::current::with_mut(|rt| {
        if before_paint {
            rt.layout_effects.push(queued);
        } else {
            rt.effects.push(queued);
        }
    });
}
