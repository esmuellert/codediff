//! `context!` — the marker, its props, and the three impls.

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::parse::{Parse, ParseStream};
use syn::{Expr, Ident, Token, Type, Visibility, parse2};

/// `context!(pub Theme: theme::Theme = theme::Theme::DARK);`
struct Declaration {
    docs: Vec<syn::Attribute>,
    visibility: Visibility,
    name: Ident,
    value: Type,
    default: Expr,
    same: Option<Expr>,
}

impl Parse for Declaration {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let docs = input.call(syn::Attribute::parse_outer)?;
        let visibility = input.parse()?;
        let name = input.parse()?;
        input.parse::<Token![:]>()?;
        let value = input.parse()?;
        input.parse::<Token![=]>()?;
        let default = input.parse()?;
        // A value with no `==` of its own names the comparison here.
        let same = if input.peek(Token![,]) {
            input.parse::<Token![,]>()?;
            Some(input.parse()?)
        } else {
            None
        };
        Ok(Self { docs, visibility, name, value, default, same })
    }
}

pub(crate) fn expand(input: TokenStream) -> TokenStream {
    let declaration: Declaration = match parse2(input) {
        Ok(declaration) => declaration,
        Err(error) => return error.to_compile_error(),
    };

    let Declaration { docs, visibility, name, value, default, same } = declaration;
    let props_name = format_ident!("{}Props", name);
    let text = name.to_string();

    let same = match same {
        Some(same) => quote! { (#same)(old, new) },
        None => quote! { old == new },
    };

    quote! {
        #(#docs)*
        #visibility struct #name;

        #visibility struct #props_name {
            pub value: #value,
            pub children: ::loom::Children,
        }

        impl ::loom::Context for #name {
            type Value = #value;
            fn default_value() -> Self::Value { #default }
            fn same(old: &Self::Value, new: &Self::Value) -> bool { #same }
        }

        impl ::loom::Component for #name {
            type Props = #props_name;
            const NAME: &'static str = #text;
            fn render(props: &Self::Props, scope: &mut ::loom::Scope) -> ::loom::Node {
                ::loom::offer::<#name>(scope, props.value.clone());
                ::loom::Node::Fragment(props.children.clone())
            }
        }

        impl ::loom::Element for #name {
            type Props = #props_name;
            fn build(props: Self::Props, key: Option<::loom::Key>) -> ::loom::Node {
                ::loom::Node::part::<#name>(props, key)
            }
        }
    }
}
