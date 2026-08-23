//! The grammar of §11.1.

use syn::ext::IdentExt;
use syn::parse::{ParseStream, Result};
use syn::{Expr, Ident, LitStr, Pat, Path, Token, braced, token};

pub(crate) enum Node {
    Element(Element),
    Text(LitStr),
    Block(Expr),
    If(If),
    Match(Match),
    For(For),
}

pub(crate) struct Element {
    pub path: Path,
    pub key: Option<Expr>,
    pub props: Vec<(Ident, Expr)>,
    pub rest: bool,
    pub children: Vec<Node>,
}

pub(crate) struct If {
    pub condition: Condition,
    pub then: Vec<Node>,
    pub otherwise: Option<Box<Otherwise>>,
}

pub(crate) enum Condition {
    Plain(Expr),
    Let(Pat, Expr),
}

pub(crate) enum Otherwise {
    If(If),
    Block(Vec<Node>),
}

pub(crate) struct Match {
    pub subject: Expr,
    pub arms: Vec<(Pat, Option<Expr>, Vec<Node>)>,
}

pub(crate) struct For {
    pub pattern: Pat,
    pub over: Expr,
    pub body: Vec<Node>,
}

pub(crate) fn nodes(input: ParseStream) -> Result<Vec<Node>> {
    let mut out = Vec::new();
    while !input.is_empty() {
        out.push(node(input)?);
        // The comma between entries is optional.
        let _ = input.parse::<Token![,]>();
    }
    Ok(out)
}

fn node(input: ParseStream) -> Result<Node> {
    if input.peek(LitStr) {
        return Ok(Node::Text(input.parse()?));
    }
    if input.peek(Token![if]) {
        return Ok(Node::If(if_chain(input)?));
    }
    if input.peek(Token![match]) {
        return Ok(Node::Match(match_arms(input)?));
    }
    if input.peek(Token![for]) {
        return Ok(Node::For(for_loop(input)?));
    }
    if input.peek(token::Brace) {
        let inner;
        braced!(inner in input);
        return Ok(Node::Block(inner.parse()?));
    }
    Ok(Node::Element(element(input)?))
}

fn element(input: ParseStream) -> Result<Element> {
    let path: Path = input.parse()?;
    let inner;
    braced!(inner in input);

    let mut key = None;
    let mut props = Vec::new();
    let mut rest = false;
    let mut children = Vec::new();

    while !inner.is_empty() {
        if inner.peek(Token![..]) {
            inner.parse::<Token![..]>()?;
            rest = true;
        } else if is_prop(&inner) {
            // `ref` is a keyword, so the name is parsed with `parse_any`.
            let name = Ident::parse_any(&inner)?;
            inner.parse::<Token![:]>()?;
            let value: Expr = inner.parse()?;
            if name == "key" {
                key = Some(value);
            } else if name == "ref" {
                props.push((Ident::new("node_ref", name.span()), value));
            } else {
                props.push((name, value));
            }
        } else {
            children.push(node(&inner)?);
        }
        let _ = inner.parse::<Token![,]>();
    }

    Ok(Element { path, key, props, rest, children })
}

/// A prop is `ident :` with no `::` after it, which is what tells it from a
/// child element whose path starts with the same token.
fn is_prop(input: ParseStream) -> bool {
    let ahead = input.fork();
    if Ident::parse_any(&ahead).is_err() {
        return false;
    }
    ahead.peek(Token![:]) && !ahead.peek(Token![::])
}

fn if_chain(input: ParseStream) -> Result<If> {
    input.parse::<Token![if]>()?;
    let condition = if input.peek(Token![let]) {
        input.parse::<Token![let]>()?;
        let pattern = Pat::parse_multi_with_leading_vert(input)?;
        input.parse::<Token![=]>()?;
        Condition::Let(pattern, Expr::parse_without_eager_brace(input)?)
    } else {
        Condition::Plain(Expr::parse_without_eager_brace(input)?)
    };

    let inner;
    braced!(inner in input);
    let then = nodes(&inner)?;

    let otherwise = if input.peek(Token![else]) {
        input.parse::<Token![else]>()?;
        if input.peek(Token![if]) {
            Some(Box::new(Otherwise::If(if_chain(input)?)))
        } else {
            let inner;
            braced!(inner in input);
            Some(Box::new(Otherwise::Block(nodes(&inner)?)))
        }
    } else {
        None
    };

    Ok(If { condition, then, otherwise })
}

fn match_arms(input: ParseStream) -> Result<Match> {
    input.parse::<Token![match]>()?;
    let subject = Expr::parse_without_eager_brace(input)?;
    let body;
    braced!(body in input);

    let mut arms = Vec::new();
    while !body.is_empty() {
        let pattern = Pat::parse_multi_with_leading_vert(&body)?;
        let guard = if body.peek(Token![if]) {
            body.parse::<Token![if]>()?;
            Some(body.parse()?)
        } else {
            None
        };
        body.parse::<Token![=>]>()?;
        let inner;
        braced!(inner in body);
        arms.push((pattern, guard, nodes(&inner)?));
        let _ = body.parse::<Token![,]>();
    }

    Ok(Match { subject, arms })
}

fn for_loop(input: ParseStream) -> Result<For> {
    input.parse::<Token![for]>()?;
    let pattern = Pat::parse_multi_with_leading_vert(input)?;
    input.parse::<Token![in]>()?;
    let over = Expr::parse_without_eager_brace(input)?;
    let body;
    braced!(body in input);
    Ok(For { pattern, over, body: nodes(&body)? })
}
