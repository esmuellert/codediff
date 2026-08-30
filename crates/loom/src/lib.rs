//! A React for the terminal: components, hooks, flexbox, and a paint pass.

mod component;
mod current;
mod event;
mod frame;
mod hook;
mod layout;
mod node;
mod paint;
mod reconcile;
mod run;
mod runtime;
mod scope;
mod screen;
mod tree;
pub mod testing;

pub use component::Component;
pub use event::{
    Bubble, Focus, Listeners, Mouse, capture_pointer, focus_next, focus_previous, release_pointer,
};
pub use hook::{
    Always, Cleanup, Context, ExternalStore, Notify, Observable, Observer, Promise, Ref, Resolver,
    SetState, Size, Snapshot, Subscription, observable, promise, use_context, use_effect,
    use_layout_effect, use_measure, use_memo, use_ref, use_state, use_sync_external_store,
};
pub use hook::use_exit;
/// What `context!`'s `Component::render` calls. Not API: the way to offer a
/// value is to write the provider element.
#[doc(hidden)]
pub use hook::offer;
pub use layout::{Basis, Edges, Layout};
pub use node::{Children, Element, Key, Node, NodeHandle};
pub use paint::{
    Canvas, CanvasProps, Column, ColumnProps, Divider, DividerProps, Gap, GapProps, Paint, Row,
    RowProps, Stack, StackProps, Text, TextProps,
};
pub use run::{Flow, deliver_input, run};
pub use scope::{Scope, ScopeId};
pub use screen::{Screen, restore};
pub use tree::Tree;

pub use loom_macros::{component, context, rsx};

/// Re-exported so a consumer builds against the same versions we did.
pub use crokey;
pub use crossterm;
pub use ratatui;
