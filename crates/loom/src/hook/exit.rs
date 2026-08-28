//! Stopping the loop from inside a component.

use crate::scope::Scope;

/// Stops the loop the next time it looks.
///
/// ```ignore
/// let exit = use_exit(scope);
/// let keys = Listeners::new().on_key(move |_| { exit(); Bubble::Stop });
/// ```
pub fn use_exit(scope: &mut Scope) -> &'static dyn Fn() {
    let _ = scope;
    &|| {
        super::require_runtime();
        crate::current::with_mut(|rt| rt.exit = true);
    }
}
