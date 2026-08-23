//! Passing a value down without threading it through every component.

use std::any::TypeId;
use std::rc::Rc;

use crate::scope::{Scope, ScopeId};

/// One context: the key a reader names and the element a provider writes.
///
/// Declared with `context!`, never by hand.
pub trait Context: 'static {
    type Value: Clone + 'static;
    /// What a reader gets when nothing above it provides one.
    fn default_value() -> Self::Value;
    /// Whether a new offer matches the last, so the version can stay put.
    fn same(old: &Self::Value, new: &Self::Value) -> bool;
}

/// The nearest ancestor's value for `C`, or `C::default_value()`.
///
/// The read is recorded, so a memoised component cannot go stale.
pub fn use_context<C: Context>(scope: &mut Scope) -> C::Value {
    let id = scope.id;
    let key = TypeId::of::<C>();

    let found = crate::current::with_mut(|rt| {
        let found = rt.read_context(id, key);
        let version = found.as_ref().map_or(0, |(_, v)| *v);
        if let Some(mounted) = rt.scopes.get_mut(id) {
            match mounted.reads.iter_mut().find(|(t, _)| *t == key) {
                Some(read) => read.1 = version,
                None => mounted.reads.push((key, version)),
            }
        }
        found
    })
    .flatten();

    match found {
        Some((value, _)) => value
            .downcast_ref::<C::Value>()
            .cloned()
            .unwrap_or_else(C::default_value),
        None => C::default_value(),
    }
}

/// What `context!`'s `Component::render` calls.
///
/// Not API: the way to offer a value is to write the provider element.
#[doc(hidden)]
pub fn offer<C: Context>(scope: &mut Scope, value: C::Value) {
    let id = scope.id;
    let key = TypeId::of::<C>();

    crate::current::with_mut(|rt| {
        let offers = rt.offers.entry(id).or_default();
        match offers.iter_mut().find(|o| o.context == key) {
            Some(held) => {
                let unchanged = held
                    .value
                    .downcast_ref::<C::Value>()
                    .is_some_and(|last| C::same(last, &value));
                if !unchanged {
                    rt.context_version += 1;
                    held.version = rt.context_version;
                    held.value = Rc::new(value);
                    // Every reader below this scope has to run again.
                    mark_readers(rt, id, key);
                }
            }
            None => {
                rt.context_version += 1;
                let version = rt.context_version;
                offers.push(crate::runtime::Offer { context: key, value: Rc::new(value), version });
            }
        }
    });
}

/// Marks every scope below `from` that read `key`.
fn mark_readers(rt: &mut crate::runtime::Runtime, from: ScopeId, key: TypeId) {
    let children = rt.scopes.get(from).map(|m| m.children.clone()).unwrap_or_default();
    for child in children {
        let reads = rt.scopes.get(child).is_some_and(|m| m.reads.iter().any(|(t, _)| *t == key));
        if reads {
            rt.mark(child);
        }
        // A nearer provider of the same context shadows this one, so the walk
        // stops there.
        let shadowed = rt
            .offers
            .get(&child)
            .is_some_and(|offers| offers.iter().any(|o| o.context == key));
        if !shadowed {
            mark_readers(rt, child, key);
        }
    }
}
