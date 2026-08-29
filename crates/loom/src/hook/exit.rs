//! Stopping the loop from inside a component.

/// Stops the loop. Called from a listener or an effect.
pub fn use_exit(scope: &mut crate::scope::Scope) -> &'static dyn Fn() {
    let _ = scope;
    &|| {
        super::require_runtime();
        crate::current::with_mut(|rt| rt.exit = true);
    }
}
