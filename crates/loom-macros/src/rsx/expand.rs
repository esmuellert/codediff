//! The table of §11.2.

use proc_macro2::TokenStream;
use quote::{format_ident, quote};

use super::parse::{Condition, Element, For, If, Match, Node, Otherwise};

/// Several nodes are a fragment; one is itself; none is `Empty`.
pub(crate) fn rsx(nodes: &[Node]) -> TokenStream {
    match nodes {
        [] => quote! { ::loom::Node::Empty },
        [one] => node(one),
        many => {
            let each = many.iter().map(node);
            quote! { ::loom::Node::Fragment(vec![#(#each),*]) }
        }
    }
}

fn node(node: &Node) -> TokenStream {
    match node {
        Node::Element(element) => build(element),
        Node::Text(text) => quote! {
            <::loom::Text as ::loom::Element>::build(
                ::loom::TextProps { text: #text.into(), ..Default::default() },
                None,
            )
        },
        Node::Block(expr) => quote! { ::loom::Node::from(#expr) },
        Node::If(chain) => branch(chain),
        Node::Match(arms) => match_arms(arms),
        Node::For(loop_) => for_loop(loop_),
    }
}

fn build(element: &Element) -> TokenStream {
    let path = &element.path;
    let props = if element.props.is_empty() && element.children.is_empty() {
        quote! { ::std::default::Default::default() }
    } else {
        let props_name = props_of(path);
        let names = element.props.iter().map(|(name, _)| name);
        let values = element.props.iter().map(|(_, value)| value);
        let children = if element.children.is_empty() {
            quote! {}
        } else {
            let each = element.children.iter().map(node);
            quote! { children: vec![#(#each),*], }
        };
        // `..` lands last, where Rust requires it. Children with no explicit
        // props still need the host defaults around them.
        let rest = if element.rest || element.props.is_empty() {
            quote! { ..Default::default() }
        } else {
            quote! {}
        };
        quote! { #props_name { #(#names: #values,)* #children #rest } }
    };

    let key = match &element.key {
        Some(key) => quote! { Some(::loom::Key::from(#key)) },
        None => quote! { None },
    };

    quote! {
        <#path as ::loom::Element>::build(#props, #key)
    }
}

/// `Name` becomes `NameProps`, keeping whatever module path it came with.
fn props_of(path: &syn::Path) -> TokenStream {
    let mut path = path.clone();
    if let Some(last) = path.segments.last_mut() {
        last.ident = format_ident!("{}Props", last.ident);
    }
    quote! { #path }
}

fn branch(chain: &If) -> TokenStream {
    let then = fragment(&chain.then);
    let otherwise = match &chain.otherwise {
        Some(next) => match next {
            Otherwise::If(inner) => branch(inner),
            Otherwise::Block(nodes) => fragment(nodes),
        },
        None => quote! { ::loom::Node::Empty },
    };

    match &chain.condition {
        Condition::Plain(condition) => quote! {
            if #condition { #then } else { #otherwise }
        },
        Condition::Let(pattern, subject) => quote! {
            if let #pattern = #subject { #then } else { #otherwise }
        },
    }
}

fn match_arms(arms: &Match) -> TokenStream {
    let subject = &arms.subject;
    let each = arms.arms.iter().map(|(pattern, guard, body)| {
        let body = fragment(body);
        match guard {
            Some(guard) => quote! { #pattern if #guard => #body },
            None => quote! { #pattern => #body },
        }
    });
    quote! { match #subject { #(#each,)* } }
}

fn for_loop(loop_: &For) -> TokenStream {
    let pattern = &loop_.pattern;
    let over = &loop_.over;
    let body = fragment(&loop_.body);
    quote! {
        ::loom::Node::Fragment(
            ::std::iter::IntoIterator::into_iter(#over)
                .map(|#pattern| #body)
                .collect::<Vec<_>>()
        )
    }
}

/// A branch arm is always a fragment, so both sides have one type.
fn fragment(nodes: &[Node]) -> TokenStream {
    let each = nodes.iter().map(node);
    quote! { ::loom::Node::Fragment(vec![#(#each),*]) }
}
