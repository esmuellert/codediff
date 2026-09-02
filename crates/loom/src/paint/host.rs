//! The built-in hosts. Everything loom lays out and paints itself.

use std::rc::Rc;

use ratatui::style::Style;

use super::Paint;
use crate::event::Listeners;
use crate::hook::Ref;
use crate::layout::{Axis, Layout};
use crate::node::{Children, Element, Host, Key, Node, NodeHandle};

/// Every host carries this, and `rsx!` spells it `ref`.
type NodeRef = Option<Ref<Option<NodeHandle>>>;

macro_rules! container {
    ($name:ident, $props:ident, $axis:expr, $doc:literal) => {
        #[doc = $doc]
        pub struct $name;

        #[derive(Default)]
        pub struct $props {
            pub layout: Layout,
            pub listeners: Listeners,
            pub focusable: bool,
            pub auto_focus: bool,
            pub too_small: Option<Node>,
            pub node_ref: NodeRef,
            pub children: Children,
        }

        impl Element for $name {
            type Props = $props;
            fn build(props: Self::Props, key: Option<Key>) -> Node {
                Node::from_host(Host {
                    key,
                    name: stringify!($name),
                    layout: props.layout,
                    listeners: props.listeners,
                    focusable: props.focusable,
                    auto_focus: props.auto_focus,
                    node_ref: props.node_ref,
                    too_small: props.too_small.map(Box::new),
                    children: props.children,
                    axis: $axis,
                    ..Host::default()
                })
            }
        }
    };
}

container!(Row, RowProps, Axis::Across, "Children across.");
container!(Column, ColumnProps, Axis::Down, "Children down.");
container!(
    Stack,
    StackProps,
    Axis::Over,
    "Children painted over one another, in declaration order."
);

/// Empty space. `Gap { layout: Layout { grow: 1, .. } }` pushes what follows
/// away.
pub struct Gap;

#[derive(Default)]
pub struct GapProps {
    pub layout: Layout,
    pub node_ref: NodeRef,
}

impl Element for Gap {
    type Props = GapProps;
    fn build(props: Self::Props, key: Option<Key>) -> Node {
        Node::from_host(Host {
            key,
            name: "Gap",
            layout: props.layout,
            node_ref: props.node_ref,
            ..Host::default()
        })
    }
}

/// One cell of `symbol`, repeated down or across.
pub struct Divider;

pub struct DividerProps {
    pub layout: Layout,
    pub symbol: &'static str,
    pub style: Style,
    pub node_ref: NodeRef,
}

impl Default for DividerProps {
    fn default() -> Self {
        Self {
            layout: Layout::default(),
            symbol: "\u{2502}",
            style: Style::new(),
            node_ref: None,
        }
    }
}

impl Element for Divider {
    type Props = DividerProps;
    fn build(props: Self::Props, key: Option<Key>) -> Node {
        let symbol = props.symbol;
        let style = props.style;
        Node::from_host(Host {
            key,
            name: "Divider",
            layout: props.layout,
            node_ref: props.node_ref,
            paint: Some(Rc::new(move |brush: &mut Paint<'_>| {
                let area = brush.area();
                for y in area.top()..area.bottom() {
                    for x in area.left()..area.right() {
                        brush.set(x, y, symbol, style);
                    }
                }
            })),
            ..Host::default()
        })
    }
}

/// Text this program generated. Measures itself.
pub struct Text;

#[derive(Default)]
pub struct TextProps {
    pub layout: Layout,
    pub text: Rc<str>,
    pub style: Style,
    pub node_ref: NodeRef,
}

impl Element for Text {
    type Props = TextProps;
    fn build(props: Self::Props, key: Option<Key>) -> Node {
        Node::from_host(Host {
            key,
            name: "Text",
            layout: props.layout,
            node_ref: props.node_ref,
            text: Some(props.text),
            style: props.style,
            // The one measurable host; `reconcile` swaps in the real function.
            measure: Some(|_, _| (0, 1)),
            ..Host::default()
        })
    }
}

/// The escape hatch: a rectangle handed to a painting function.
pub struct Canvas;

pub struct CanvasProps {
    pub layout: Layout,
    pub listeners: Listeners,
    pub focusable: bool,
    pub auto_focus: bool,
    pub node_ref: NodeRef,
    pub paint: Rc<dyn Fn(&mut Paint<'_>)>,
}

impl Default for CanvasProps {
    fn default() -> Self {
        Self {
            // A canvas is where unbounded painting would otherwise happen.
            layout: Layout {
                clip: true,
                ..Layout::default()
            },
            listeners: Listeners::default(),
            focusable: false,
            auto_focus: false,
            node_ref: None,
            paint: Rc::new(|_| {}),
        }
    }
}

impl Element for Canvas {
    type Props = CanvasProps;
    fn build(props: Self::Props, key: Option<Key>) -> Node {
        Node::from_host(Host {
            key,
            name: "Canvas",
            layout: props.layout,
            listeners: props.listeners,
            focusable: props.focusable,
            auto_focus: props.auto_focus,
            node_ref: props.node_ref,
            paint: Some(props.paint),
            ..Host::default()
        })
    }
}
