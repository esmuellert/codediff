//! The built-in hosts. Everything loom lays out and paints itself.

use std::rc::Rc;

use ratatui::style::Style;

use super::Paint;
use crate::event::{Bubble, Listeners};
use crate::hook::Ref;
use crate::layout::{Axis, Layout};
use crate::node::{Children, Element, Host, Key, Node, NodeHandle};
use crate::scroll::{ScrollHandle, ScrollView};

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

/// A clipped viewport whose children move with one two-dimensional offset.
pub struct Scroll;

pub struct ScrollProps {
    pub layout: Layout,
    pub view: ScrollView,
    /// When present, the host consumes wheel movement before it bubbles.
    pub handle: Option<ScrollHandle>,
    /// Which wheel axes this host owns.
    pub horizontal: bool,
    pub vertical: bool,
    /// Number of content cells moved by one wheel notch.
    pub wheel_step: i32,
    /// Optional extent and origin for virtualized children.
    pub content_width: Option<u32>,
    pub content_height: Option<u32>,
    pub content_offset: crate::scroll::ScrollOffset,
    pub listeners: Listeners,
    pub focusable: bool,
    pub auto_focus: bool,
    pub node_ref: NodeRef,
    pub children: Children,
}

impl Default for ScrollProps {
    fn default() -> Self {
        Self {
            layout: Layout::default(),
            view: ScrollView::default(),
            handle: None,
            horizontal: true,
            vertical: true,
            wheel_step: 1,
            content_width: None,
            content_height: None,
            content_offset: crate::scroll::ScrollOffset::ZERO,
            listeners: Listeners::default(),
            focusable: false,
            auto_focus: false,
            node_ref: None,
            children: Vec::new(),
        }
    }
}

impl Element for Scroll {
    type Props = ScrollProps;

    fn build(mut props: Self::Props, key: Option<Key>) -> Node {
        if let Some(handle) = props.handle.clone() {
            let horizontal = props.horizontal;
            let vertical = props.vertical;
            let wheel_step = props.wheel_step;
            props.listeners = props.listeners.on_wheel(move |wheel| {
                let dx = horizontal
                    .then_some(wheel.horizontal.saturating_mul(wheel_step))
                    .unwrap_or(0);
                let dy = vertical
                    .then_some(wheel.vertical.saturating_mul(wheel_step))
                    .unwrap_or(0);
                if dx == 0 && dy == 0 {
                    Bubble::Continue
                } else {
                    handle.scroll_by(dx, dy);
                    Bubble::Stop
                }
            });
        }
        props.layout.clip = true;
        Node::from_host(Host {
            key,
            name: "Scroll",
            layout: props.layout,
            listeners: props.listeners,
            focusable: props.focusable,
            auto_focus: props.auto_focus,
            node_ref: props.node_ref,
            children: props.children,
            scroll: Some(props.view),
            scroll_axes: (props.horizontal, props.vertical),
            scroll_content_width: props.content_width,
            scroll_content_height: props.content_height,
            scroll_content_offset: props.content_offset,
            axis: Axis::Down,
            ..Host::default()
        })
    }
}

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
