//! What survives a frame: the scope tree, the hook slots, focus, and where
//! every node landed.

use std::any::TypeId;
use std::collections::HashMap;
use std::rc::Rc;

use ratatui::layout::Rect;

use crate::event::Listeners;
use crate::frame::FrameNode;
use crate::hook::{EffectRun, Hooks};
use crate::node::{Key, NodeHandle};
use crate::scope::{Mounted, ScopeId, Scopes};

pub(crate) struct Runtime {
    pub scopes: Scopes,
    /// Hook slots, in a slab parallel to the scope slab.
    pub hooks: HashMap<ScopeId, Hooks>,
    pub root: Option<ScopeId>,
    /// Which context values each scope offers, and the version of each.
    pub offers: HashMap<ScopeId, Vec<Offer>>,
    /// Every host that got a rectangle last frame, deepest last.
    pub placed: Vec<FrameNode>,
    pub focused: Option<NodeHandle>,
    pub captured: Option<NodeHandle>,
    /// The node whose listener is running, so `capture_pointer` knows what to
    /// capture.
    pub handling: Option<NodeHandle>,
    /// Set by a state write, a store notification, or a resize.
    pub dirty: Vec<ScopeId>,
    /// Effects waiting for the paint to finish.
    pub effects: Vec<EffectRun>,
    /// Effects waiting for layout to finish, run before paint.
    pub layout_effects: Vec<EffectRun>,
    /// Which effect slot is open, so `promise` and `observable` know their
    /// address. `None` outside an effect body.
    pub running_effect: Option<(ScopeId, u16, u64)>,
    /// How many component functions ran during the last frame.
    pub renders: usize,
    /// How many times a component of each name has run, for tests.
    pub renders_by_name: HashMap<&'static str, usize>,
    pub rounds: usize,
    /// Bumped whenever any context value changes, so a memoised reader that
    /// read one knows to run again.
    pub context_version: u64,
    /// Set by `use_exit`'s closure. The loop reads it and stops.
    pub exit: bool,
}

/// One context value a scope offers to everything below it.
pub(crate) struct Offer {
    pub context: TypeId,
    pub value: Rc<dyn std::any::Any>,
    pub version: u64,
}

impl Runtime {
    pub fn new() -> Self {
        Self {
            scopes: Scopes::new(),
            hooks: HashMap::new(),
            root: None,
            offers: HashMap::new(),
            placed: Vec::new(),
            focused: None,
            captured: None,
            handling: None,
            dirty: Vec::new(),
            effects: Vec::new(),
            layout_effects: Vec::new(),
            running_effect: None,
            renders: 0,
            renders_by_name: HashMap::new(),
            rounds: 0,
            context_version: 0,
            exit: false,
        }
    }

    pub fn mount(
        &mut self,
        name: &'static str,
        type_id: TypeId,
        key: Option<Key>,
        parent: Option<ScopeId>,
        props: Rc<dyn std::any::Any>,
        render: fn(&dyn std::any::Any, &mut crate::scope::Scope) -> crate::node::Node,
        props_equal: Option<fn(&dyn std::any::Any, &dyn std::any::Any) -> bool>,
    ) -> ScopeId {
        let id = self.scopes.insert(Mounted {
            name,
            type_id,
            key,
            parent,
            children: Vec::new(),
            props,
            render,
            props_equal,
            produced: Vec::new(),
            dirty: true,
            renders: 0,
            reads: Vec::new(),
        });
        self.hooks.insert(id, Hooks::new());
        if let Some(parent) = parent
            && let Some(up) = self.scopes.get_mut(parent)
        {
            up.children.push(id);
        }
        id
    }

    /// R6.2 — deepest first, so a child's cleanup sees a parent that is still
    /// there.
    pub fn unmount(&mut self, id: ScopeId) {
        let children = self.scopes.get(id).map(|m| m.children.clone()).unwrap_or_default();
        for child in children {
            self.unmount(child);
        }

        if let Some(hooks) = self.hooks.remove(&id) {
            hooks.cleanup();
        }
        self.offers.remove(&id);
        self.dirty.retain(|&d| d != id);
        self.effects.retain(|e| e.scope != id);
        self.layout_effects.retain(|e| e.scope != id);
        if self.focused.is_some_and(|f| f.scope == id) {
            self.focused = None;
        }
        if self.captured.is_some_and(|c| c.scope == id) {
            self.captured = None;
        }

        if let Some(mounted) = self.scopes.remove(id)
            && let Some(parent) = mounted.parent
            && let Some(up) = self.scopes.get_mut(parent)
        {
            up.children.retain(|&c| c != id);
        }
    }

    pub fn name_of(&self, id: ScopeId) -> &'static str {
        self.scopes.get(id).map_or("?", |m| m.name)
    }

    pub fn is_alive(&self, id: ScopeId) -> bool {
        self.scopes.is_alive(id)
    }

    /// Marks a scope, and every ancestor, so the next frame reaches it.
    ///
    /// A parent that is not marked hands back last frame's subtree without
    /// looking inside it, so a child that changed would never be reached. A
    /// memoised child whose props still match is left clean and keeps its own
    /// subtree, which is what stops this walking the whole tree.
    pub fn mark(&mut self, id: ScopeId) {
        if !self.scopes.is_alive(id) {
            return;
        }
        let mut at = Some(id);
        while let Some(scope) = at {
            let Some(mounted) = self.scopes.get_mut(scope) else { break };
            if mounted.dirty && scope != id {
                // Everything above is marked already.
                break;
            }
            mounted.dirty = true;
            at = mounted.parent;
        }
        if !self.dirty.contains(&id) {
            self.dirty.push(id);
        }
    }

    /// R6.3 — a marked scope's ancestors are walked so the frame reaches it,
    /// but they are not themselves re-run.
    pub fn mark_all(&mut self) {
        if let Some(root) = self.root {
            self.mark_subtree(root);
        }
    }

    fn mark_subtree(&mut self, id: ScopeId) {
        self.mark(id);
        let children = self.scopes.get(id).map(|m| m.children.clone()).unwrap_or_default();
        for child in children {
            self.mark_subtree(child);
        }
    }

    pub fn needs_draw(&self) -> bool {
        !self.dirty.is_empty()
    }

    pub fn area_of(&self, node: NodeHandle) -> Rect {
        self.placed
            .iter()
            .find(|p| p.scope == node.scope && p.nth == node.nth)
            .map_or(Rect::ZERO, |p| p.area)
    }

    pub fn focused_node(&self) -> Option<NodeHandle> {
        self.focused
    }


    /// Whether `other` is `outer` or sits inside it, by walking the placed
    /// tree upward.
    pub fn node_contains(&self, outer: NodeHandle, other: NodeHandle) -> bool {
        if outer == other {
            return true;
        }
        let Some(start) = self.placed.iter().position(|p| p.scope == other.scope && p.nth == other.nth)
        else {
            return false;
        };
        let mut at = self.placed[start].parent;
        while let Some(i) = at {
            let up = &self.placed[i];
            if up.scope == outer.scope && up.nth == outer.nth {
                return true;
            }
            at = up.parent;
        }
        false
    }

    /// The nearest offer of `context` at or above `from`, and its version.
    pub fn read_context(&self, from: ScopeId, context: TypeId) -> Option<(Rc<dyn std::any::Any>, u64)> {
        let mut at = Some(from);
        while let Some(id) = at {
            if let Some(offers) = self.offers.get(&id)
                && let Some(offer) = offers.iter().find(|o| o.context == context)
            {
                return Some((Rc::clone(&offer.value), offer.version));
            }
            at = self.scopes.get(id).and_then(|m| m.parent);
        }
        None
    }

    pub fn listeners_of(&self, node: NodeHandle) -> Option<&Listeners> {
        self.placed
            .iter()
            .find(|p| p.scope == node.scope && p.nth == node.nth)
            .map(|p| &p.listeners)
    }
}
