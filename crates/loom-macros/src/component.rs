//! `#[component]` — the props struct, the two impls, and the name.

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{FnArg, ItemFn, Pat, ReturnType, parse2};

pub(crate) fn expand(args: TokenStream, input: TokenStream) -> TokenStream {
    let memo = args.to_string().contains("memo");
    let function: ItemFn = match parse2(input) {
        Ok(function) => function,
        Err(error) => return error.to_compile_error(),
    };

    let name = function.sig.ident.clone();
    let props_name = format_ident!("{}Props", name);
    let visibility = function.vis.clone();
    let body = function.block.clone();
    let generics = function.sig.generics.clone();

    let ReturnType::Type(_, answers) = function.sig.output.clone() else {
        return syn::Error::new_spanned(&function.sig, "a component answers a Node")
            .to_compile_error();
    };

    // The first parameter is the scope; the rest are the props.
    let mut inputs = function.sig.inputs.iter();
    let (scope, scope_type) = match inputs.next() {
        Some(FnArg::Typed(scope)) => (scope.pat.clone(), scope.ty.clone()),
        _ => {
            return syn::Error::new_spanned(
                &function.sig,
                "a component's first parameter is `scope: &mut Scope`",
            )
            .to_compile_error();
        }
    };

    let mut fields = Vec::new();
    let mut names = Vec::new();
    for argument in inputs {
        let FnArg::Typed(argument) = argument else {
            return syn::Error::new_spanned(argument, "a component takes no self")
                .to_compile_error();
        };
        let Pat::Ident(ident) = argument.pat.as_ref() else {
            return syn::Error::new_spanned(&argument.pat, "a prop is one name").to_compile_error();
        };
        let field = ident.ident.clone();
        let ty = argument.ty.clone();
        fields.push(quote! { pub #field: #ty });
        names.push(field);
    }

    let props_equal = if memo {
        quote! {
            Some(|a: &dyn ::std::any::Any, b: &dyn ::std::any::Any| {
                match (a.downcast_ref::<#props_name>(), b.downcast_ref::<#props_name>()) {
                    (Some(a), Some(b)) => a == b,
                    _ => false,
                }
            })
        }
    } else {
        quote! { None }
    };

    let text = name.to_string();

    // A component with no props is written `Name {}`, which expands to
    // `NameProps { ..Default::default() }`.
    let default = if fields.is_empty() {
        quote! { #[derive(Default)] }
    } else {
        quote! {}
    };

    quote! {
        #[allow(non_camel_case_types)]
        #visibility struct #name;

        #default
        #visibility struct #props_name #generics {
            #(#fields,)*
        }

        impl ::loom::Component for #name {
            type Props = #props_name;
            const NAME: &'static str = #text;
            fn render(props: &Self::Props, #scope: #scope_type) -> #answers {
                // By reference, so a prop is borrowed rather than cloned.
                let #props_name { #(#names,)* } = props;
                #body
            }
        }

        impl ::loom::Element for #name {
            type Props = #props_name;
            fn build(props: Self::Props, key: Option<::loom::Key>) -> ::loom::Node {
                let equal: Option<fn(&dyn ::std::any::Any, &dyn ::std::any::Any) -> bool> =
                    #props_equal;
                let mut node = ::loom::Node::part::<#name>(props, key);
                if let ::loom::Node::Part(part) = &mut node {
                    part.props_equal = equal;
                }
                node
            }
        }
    }
}
