//! Reconciles frame descriptions with the live scope tree.

use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
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
    pub children: Rc<Vec<Fiber>>,
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
    pub scroll: Option<crate::scroll::ScrollView>,
    pub scroll_axes: (bool, bool),
    pub scroll_content_width: Option<u32>,
    pub scroll_content_height: Option<u32>,
    pub scroll_content_offset: crate::scroll::ScrollOffset,
    pub text: Option<Rc<str>>,
    pub style: ratatui::style::Style,
}

/// Where a component's children are matched from, as one list is walked.
struct Cursor {
    /// Keyed children indexed once by their sibling key and component type.
    keyed: HashMap<(Key, std::any::TypeId), Vec<ScopeId>>,
    /// The next old unkeyed child to compare.
    unkeyed: Vec<ScopeId>,
    /// Which old children this frame has already claimed.
    used: HashSet<ScopeId>,
    /// The order in which surviving children are written back to the parent.
    next: Vec<ScopeId>,
    /// How many unkeyed children have been matched so far.
    position: usize,
    /// The next candidate in each keyed sibling bucket.
    keyed_position: HashMap<(Key, std::any::TypeId), usize>,
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

    let mut cursor = Cursor::new(held, old);
    let out = expand(held, produced, scope, &mut cursor);

    {
        let mut rt = held.borrow_mut();
        // Detach and unmount children not produced this frame in one sibling
        // pass, so removing many old children stays linear.
        rt.unmount_children_except(scope, &cursor.used);
        if let Some(mounted) = rt.scopes.get_mut(scope) {
            mounted.children = cursor.next;
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
                        Some(key) => cursor.keyed(key, part.type_id),
                        None => cursor.positional(&rt, part.type_id),
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

                cursor.used.insert(scope);
                cursor.next.push(scope);
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
            scroll: host.scroll,
            scroll_axes: host.scroll_axes,
            scroll_content_width: host.scroll_content_width,
            scroll_content_height: host.scroll_content_height,
            scroll_content_offset: host.scroll_content_offset,
            text: host.text,
            style: host.style,
        }),
        children: Rc::new(inner),
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
impl Cursor {
    fn new(held: &RuntimeRef, old: Vec<ScopeId>) -> Self {
        let mut keyed = HashMap::new();
        let mut unkeyed = Vec::new();
        {
            let rt = held.borrow();
            for &id in &old {
                let Some(mounted) = rt.scopes.get(id) else {
                    continue;
                };
                if let Some(key) = &mounted.key {
                    keyed
                        .entry((key.clone(), mounted.type_id))
                        .or_insert_with(Vec::new)
                        .push(id);
                } else {
                    unkeyed.push(id);
                }
            }
        }
        Self {
            keyed,
            unkeyed,
            used: HashSet::new(),
            next: Vec::new(),
            position: 0,
            keyed_position: HashMap::new(),
        }
    }

    /// Matches a keyed child from its sibling bucket. Duplicate keys reuse
    /// old scopes in source order, which keeps the operation deterministic.
    fn keyed(&mut self, key: &Key, type_id: std::any::TypeId) -> Option<ScopeId> {
        let bucket_key = (key.clone(), type_id);
        let at = self.keyed_position.get(&bucket_key).copied().unwrap_or(0);
        let id = self
            .keyed
            .get(&bucket_key)
            .and_then(|bucket| bucket.get(at))
            .copied();
        if id.is_some() {
            self.keyed_position.insert(bucket_key, at + 1);
        }
        id
    }

    /// Matches an unkeyed child by its unkeyed sibling position.
    fn positional(&mut self, rt: &Runtime, type_id: std::any::TypeId) -> Option<ScopeId> {
        let id = self.unkeyed.get(self.position).copied();
        self.position += 1;
        id.filter(|&id| rt.scopes.get(id).is_some_and(|m| m.type_id == type_id))
    }
}
