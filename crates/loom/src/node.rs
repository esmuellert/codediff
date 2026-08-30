//! One frame's description, and what names a child across frames.

use std::any::TypeId;
use std::rc::Rc;

use ratatui::layout::Rect;

use crate::component::Component;
use crate::event::Listeners;
use crate::hook::Ref;
use crate::layout::Layout;
use crate::paint::Paint;
use crate::scope::{Scope, ScopeId};

/// One entry in the description of a frame.
///
/// Built by `rsx!` and thrown away after reconciliation. What survives a frame
/// is the scope tree. `Clone` is cheap: every piece inside is an `Rc`, a
/// function pointer or a `Copy` value, and it is what lets a component render
/// the children it was handed by reference.
#[derive(Clone)]
pub enum Node {
    /// An `if` with no `else`, or a component that decided to show nothing.
    Empty,
    /// Several nodes in one slot: a `for` body, or a component with two roots.
    Fragment(Vec<Node>),
    /// Something loom lays out and paints itself.
    Host(Box<Host>),
    /// Something whose shape is known only after running a function.
    Part(Box<Part>),
}

#[derive(Clone)]
pub struct Host {
    pub key: Option<Key>,
    pub name: &'static str,
    pub layout: Layout,
    /// Ink on cells. `None` for a container that only arranges its children.
    pub paint: Option<Rc<dyn Fn(&mut Paint<'_>)>>,
    /// Measured on the main axis when `Basis::Auto`. `None` measures as zero.
    pub measure: Option<fn(&Host, u16) -> (u16, u16)>,
    pub listeners: Listeners,
    pub focusable: bool,
    pub auto_focus: bool,
    /// Where to write this node's handle once it has a rectangle.
    pub node_ref: Option<Ref<Option<NodeHandle>>>,
    /// Painted instead of the children when they cannot meet their minimums.
    pub too_small: Option<Box<Node>>,
    pub children: Vec<Node>,
    /// Which way this host arranges its children.
    pub(crate) axis: crate::layout::Axis,
    /// `Text` carries its own string; `measure` reads it back through here.
    pub(crate) text: Option<Rc<str>>,
    pub(crate) style: ratatui::style::Style,
}

impl Default for Host {
    fn default() -> Self {
        Self {
            key: None,
            name: "Host",
            layout: Layout::default(),
            paint: None,
            measure: None,
            listeners: Listeners::default(),
            focusable: false,
            auto_focus: false,
            node_ref: None,
            too_small: None,
            children: Vec::new(),
            axis: crate::layout::Axis::Across,
            text: None,
            style: ratatui::style::Style::new(),
        }
    }
}

/// A node that has been laid out — what a `ref` holds once it points at
/// something. `Copy`, and valid until the node unmounts.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct NodeHandle {
    pub(crate) scope: ScopeId,
    /// Which host within that scope, in paint order.
    pub(crate) nth: u32,
}

#[derive(Clone)]
pub struct Part {
    pub key: Option<Key>,
    pub name: &'static str,
    pub type_id: TypeId,
    pub props: Rc<dyn std::any::Any>,
    /// `Component::render`, with the props type erased.
    pub render: fn(&dyn std::any::Any, &mut Scope) -> Node,
    /// Props equality, for `#[component(memo)]`. `None` means "always re-run".
    pub props_equal: Option<fn(&dyn std::any::Any, &dyn std::any::Any) -> bool>,
}

pub type Children = Vec<Node>;

impl Node {
    /// What `#[component]`'s `Element::build` calls.
    pub fn part<C: Component>(props: C::Props, key: Option<Key>) -> Node {
        Node::Part(Box::new(Part {
            key,
            name: C::NAME,
            type_id: TypeId::of::<C>(),
            props: Rc::new(props),
            render: |props, scope| {
                let props = props.downcast_ref::<C::Props>().expect("props of the declared type");
                C::render(props, scope)
            },
            props_equal: None,
        }))
    }

    /// The same, for a component whose props are compared before it re-runs.
    pub fn memo_part<C>(props: C::Props, key: Option<Key>) -> Node
    where
        C: Component,
        C::Props: PartialEq,
    {
        let mut node = Node::part::<C>(props, key);
        if let Node::Part(part) = &mut node {
            part.props_equal = Some(|a, b| match (a.downcast_ref::<C::Props>(), b.downcast_ref::<C::Props>()) {
                (Some(a), Some(b)) => a == b,
                _ => false,
            });
        }
        node
    }

    /// What a built-in host's `Element::build` calls.
    pub fn from_host(host: Host) -> Node {
        Node::Host(Box::new(host))
    }

    /// Flattens fragments away, so reconciliation sees one list of children.
    pub(crate) fn flatten(self, into: &mut Vec<Node>) {
        match self {
            Node::Empty => {}
            Node::Fragment(nodes) => {
                for node in nodes {
                    node.flatten(into);
                }
            }
            other => into.push(other),
        }
    }

}

/// What names a child across frames.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum Key {
    Number(u64),
    Text(Rc<str>),
}

impl From<u64> for Key {
    fn from(n: u64) -> Self {
        Key::Number(n)
    }
}
impl From<u32> for Key {
    fn from(n: u32) -> Self {
        Key::Number(u64::from(n))
    }
}
impl From<usize> for Key {
    fn from(n: usize) -> Self {
        Key::Number(n as u64)
    }
}
impl From<&str> for Key {
    fn from(s: &str) -> Self {
        Key::Text(Rc::from(s))
    }
}
impl From<String> for Key {
    fn from(s: String) -> Self {
        Key::Text(Rc::from(s.as_str()))
    }
}

/// What `rsx!` calls. Implemented by `#[component]` for your components and
/// by hand for the built-in hosts, so the macro emits one call for both.
pub trait Element: 'static {
    type Props: 'static;
    fn build(props: Self::Props, key: Option<Key>) -> Node;
}

impl From<Option<Node>> for Node {
    fn from(node: Option<Node>) -> Self {
        node.unwrap_or(Node::Empty)
    }
}

impl From<Vec<Node>> for Node {
    fn from(nodes: Vec<Node>) -> Self {
        Node::Fragment(nodes)
    }
}

impl From<()> for Node {
    fn from((): ()) -> Self {
        Node::Empty
    }
}

impl NodeHandle {
    /// The rectangle the last layout gave it.
    pub fn area(self) -> Rect {
        crate::current::with(|rt| rt.area_of(self)).unwrap_or(Rect::ZERO)
    }
    /// Take focus. A no-op if the node is not `focusable`.
    pub fn focus(self) {
        let focusable = crate::current::with(|rt| crate::event::focusable(rt, self)).unwrap_or(false);
        if focusable && let Some(held) = crate::current::held() {
            crate::event::move_focus(&held, Some(self));
        }
    }
    pub fn has_focus(self) -> bool {
        crate::current::with(|rt| rt.focused_node() == Some(self)).unwrap_or(false)
    }
    /// Whether `other` is this node or sits inside it.
    pub fn contains(self, other: NodeHandle) -> bool {
        crate::current::with(|rt| rt.node_contains(self, other)).unwrap_or(false)
    }
    /// A handle to an unmounted node answers `Rect::ZERO` and `false` rather
    /// than panicking.
    pub fn is_mounted(self) -> bool {
        crate::current::with(|rt| rt.is_alive(self.scope)).unwrap_or(false)
    }
}
