//! Matching this frame's description against the live scope tree.
//!
//! The runtime is borrowed in short bursts and never across a component's own
//! function, because that function calls hooks that reach the runtime too.

use std::cell::RefCell;
use std::rc::Rc;

use crate::node::{Host, Key, Node, NodeHandle};
use crate::runtime::Runtime;
use crate::scope::{Scope, ScopeId};

/// The runtime, as everything here holds it.
pub(crate) type RuntimeRef = Rc<RefCell<Runtime>>;

/// A host that survived reconciliation, with the scope that produced it.
///
/// `Rc` throughout, so a clean component's subtree is handed back by cloning
/// rather than by running the component again.
#[derive(Clone)]
pub(crate) struct Fiber {
    pub scope: ScopeId,
    pub host_desc: Rc<HostDesc>,
    pub children: Vec<Fiber>,
    /// Painted instead of the children when they cannot meet their minimums.
    pub too_small: Option<Rc<Vec<Fiber>>>,
}

/// One host's own properties, with its children lifted out.
pub struct HostDesc {
    pub name: &'static str,
    pub layout: crate::layout::Layout,
    pub paint: Option<Rc<dyn Fn(&mut crate::paint::Paint<'_>)>>,
    pub measure: Option<fn(&HostDesc, u16) -> (u16, u16)>,
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
    // R6.3 / I12 — a component runs when its props changed, its own state
    // changed, or its parent ran. Otherwise last frame's subtree stands.
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
    let Some((props, render, name, old)) = ready else { return Vec::new() };

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

    // Nothing is borrowed here, so the component's hooks can reach the
    // runtime while its own function is on the stack.
    let mut token = Scope { id: scope };
    let produced = render(props.as_ref(), &mut token);

    {
        let mut rt = held.borrow_mut();
        if let Some(hooks) = rt.hooks.get_mut(&scope) {
            crate::hook::finish_render(name, hooks);
        }
    }

    let mut cursor = Cursor { old: old.clone(), used: Vec::new(), position: 0 };
    let out = expand(held, produced, scope, &mut cursor);

    {
        let mut rt = held.borrow_mut();
        // R6.2 — anything this frame did not name is gone, deepest first.
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
                                let same = mounted
                                    .props_equal
                                    .is_some_and(|eq| eq(mounted.props.as_ref(), part.props.as_ref()));
                                mounted.props = Rc::clone(&part.props);
                                mounted.render = part.render;
                                mounted.props_equal = part.props_equal;
                                if !same {
                                    mounted.dirty = true;
                                }
                            }
                            scope
                        }
                        None => rt.mount(
                            part.name,
                            part.type_id,
                            part.key.clone(),
                            Some(owner),
                            Rc::clone(&part.props),
                            part.render,
                            part.props_equal,
                        ),
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
            measure: host.measure.map(|_| measure_text as fn(&HostDesc, u16) -> (u16, u16)),
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

/// R5.3.1 — the one `measure` in the crate.
fn measure_text(desc: &HostDesc, _room: u16) -> (u16, u16) {
    match &desc.text {
        Some(text) => (ratatui::text::Span::styled(text.as_ref(), desc.style).width() as u16, 1),
        None => (0, 1),
    }
}

/// R6.1.1 — a key names one child wherever it moved to.
fn keyed(rt: &Runtime, cursor: &Cursor, key: &Key, type_id: std::any::TypeId) -> Option<ScopeId> {
    cursor.old.iter().copied().find(|&id| {
        !cursor.used.contains(&id)
            && rt
                .scopes
                .get(id)
                .is_some_and(|m| m.key.as_ref() == Some(key) && m.type_id == type_id)
    })
}

/// R6.1.2 — without a key, the nth unkeyed child.
fn positional(rt: &Runtime, cursor: &mut Cursor, type_id: std::any::TypeId) -> Option<ScopeId> {
    let at = cursor.position;
    cursor.position += 1;
    cursor
        .old
        .iter()
        .copied()
        .filter(|&id| rt.scopes.get(id).is_some_and(|m| m.key.is_none()))
        .nth(at)
        // R6.1.3 — a different component at the same place starts fresh.
        .filter(|&id| rt.scopes.get(id).is_some_and(|m| m.type_id == type_id))
        .filter(|id| !cursor.used.contains(id))
}
