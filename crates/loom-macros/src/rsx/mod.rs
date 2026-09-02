//! The two halves of `rsx!`, and the error they share.

mod expand;
mod parse;

use proc_macro2::TokenStream;
use syn::parse::Parser;

pub(crate) fn expand(input: TokenStream) -> TokenStream {
    match parse::nodes.parse2(input) {
        Ok(nodes) => expand::rsx(&nodes),
        Err(error) => error.to_compile_error(),
    }
}
