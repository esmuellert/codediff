//! The three macros loom's components are written with.

mod component;
mod context;
mod rsx;

use proc_macro::TokenStream;

/// Turns a function into a component: a props struct, a `Component` impl and
/// an `Element` impl.
///
/// `#[component(memo)]` compares the props before re-running.
#[proc_macro_attribute]
pub fn component(args: TokenStream, input: TokenStream) -> TokenStream {
    component::expand(args.into(), input.into()).into()
}

/// Declares one context: the key a reader names and the element a provider
/// writes.
#[proc_macro]
pub fn context(input: TokenStream) -> TokenStream {
    context::expand(input.into()).into()
}

/// One frame's description, as Rust syntax.
#[proc_macro]
pub fn rsx(input: TokenStream) -> TokenStream {
    rsx::expand(input.into()).into()
}
