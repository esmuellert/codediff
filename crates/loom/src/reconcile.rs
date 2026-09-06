//! Reconciles frame descriptions with the live scope tree.

use std::cell::RefCell;
use std::rc::Rc;

use crate::node::{Host, Key, MeasureFn, Node, NodeHandle, PaintFn};
use crate::runtime::Runtime;
use crate::scope::{Scope, ScopeId};

/// The runtime, as everything here holds it.
pub(crate) type RuntimeRef = Rc<RefCell<Runtime>>;

/// A host that survived reconciliation.
#[derive(Clone)]
pub(crate) struct Fiber {
    pub scope: ScopeId,
    pub host_desc: Rc<HostDesc>,
    pub children: Vec<Fiber>,
    /// Fallback painted when children cannot meet their minimums.
    pub too_small: Option<Rc<Vec<Fiber>>>,
}

/// One host's own properties, with its children lifted out.
pub struct HostDesc {
    pub name: &'static str,
    pub layout: crate::layout::Layout,
    pub paint: Option<Rc<PaintFn>>,
    pub measure: Option<MeasureFn<HostDesc>>,
    pub listeners: crate::event::Listeners,
    pub focusable: bool,
    pub auto_focus: bool,
    pub node_ref: Option<crate::hook::Ref<Option<NodeHandle>>>,
    pub axis: crate::layout::Axis,
    pub text: Option<Rc<str>>,
    pub style: ratatui::style::Style,
}

/// Where a component's children are matched from, as one list is walked.
struct Cursor {
    /// The scope's children as they were last frame.
    old: Vec<ScopeId>,
    /// Which of them this frame has already claimed.
    used: Vec<ScopeId>,
    /// How many unkeyed children have been matched so far.
    position: usize,
}

/// Runs the root and everything under it that needs running.
pub(crate) fn frame(held: &RuntimeRef, root: ScopeId) -> Vec<Fiber> {
    run(held, root)
}

/// Runs one component, or hands back what it produced last frame.
fn run(held: &RuntimeRef, scope: ScopeId) -> Vec<Fiber> {
    // Run dirty components; reuse clean subtrees.
    let ready = {
        let rt = held.borrow();
        match rt.scopes.get(scope) {
            Some(mounted) if !mounted.dirty => return mounted.produced.clone(),
            Some(mounted) => Some((
                Rc::clone(&mounted.props),
                mounted.render,
                mounted.name,
                mounted.children.clone(),
            )),
            None => None,
        }
    };
    let Some((props, render, name, old)) = ready else {
        return Vec::new();
    };

    {
        let mut rt = held.borrow_mut();
        if let Some(hooks) = rt.hooks.get_mut(&scope) {
            hooks.index = 0;
        }
        if let Some(mounted) = rt.scopes.get_mut(scope) {
            mounted.dirty = false;
            mounted.renders += 1;
            mounted.reads.clear();
        }
        rt.renders += 1;
        *rt.renders_by_name.entry(name).or_insert(0) += 1;
    }

    // Release the runtime borrow before calling the component.
    let mut token = Scope { id: scope };
    let produced = render(props.as_ref(), &mut token);

    {
        let mut rt = held.borrow_mut();
        if let Some(hooks) = rt.hooks.get_mut(&scope) {
            crate::hook::finish_render(name, hooks);
        }
    }

    let mut cursor = Cursor {
        old: old.clone(),
        used: Vec::new(),
        position: 0,
    };
    let out = expand(held, produced, scope, &mut cursor);

    {
        let mut rt = held.borrow_mut();
        // Unmount children not produced this frame.
        for gone in old {
            if !cursor.used.contains(&gone) {
                rt.unmount(gone);
            }
        }
        if let Some(mounted) = rt.scopes.get_mut(scope) {
            mounted.children = cursor.used;
            mounted.produced = out.clone();
        }
    }

    out
}

/// Turns one node into the hosts it stands for, mounting or matching every
/// component it names.
fn expand(held: &RuntimeRef, node: Node, owner: ScopeId, cursor: &mut Cursor) -> Vec<Fiber> {
    let mut flat = Vec::new();
    node.flatten(&mut flat);

    let mut out = Vec::new();
    for child in flat {
        match child {
            Node::Host(host) => out.push(host_into(held, *host, owner, cursor)),
            Node::Part(part) => {
                let scope = {
                    let mut rt = held.borrow_mut();
                    let matched = match &part.key {
                        Some(key) => keyed(&rt, cursor, key, part.type_id),
                        None => positional(&rt, cursor, part.type_id),
                    };
                    match matched {
                        Some(scope) => {
                            if let Some(mounted) = rt.scopes.get_mut(scope) {
                                // A memoised component whose props match keeps
                                // last frame's subtree.
                                let same = mounted.props_equal.is_some_and(|eq| {
                                    eq(mounted.props.as_ref(), part.props.as_ref())
                                });
                                mounted.props = Rc::clone(&part.props);
                                mounted.render = part.render;
                                mounted.props_equal = part.props_equal;
                                if !same {
                                    mounted.dirty = true;
                                }
                            }
                            scope
                        }
                        None => rt.mount(*part, Some(owner)),
                    }
                };

                cursor.used.push(scope);
                out.extend(run(held, scope));
            }
            Node::Empty | Node::Fragment(_) => {}
        }
    }
    out
}

fn host_into(held: &RuntimeRef, mut host: Host, owner: ScopeId, cursor: &mut Cursor) -> Fiber {
    let children = std::mem::take(&mut host.children);
    let too_small = host.too_small.take();

    let mut inner = Vec::new();
    for child in children {
        inner.extend(expand(held, child, owner, cursor));
    }

    let too_small = too_small.map(|node| Rc::new(expand(held, *node, owner, cursor)));

    Fiber {
        scope: owner,
        host_desc: Rc::new(HostDesc {
            name: host.name,
            layout: host.layout,
            paint: host.paint,
            measure: host.measure.map(|_| measure_text as MeasureFn<HostDesc>),
            listeners: host.listeners,
            focusable: host.focusable,
            auto_focus: host.auto_focus,
            node_ref: host.node_ref,
            axis: host.axis,
            text: host.text,
            style: host.style,
        }),
        children: inner,
        too_small,
    }
}

/// Measures a text host.
fn measure_text(desc: &HostDesc, _room: u16) -> (u16, u16) {
    match &desc.text {
        Some(text) => (
            ratatui::text::Span::styled(text.as_ref(), desc.style).width() as u16,
            1,
        ),
        None => (0, 1),
    }
}

/// Matches keyed children by key and type.
fn keyed(rt: &Runtime, cursor: &Cursor, key: &Key, type_id: std::any::TypeId) -> Option<ScopeId> {
    cursor.old.iter().copied().find(|&id| {
        !cursor.used.contains(&id)
            && rt
                .scopes
                .get(id)
                .is_some_and(|m| m.key.as_ref() == Some(key) && m.type_id == type_id)
    })
}

/// Matches unkeyed children by position.
fn positional(rt: &Runtime, cursor: &mut Cursor, type_id: std::any::TypeId) -> Option<ScopeId> {
    let at = cursor.position;
    cursor.position += 1;
    cursor
        .old
        .iter()
        .copied()
        .filter(|&id| rt.scopes.get(id).is_some_and(|m| m.key.is_none()))
        .nth(at)
        // A type change starts a new scope.
        .filter(|&id| rt.scopes.get(id).is_some_and(|m| m.type_id == type_id))
        .filter(|id| !cursor.used.contains(id))
}
