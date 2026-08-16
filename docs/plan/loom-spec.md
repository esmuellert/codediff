# `loom` — implementation specification

A render framework for `codediff`: components, hooks, layout, paint, events,
worker replies. Two new crates, `loom` and `loom-macros`. Nothing in this
document is built yet.

Read it as the answer to "what exactly do I write". Every rule is numbered and
carries the name of the test that proves it. Every panic carries its message.

---

## 1. Scope and non-goals

### 1.1 In

| | |
|---|---|
| element tree | a value describing one frame, built by `rsx!`, thrown away after reconciliation |
| reconciliation | matching this frame's description against the live instance tree, so state has an owner |
| function components | `#[component] fn Name(scope: &mut Scope, …) -> Node` |
| hooks | `use_state`, `use_ref`, `use_memo`, `use_effect`, `use_layout_effect`, `use_context`, `use_sync_external_store` |
| layout | one axis per container, integer terminal cells, two passes |
| paint | a top-down walk that writes into `ratatui::buffer::Buffer` |
| events | hit-test by rectangle, focus by scope, bubbling, pointer capture |
| worker replies | `Promise<T>` and `Observable<T>`, refused when stale |
| `rsx!` | a proc macro over Rust syntax, props as a struct literal |
| testing | `loom::testing::Harness` — mount, draw, read the screen as text |

### 1.2 Out

| | why |
|---|---|
| a cell diff | `ratatui::Terminal` already diffs the previous and current buffers and writes only the changed cells |
| a mutation stream (`WriteMutations`) | it exists to talk to a retained host tree over a slow boundary; we write into a `Vec<Cell>` in-process |
| taffy | we lay out about a dozen boxes over integer cells; `f32` geometry rounded back to cells is where column drift comes from |
| a scroll container | a diff viewer must never lay out 100,000 rows to show 40; `Viewport` holds `top` and the component renders the slice |
| a capture phase | pointer capture covers the one drag this program has |
| error boundaries | a panic in a painter belongs in a backtrace with the terminal restored, which `Screen`'s `Drop` already does |
| `use_callback` | React documents it as `useMemo(() => fn, deps)`, and a closure is a value, so `use_memo` already is it |
| `use(Context)` | React 19's conditional read; `use_context` is `useContext`, which cannot be conditional either, and no component here wants to be |
| `useId`, `useReducer`, suspense, portals, hot reload, SSR | no user here |
| struct components, borrowed props, GATs | locked out by decisions 7 and 8 |
| text wrapping | no pane in this program wraps |

### 1.3 Settled disagreements

The two prior designs disagreed on these. The choice is made; the reason is one
sentence.

| question | choice | why |
|---|---|---|
| crate name | `loom`, macros in `loom-macros` | one plain English word, like `align` and `syntax`; both crates are `publish = false` so the name on crates.io is a coincidence |
| layout model | CSS flexbox, implemented here in whole cells | the model is proven and documented; the crate is not the model. `f32` rounded back to cells is where column drift comes from, and CSS cannot say "if this does not fit, do not draw it" — so we take the algorithm and replace overflow with refusal (§5.6) |
| state access in listeners | a render snapshot plus a `SetState<T>` handle | `let (cursor, set_cursor) = use_state(…)` and `set_cursor(&|n| n + 1)` are React's value-and-setter model in Rust |
| worker replies | typed handles with a generation check | a `TaskId` token says where to deliver; a generation says whether it is still wanted |
| render parameter | `scope` | `cx` reads as "context", and context is a different thing here |
| state setter | `SetState<T>`, called like a function, `Copy` | one way to write, so there is no second name to choose; the runtime keeps an 8-byte writer per state slot, which is what lets the handle stay `Copy` |
| mutable value without redraw | `use_ref`, returning `Ref<T>` | state stays immutable and React-like; large models and imperative caches have an explicit home, and a `Ref<Option<NodeHandle>>` is React's other use for a ref |
| context storage | parent walk keyed by the context's marker `TypeId`, read recorded as `(TypeId, version)` | the marker type is the identity React gets from object identity, and the recorded version is the twenty lines that keep `memo` honest |
| provider syntax | the context is the element: `ThemeContext { value: dark, … }` | React's `<ThemeContext value={dark}>`; one declaration is both the key a reader names and the element a provider writes |
| listeners in `rsx!` | one `listeners:` prop built by a chain | one prop, one type, no macro magic, and an unknown handler is an unknown method |
| keyed lists | `HashMap<Key, ScopeId>`, no longest-increasing-subsequence | LIS pays for moving DOM nodes; we reorder ids in a `Vec` and repaint |
| hidden panes | `Layout::hidden`, mounted and out of layout | hiding the diff on a narrow screen must not throw its viewport away |
| `Display`/`Align`/`Justify` | not built | nothing in this program centres a box on its cross axis |

---

## 2. Vocabulary

One word per idea. Where an idea already has a name in ratatui, Neovim or this
repository, that name is used.

| word | means | not |
|---|---|---|
| **node** | one entry in the description of a frame: `Empty`, `Fragment`, `Host` or `Part` | element, vnode |
| **host** | a node `loom` places and paints itself: `Row`, `Column`, `Stack`, `Gap`, `Divider`, `Text`, `Canvas` | widget |
| **part** | one appearance of a component in one frame | instance |
| **component** | a function you write, with props and hooks | |
| **scope** | the live instance of a component: its identity, its hook slots, its rectangle | fiber, instance |
| **slot** | one hook's storage inside a scope | |
| **state** | a value that survives redraws; a component reads one snapshot of it per render | live mutable handle |
| **setter** | the `Copy` handle a component calls to write its state | state |
| **ref** | a mutable hook value with silent writes | state |
| **props** | a `'static` struct, one field per parameter | attributes |
| **key** | what names a child across frames, so state follows it when a list is reordered | id |
| **mount / update / unmount** | the three things reconciliation does to a scope | |
| **measure / assign** | the two layout passes: content asks for a size, parent hands out rectangles | |
| **paint** | writing cells | draw, render |
| **cells** | `ratatui::buffer::Buffer`, aliased `Cells` in feature code as `draw/` already does | |
| **canvas** | the host that hands its rectangle to a painting function | escape hatch |
| **too small** | what a container paints when its children cannot meet their minimum — the repository's existing words | |
| **hidden** | mounted, out of layout, unpainted, unhittable — Neovim's word for a loaded buffer nobody is showing | |
| **listener** | a closure a host registers for keys, mouse or focus | handler (banned by `lint-arch`) |
| **bubble** | offering an event to each ancestor in turn until one stops it | |
| **promise** | one answer, arriving later | completion, task |
| **observable** | answers that keep coming | subscription, stream |
| **external store** | something outside the tree that changes on its own, read through a snapshot | |
| **snapshot** | one reading of an external store, compared by identity | |
| **generation** | the counter that makes a stale address fail rather than land on a stranger | |
| **redraw** | mark a scope for re-running, Neovim's `:redraw` | invalidate, dirty |

---

## 3. Public types

Everything `loom` exports. `loom-macros` is re-exported through `loom`, so
feature code never names it.

### 3.1 `lib.rs`

```rust
#![doc = include_str!("../README.md")]

mod component;
mod current;
mod event;
mod frame;
mod hook;
mod layout;
mod node;
mod paint;
mod reconcile;
mod scope;
mod tree;
pub mod testing;

pub use component::Component;
pub use event::{
    Bubble, Focus, Listeners, Mouse, capture_pointer, focus_next, focus_previous,
    release_pointer,
};
pub use hook::{
    Always, Cleanup, Context, ExternalStore, Notify, Observable, Observer, Promise, Ref,
    Resolver, SetState, Snapshot, Subscription, observable, promise, use_context, use_effect,
    use_layout_effect, use_memo, use_ref, use_state, use_sync_external_store,
};
/// What `context!`'s `Component::render` calls. Not API: the way to offer a
/// value is to write the provider element.
#[doc(hidden)]
pub use hook::offer;
pub use layout::{Basis, Edges, Layout};
pub use node::{Children, Element, Key, Node, NodeHandle};
pub use paint::{Canvas, CanvasProps, Column, ColumnProps, Divider, DividerProps, Gap, GapProps,
    Paint, Row, RowProps, Stack, StackProps, Text, TextProps};
pub use scope::{Scope, ScopeId};
pub use tree::Tree;

pub use loom_macros::{component, context, rsx};

/// Re-exported so a consumer builds against the same versions we did.
pub use crokey;
pub use crossterm;
pub use ratatui;
```

### 3.2 Nodes

```rust
// node.rs
use std::any::TypeId;
use std::rc::Rc;

use ratatui::layout::Rect;

/// One entry in the description of a frame.
///
/// Built by `rsx!` and thrown away after reconciliation. What survives a frame
/// is the scope tree.
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
    /// Where to write this node's handle once it has a rectangle. React's
    /// `ref`, and `rsx!` spells it `ref` too.
    pub node_ref: Option<Ref<Option<NodeHandle>>>,
    /// Painted instead of the children when they cannot meet their minimums.
    pub too_small: Option<Box<Node>>,
    pub children: Vec<Node>,
}

/// A node that has been laid out — what a `ref` holds once it points at
/// something. `Copy`, and valid until the node unmounts.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct NodeHandle { /* private: the scope, and which host within it */ }

impl NodeHandle {
    /// The rectangle the last layout gave it. The DOM's
    /// `getBoundingClientRect`.
    pub fn area(self) -> Rect;
    /// Take focus. The DOM's `node.focus()`. A no-op if the node is not
    /// `focusable`.
    pub fn focus(self);
    /// The DOM's `node === document.activeElement`.
    pub fn has_focus(self) -> bool;
    /// Whether `other` is this node or sits inside it. The DOM's
    /// `node.contains()`.
    pub fn contains(self, other: NodeHandle) -> bool;
    /// Whether the node is still mounted. A handle to an unmounted node
    /// answers `Rect::ZERO` and `false` rather than panicking.
    pub fn is_mounted(self) -> bool;
}

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
    pub fn part<C: Component>(props: C::Props, key: Option<Key>) -> Node;
    /// What a built-in host's `Element::build` calls.
    pub fn from_host(host: Host) -> Node;
}

/// What names a child across frames.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub enum Key {
    Number(u64),
    Text(Rc<str>),
}

impl From<u64> for Key { /* … */ }
impl From<usize> for Key { /* … */ }
impl From<&str> for Key { /* … */ }
impl From<String> for Key { /* … */ }

/// What `rsx!` calls. Implemented by `#[component]` for your components and
/// by hand for the built-in hosts, so the macro emits one call for both.
pub trait Element: 'static {
    type Props: 'static;
    fn build(props: Self::Props, key: Option<Key>) -> Node;
}

impl From<Node> for Node { /* identity, for `{ expr }` */ }
impl From<Option<Node>> for Node { /* `None` → `Empty` */ }
impl From<Vec<Node>> for Node { /* → `Fragment` */ }
impl From<()> for Node { /* → `Empty` */ }
```

### 3.3 Components and scopes

```rust
// component.rs
pub trait Component: 'static {
    type Props: 'static;
    const NAME: &'static str;
    fn render(props: &Self::Props, scope: &mut Scope) -> Node;
}
```

```rust
// scope.rs
/// Names one live component. The generation is bumped when a slab entry is
/// reused, so a stale handle fails a check instead of reading a stranger.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub struct ScopeId {
    index: u32,
    generation: u32,
}

/// The token a component holds while it runs.
///
/// Two integers. Everything a hook touches lives in the runtime and is reached
/// by a short borrow, so a hook may walk the tree while the component's own
/// function is on the stack.
pub struct Scope {
    id: ScopeId,
    parent: Option<ScopeId>,
}

impl Scope {
    pub fn id(&self) -> ScopeId;
    pub fn name(&self) -> &'static str;
}
```

### 3.4 State and refs

```rust
// hook/state.rs
use std::ops::Deref;

/// Writes one state slot. Called like a function.
///
/// The closure is given the value the slot will hold when the next render
/// starts, and answers what to put there.
///
/// ```
/// set_cursor(&|_| 5);
/// set_cursor(&|cursor| cursor + 1);
/// ```
pub struct SetState<T: 'static> {
    scope: ScopeId,
    slot: u16,
    write: &'static dyn Fn(&dyn Fn(T) -> T),
}

impl<T> Clone for SetState<T> { fn clone(&self) -> Self { *self } }
impl<T> Copy for SetState<T> {}
impl<T> PartialEq for SetState<T> { /* scope and slot */ }
impl<T> Eq for SetState<T> {}

impl<T: 'static> Deref for SetState<T> {
    type Target = dyn Fn(&dyn Fn(T) -> T);
    fn deref(&self) -> &Self::Target { self.write }
}

impl<T: 'static> SetState<T> {
    /// Whether the owning component is still mounted.
    pub fn is_mounted(self) -> bool;
}
```

React hands either a value or a function to one setter. Rust's call syntax
fixes one argument type, so every write here is the function, and a constant is
`&|_| 5`. Taking the closure by reference is what lets a write borrow whatever
is in scope; nothing is boxed and nothing needs `'static`.

`use_state` gives a slot its writer when the component mounts, and the runtime
holds that writer for the run of the program — 12 bytes naming the scope and
slot. Paying it once per slot is what keeps `SetState<T>` `Copy` at 32 bytes,
so a listener takes a setter without cloning it.

```rust
// hook/reference.rs
/// A mutable value that survives renders without causing one.
///
/// The Rust form of React's `useRef`. `current()` is `ref.current`: read
/// through it, call methods on it, or assign over it.
///
/// ```
/// view.current().scroll(3);
/// let top = view.current().top();
/// *view.current() = Viewport::new();
/// ```
pub struct Ref<T: 'static> {
    scope: ScopeId,
    slot: u16,
    cell: &'static RefCell<T>,
}

impl<T> Clone for Ref<T> { fn clone(&self) -> Self { *self } }
impl<T> Copy for Ref<T> {}
impl<T> PartialEq for Ref<T> { /* scope and slot */ }
impl<T> Eq for Ref<T> {}

impl<T: 'static> Ref<T> {
    /// The value in the slot. Writing through it is silent.
    pub fn current(self) -> RefMut<'static, T>;
    /// Whether the owning component is still mounted.
    pub fn is_mounted(self) -> bool;
}
```

React's ref has one name, `current`, and reads and writes both go through it.
The value here lives in the hook slot rather than in the handle, so `current`
is a call and hands back a guard; `*` reaches the value and method calls reach
it on their own.

The guard lasts to the end of the statement. Two of them on the same ref at
once panic (P4.5) — take the value out, or finish one statement before
starting the next. Different refs in one expression are fine, and are how a
canvas reads a buffer, a viewport and a store together.

The cell, like a setter's writer, is made when the component mounts and kept
for the run of the program. Paying that once per slot is what keeps `Ref<T>`
`Copy`, so a listener takes a ref without cloning it — 24 bytes.

Use state for a value that describes the frame. Use a ref for a large mutable
model, an imperative cache, or a value updated independently of rendering.
A ref write is silent, so when one changes what the frame shows, bump a state
value in the same breath — React's function components have no force-update
either, and this is the idiom its documentation gives.

A ref also does React's other job. `use_ref(scope, || None)` typed as
`Ref<Option<NodeHandle>>` is `useRef(null)`: hand it to a node as `ref` and the
runtime writes the handle in once the node has a rectangle. §5.8 is the whole
of it.

### 3.5 Layout

```rust
// layout/mod.rs
use ratatui::style::Style;

/// CSS flexbox, in whole cells, minus the parts nothing here uses.
///
/// Every field is a flexbox property under its CSS name, so "two `grow: 1`
/// beside one `Length(40)`" has an answer you can look up rather than one we
/// had to invent. §5 lists what is left out and the one thing added.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Layout {
    // As an item of its parent.
    /// `flex-basis` — the size asked for on the parent's main axis, before
    /// growing or shrinking.
    pub basis: Basis,
    /// `flex-grow` — shares of the space left over. 0 takes none.
    pub grow: u16,
    /// `flex-shrink` — shares of the overflow to give back. 0 never shrinks.
    pub shrink: u16,
    /// `min-width` / `min-height`. Nothing shrinks below these, and a parent
    /// that cannot honour them is too small (§5.5).
    pub min_width: u16,
    pub min_height: u16,
    /// `max-width` / `max-height`. Nothing grows past these.
    pub max_width: Option<u16>,
    pub max_height: Option<u16>,

    // As a container of its children.
    /// `gap` — cells between children.
    pub gap: u16,
    /// `padding` — cells inside the edges, taken off before the children.
    pub pad: Edges,

    // Neither.
    /// Painted before the children. CSS would call this `background`.
    pub fill: Option<Style>,
    /// `overflow: hidden` — children get rectangles no larger than this node's.
    pub clip: bool,
    /// `display: none`, except that the scope and its hooks stay alive:
    /// out of layout, unpainted, unhittable, still remembering.
    pub hidden: bool,
}

/// CSS's defaults: `flex: 0 1 auto`.
impl Default for Layout {
    fn default() -> Self {
        Self {
            basis: Basis::Auto,
            grow: 0,
            shrink: 1,
            min_width: 0,
            min_height: 0,
            max_width: None,
            max_height: None,
            gap: 0,
            pad: Edges::default(),
            fill: None,
            clip: false,
            hidden: false,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Basis {
    /// As much as the content measures. CSS `flex-basis: auto`.
    #[default]
    Auto,
    /// Exactly this many cells. `Length`, not `Cells`, because `Cells` is
    /// already this crate's name for the cell grid.
    Length(u16),
    /// A share of the container's inner size on the main axis.
    Percent(u16),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct Edges {
    pub top: u16,
    pub right: u16,
    pub bottom: u16,
    pub left: u16,
}

impl Edges {
    pub const fn all(n: u16) -> Self;
    pub const fn sides(n: u16) -> Self;
    pub const fn rows(n: u16) -> Self;
}
```

There is no `axis` field. `Row` lays its children out across and `Column` lays
them down, which is what those words mean; a `Layout` that disagreed with the
host holding it would be a contradiction the type should not be able to state.

`Layout`, not `Style`: `Style` means colour in every file of this repository.

### 3.6 Paint

```rust
// paint/mod.rs
use ratatui::buffer::Buffer as Cells;
use ratatui::layout::Rect;

/// What a `Canvas` is handed.
pub struct Paint<'a> {
    cells: &'a mut Cells,
    area: Rect,
    clip: Rect,
    focused: bool,
}

impl<'a> Paint<'a> {
    /// The cell grid. Writing outside `clip` is a bug; see R7.2.
    pub fn cells(&mut self) -> &mut Cells;
    /// This node's rectangle.
    pub fn area(&self) -> Rect;
    /// `area` intersected with every clipping ancestor.
    pub fn clip(&self) -> Rect;
    /// Whether this node holds focus.
    pub fn has_focus(&self) -> bool;
}
```

The built-in hosts, each with a props struct the macro fills in. Every one of
them also carries `pub node_ref: Option<Ref<Option<NodeHandle>>>`, which `rsx!`
spells `ref`; it is left out of the listings below to keep them readable.

```rust
// paint/host.rs
pub struct Row;      pub struct RowProps      { pub layout: Layout, pub listeners: Listeners,
                                                pub focusable: bool, pub too_small: Option<Node>,
                                                pub children: Children }
pub struct Column;   pub struct ColumnProps   { /* the same fields */ }
/// Children painted over one another, in declaration order.
pub struct Stack;    pub struct StackProps    { /* the same fields */ }
/// Empty space. `Gap { layout: Layout { grow: 1, .. } }` pushes what follows away.
pub struct Gap;      pub struct GapProps      { pub layout: Layout }
/// One cell of `symbol`, repeated down or across.
pub struct Divider;  pub struct DividerProps  { pub layout: Layout,
                                                pub symbol: &'static str,
                                                pub style: ratatui::style::Style }
/// Text this program generated. Measures itself. Does not sanitise control
/// characters — untrusted text goes through a `Canvas`.
pub struct Text;     pub struct TextProps     { pub layout: Layout,
                                                pub text: std::rc::Rc<str>,
                                                pub style: ratatui::style::Style }
/// The escape hatch: a rectangle handed to a painting function.
pub struct Canvas;   pub struct CanvasProps   { pub layout: Layout, pub listeners: Listeners,
                                                pub focusable: bool,
                                                pub paint: std::rc::Rc<dyn Fn(&mut Paint<'_>)> }

// … and so on; every host's props derive or implement `Default`.
```

### 3.7 Events

```rust
// event/mod.rs
use crokey::KeyCombination;
use crossterm::event::MouseButton;
use ratatui::layout::Position;

/// What a listener says about an event it was given.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Bubble {
    /// Dealt with. Nothing above sees it.
    Stop,
    /// Not mine. Offer it to my parent.
    Continue,
}

pub struct Mouse {
    /// Which button is down, `None` when none is. On a move this is React's
    /// `buttons` — what separates a drag from a plain move.
    pub button: Option<MouseButton>,
    /// Where on the screen.
    pub at: Position,
    /// Where within this node's rectangle.
    pub local: Position,
}

pub struct Focus {
    /// The node on the other side of the move: the one losing focus in an
    /// `on_focus`, the one gaining it in an `on_blur`. `None` at either end
    /// of the sequence. React's `relatedTarget`.
    pub related: Option<NodeHandle>,
}

/// Every listener one host can carry.
#[derive(Clone, Default)]
pub struct Listeners { /* private */ }

impl Listeners {
    pub fn new() -> Self;
    pub fn on_key(self, listen: impl Fn(KeyCombination) -> Bubble + 'static) -> Self;
    pub fn on_mouse_down(self, listen: impl Fn(Mouse) -> Bubble + 'static) -> Self;
    /// Fires with a button held or without one. React's `onMouseMove`; there
    /// is no separate drag event, `Mouse::button` is the difference.
    pub fn on_mouse_move(self, listen: impl Fn(Mouse) -> Bubble + 'static) -> Self;
    pub fn on_mouse_up(self, listen: impl Fn(Mouse) -> Bubble + 'static) -> Self;
    /// Positive is down. React's `onWheel`.
    pub fn on_wheel(self, listen: impl Fn(i32) -> Bubble + 'static) -> Self;
    /// Focus arrived, at this scope or at one inside it. React's `onFocus`.
    pub fn on_focus(self, listen: impl Fn(Focus) -> Bubble + 'static) -> Self;
    /// Focus left, from this scope or from one inside it. React's `onBlur`.
    pub fn on_blur(self, listen: impl Fn(Focus) -> Bubble + 'static) -> Self;
}

/// Move focus to the next focusable node in paint order, wrapping. A browser
/// does this itself on Tab; a terminal has no such convention, so a key
/// listener calls it. No-ops when nothing is focusable.
pub fn focus_next();
pub fn focus_previous();

/// Route every mouse event to this node until the button comes up or
/// `release_pointer` is called. Called from `on_mouse_down`. The DOM's
/// `setPointerCapture`, without the pointer id — a terminal has one pointer.
pub fn capture_pointer();
pub fn release_pointer();
```

A child tells an ancestor something by calling a function the ancestor gave it
— as a prop, or through context when it sits several levels down. That is
React's pattern and it needs nothing from the framework: `Rc<dyn Fn(T)>` is
already `Clone + 'static`, so it is a context value like any other. §10 shows
both ends.

State follows the same split. A render reads the snapshot returned by
`use_state`; a listener keeps the accompanying `SetState<T>` and calls it.

### 3.8 Worker answers

```rust
// hook/worker.rs
/// The answer to one request, arriving later.
#[must_use = "a promise with no `then` throws its answer away"]
pub struct Promise<T: 'static> { /* private */ }

impl<T: 'static> Promise<T> {
    /// Runs `take` when the answer arrives, with the owning scope entered.
    pub fn then(self, take: impl FnOnce(T) + 'static);
}

/// The answering end, kept by whoever sent the request.
///
/// Carries the owning scope, the effect's slot and the effect's generation, so
/// an answer that arrives after the deps changed or the component went away is
/// refused rather than applied.
pub struct Resolver<T: 'static> { /* private; holds a Weak to the runtime */ }

impl<T: 'static> Resolver<T> {
    /// Delivers. Returns whether it was taken.
    pub fn resolve(self, value: T) -> bool;
    pub fn is_wanted(&self) -> bool;
}

/// Answers that keep coming, for a worker that replies in pieces.
#[must_use = "an observable with no `subscribe` throws its answers away"]
pub struct Observable<T: 'static> { /* private */ }

impl<T: 'static> Observable<T> {
    /// Runs `take` on every piece.
    pub fn subscribe(self, take: impl FnMut(T) + 'static);
}

/// The delivering end of an `Observable`. RxJS's `Observer`, with the two of
/// its three methods that mean something here.
pub struct Observer<T: 'static> { /* private */ }

impl<T: 'static> Clone for Observer<T> {}

impl<T: 'static> Observer<T> {
    /// Delivers one piece. Returns whether it was taken.
    pub fn next(&self, value: T) -> bool;
    pub fn is_wanted(&self) -> bool;
    /// No more pieces, for every clone of this observer.
    pub fn complete(self);
}

/// Opens a one-shot address: the resolver the answerer keeps, and the promise
/// the asker attaches a handler to. JavaScript spells the same pair
/// `Promise.withResolvers()`.
///
/// Legal inside an effect body, where the runtime knows which slot is asking.
/// Panics: P4.6 outside one.
pub fn promise<T: 'static>() -> (Resolver<T>, Promise<T>);

/// Opens a many-shot address. Same pair, same rule.
/// Panics: P4.6 outside an effect body.
pub fn observable<T: 'static>() -> (Observer<T>, Observable<T>);
```

Three differences from the JavaScript, each because the thing it stands for is
not here.

`Promise.withResolvers()` hands back a `reject` as well. There is nothing to
reject into: `pipeline::file::Response` carries a `Result` already, so a failed
read arrives as an answer and the handler matches on it.

`then` returns nothing, where JavaScript returns a promise to chain the next
step onto. There is no next step. A handler that wants more work asks for it
(R9.1.3).

`subscribe` returns nothing, where RxJS returns a `Subscription` to
`unsubscribe()` with. An effect's cleanup closes every address it opened
(R9.3.5), so there is nothing to hold and nothing to remember to release. The
one place a `Subscription` does appear is `ExternalStore` (§4.2), because a
store is outside the tree and has to be told when a reader has gone — which is
also why React's `useSyncExternalStore` asks its `subscribe` for an
unsubscribe.

And there is no `.await`. That would need an executor, and it would have
nowhere to suspend: a component returns a `Node` now, so it can never wait.
React's components are synchronous for the same reason. Every answer arrives
as a handler that sets state and asks for a frame.

### 3.9 The tree

```rust
// tree.rs
use ratatui::buffer::Buffer as Cells;
use ratatui::layout::Rect;

/// The runtime the application owns. Single-threaded by construction: every
/// handle into it holds an `Rc` or reaches it through a thread-local.
pub struct Tree { /* private */ }

impl Tree {
    pub fn new<C: Component>(props: C::Props) -> Self;
    /// Replaces the root's props and redraws it.
    pub fn set_props<C: Component>(&mut self, props: C::Props);
    /// Reconcile, lay out, paint, run effects. The only entry point that
    /// writes cells.
    pub fn draw(&mut self, cells: &mut Cells, area: Rect);
    /// Routes a key to the focused scope, then upward. Returns whether one
    /// listener stopped it.
    pub fn press(&mut self, key: crokey::KeyCombination) -> bool;
    pub fn mouse(&mut self, event: crossterm::event::MouseEvent) -> bool;
    /// Whether anything has been marked for redraw since the last `draw`.
    pub fn needs_draw(&self) -> bool;
    /// How many render-and-layout rounds the last `draw` took. One, unless a
    /// layout effect wrote state. Capped at 4 (R5.8.2).
    pub fn layout_rounds(&self) -> usize;
    /// Mark everything. What a terminal resize does.
    pub fn redraw_all(&mut self);
    pub fn focused_scope(&self) -> Option<ScopeId>;
}
```

### 3.10 Testing

```rust
// testing.rs
use ratatui::buffer::Buffer as Cells;
use ratatui::layout::Rect;
use ratatui::style::Style;

/// One component, one screen, no terminal.
pub struct Harness { /* private */ }

impl Harness {
    /// Mounts `C` at `width` × `height`. Does not draw.
    pub fn new<C: Component>(props: C::Props, width: u16, height: u16) -> Self;
    /// Provides a context value above the root, for a component that reads one.
    pub fn provide<C: Context>(self, value: C::Value) -> Self;
    /// Replaces the root's props.
    pub fn set_props<C: Component>(&mut self, props: C::Props) -> &mut Self;

    /// Draws if anything is marked. Idempotent.
    pub fn draw(&mut self) -> &mut Self;
    /// Draws whether or not anything is marked.
    pub fn force_draw(&mut self) -> &mut Self;

    /// The screen as text, one string per row, trailing blanks trimmed.
    pub fn screen(&mut self) -> Vec<String>;
    pub fn screen_row(&mut self, y: u16) -> String;
    pub fn style_at(&mut self, x: u16, y: u16) -> Style;
    pub fn cells(&mut self) -> &Cells;

    pub fn press(&mut self, key: crokey::KeyCombination) -> &mut Self;
    pub fn click(&mut self, x: u16, y: u16) -> &mut Self;
    pub fn drag(&mut self, x: u16, y: u16) -> &mut Self;
    pub fn release(&mut self, x: u16, y: u16) -> &mut Self;
    pub fn wheel(&mut self, x: u16, y: u16, lines: i32) -> &mut Self;
    pub fn resize(&mut self, width: u16, height: u16) -> &mut Self;

    /// The scope tree as indented text: name, key, rectangle.
    pub fn tree_text(&mut self) -> String;
    /// The rectangle of the first scope with this component name.
    pub fn area_of(&self, name: &str) -> Option<Rect>;
    /// How many times a component of this name has run since the harness was
    /// built. The primitive behind every memo and reconciliation test.
    pub fn render_count_of(&self, name: &str) -> usize;
    /// How many component functions ran during the last `draw`.
    pub fn render_count(&self) -> usize;
    /// How many layout rounds the last `draw` took. One, unless a layout
    /// effect wrote state.
    pub fn layout_rounds(&self) -> usize;
    pub fn focused_name(&self) -> Option<&'static str>;
    pub fn needs_draw(&self) -> bool;
}

/// A component that renders nothing and counts its renders. For tests about
/// identity that do not want a real component.
pub struct Probe;
pub struct ProbeProps { pub tag: u32 }
```

---

## 4. Hooks

### 4.1 Storage

Hook slots live on the runtime, in a slab parallel to the scope slab:

```rust
// hook/mod.rs
struct Hooks {
    slots: Vec<Slot>,
    /// Reset to 0 at the top of each render.
    index: usize,
    #[cfg(debug_assertions)]
    sites: Vec<&'static std::panic::Location<'static>>,
}

enum Slot {
    /// The committed value, the pending one, and the slot's writer.
    State(Box<dyn PendingState>),
    /// Mutable storage that never marks its owner by itself.
    Ref(std::rc::Rc<std::cell::RefCell<dyn std::any::Any>>),
    Memo(MemoSlot),
    Effect(EffectSlot),
    LayoutEffect(EffectSlot),
    Store(StoreSlot),
}

struct StateSlot<T> {
    value: T,
    pending: Option<T>,
    write: &'static dyn Fn(&dyn Fn(T) -> T),
}

struct MemoSlot {
    deps: Box<dyn std::any::Any>,
    value: std::rc::Rc<dyn std::any::Any>,
}

struct EffectSlot {
    deps: Box<dyn std::any::Any>,
    cleanup: Option<Box<dyn FnOnce()>>,
    /// Bumped each time the effect runs, so a reply from the previous run is
    /// refused (R9.3.3).
    generation: u64,
}

struct StoreSlot {
    /// Dropped on unmount, which is what ends the subscription.
    subscription: Subscription,
    /// The last `Snapshot<T>`, compared with the next by `Rc::ptr_eq`.
    snapshot: Box<dyn std::any::Any>,
}
```

They stay on the runtime rather than on the `Scope` because a component's own
function is on the stack while it calls hooks: the scope is lifted out of its
slab entry for the duration of the render, and a hook that reached into it
could not also walk the tree. Reaching the runtime for one short borrow per
hook has neither problem.

The only primitive:

```rust
#[track_caller]
fn use_hook<H>(scope: &mut Scope, first: impl FnOnce() -> Slot, read: impl FnOnce(&mut Slot) -> H) -> H;
```

It bumps `index`, pushes on first render, checks the discriminant otherwise, and
panics with both call sites when they disagree (P4.1).

### 4.2 The hooks

```rust
// hook/state.rs
/// One render's value and a stable setter for its next value.
///
/// `first` runs once, when the component mounts. A write applies at once to
/// the pending value, and the returned `T` is this render's snapshot.
/// Panics: P4.1, P4.2.
#[track_caller]
pub fn use_state<T: Clone + PartialEq + 'static>(
    scope: &mut Scope,
    first: impl FnOnce() -> T,
) -> (T, SetState<T>);
```

The clone is the snapshot returned to the component. Small values are copied;
large immutable values live in an `Rc<T>`. A large value that is meant to be
mutated in place belongs in `use_ref`.

```rust
// hook/reference.rs
/// Mutable storage that survives a render without scheduling one.
///
/// `first` runs once. Reads and writes both go through `Ref::current`.
/// Panics: P4.1, P4.2.
#[track_caller]
pub fn use_ref<T: 'static>(scope: &mut Scope, first: impl FnOnce() -> T) -> Ref<T>;
```

```rust
// hook/memo.rs
/// A value recomputed only when `deps` changes.
///
/// Re-runs: when `deps != previous deps`. Returns the same `Rc` otherwise,
/// for as long as the component lives — React reserves the right to drop its
/// cache, this does not, so the identity is something you may rely on.
///
/// `compute` should be pure, and is not `'static`, so it may borrow what it
/// needs. Returning `Rc<T>` rather than `T` is how the same value comes back
/// each render: a borrow of the slot would hold `scope` for the rest of the
/// render, and `T` would clone the thing you called this to avoid building.
/// Panics: P4.1, P4.2.
#[track_caller]
pub fn use_memo<D, T>(scope: &mut Scope, deps: D, compute: impl FnOnce() -> T) -> std::rc::Rc<T>
where
    D: PartialEq + 'static,
    T: 'static;
```

```rust
// hook/effect.rs
/// Work to do after the frame is painted.
///
/// Re-runs after the paint of the first frame in which `deps != previous
/// deps`. What `run` returns is the cleanup: it is called before the next run
/// and again when the component goes away.
///
/// `()` as deps runs once. `Always` runs after every paint.
/// Panics: P4.1, P4.2.
#[track_caller]
pub fn use_effect<D, C>(
    scope: &mut Scope,
    deps: D,
    run: impl FnOnce() -> C + 'static,
) where
    D: PartialEq + 'static,
    C: Cleanup;

/// The same, run before the frame is painted rather than after.
///
/// Layout has finished, so every `ref` holds its node and `NodeHandle::area`
/// answers this frame's rectangle. A state write here re-renders and re-lays
/// out before anything reaches the screen, which is how a component that must
/// know its own size avoids showing a wrong frame first. §5.8 is the cost.
///
/// Prefer `use_effect`. This one holds the frame up.
/// Panics: P4.1, P4.2.
#[track_caller]
pub fn use_layout_effect<D, C>(
    scope: &mut Scope,
    deps: D,
    run: impl FnOnce() -> C + 'static,
) where
    D: PartialEq + 'static,
    C: Cleanup;

/// What `run` may return: a function that undoes the work, or `()` for
/// nothing to undo. `()` is not a `FnOnce()`, so the two impls do not overlap.
pub trait Cleanup: 'static {
    fn into_cleanup(self) -> Option<Box<dyn FnOnce()>>;
}

impl Cleanup for () { /* nothing to undo */ }
impl<F: FnOnce() + 'static> Cleanup for F { /* call it */ }

/// Deps that never compare equal, so the effect runs after every paint. This
/// is what React means by leaving the dependency array out.
#[derive(Clone, Copy, Debug)]
pub struct Always;

impl PartialEq for Always {
    fn eq(&self, _: &Self) -> bool { false }
}
```

```rust
// hook/context.rs
/// One context: the key a reader names and the element a provider writes.
///
/// Declared with `context!`, never by hand.
pub trait Context: 'static {
    type Value: Clone + 'static;
    /// What a reader gets when nothing above it provides one.
    fn default_value() -> Self::Value;
    /// Whether a new offer matches the last, so the version can stay put.
    /// `context!` fills this in with `==`. A value with no `==` of its own
    /// names the comparison in its declaration instead.
    fn same(old: &Self::Value, new: &Self::Value) -> bool;
}

// A provider's props are generated with the name `<Context>Props`, so `rsx!`
// finds them like any other component's (§11.2):
//
//     pub struct ThemeContextProps { pub value: Theme, pub children: Children }

/// The nearest ancestor's value for `C`, or `C::default_value()`.
///
/// Re-runs: every render; the read is recorded so a memoised component cannot
/// go stale.
/// Panics: none.
pub fn use_context<C: Context>(scope: &mut Scope) -> C::Value;
```

```rust
// hook/store.rs
/// Something outside the tree that changes on its own — a worker, a file
/// watcher, a clock.
///
/// `snapshot` must hand back the same `Snapshot` until something changes, and
/// a different one when it does. React asks the same of `getSnapshot`, and it
/// is why `Snapshot` compares by identity rather than by value.
pub trait ExternalStore {
    type Value: ?Sized + 'static;

    /// Starts telling `notify` about changes. Dropping the `Subscription`
    /// stops it, the way React's `subscribe` returns its own unsubscribe.
    fn subscribe(&self, notify: Notify) -> Subscription;

    /// What the value is now.
    fn snapshot(&self) -> Snapshot<Self::Value>;
}

/// A value read from a store, compared by identity.
///
/// React compares one snapshot with the next using `Object.is`: a store that
/// hands back a new object has changed, one that hands back the same object
/// has not. That is this, and it is what lets a snapshot sit in an effect's
/// deps and mean "the thing behind it moved".
pub struct Snapshot<T: ?Sized>(std::rc::Rc<T>);

impl<T: ?Sized> Clone for Snapshot<T> { /* clones the `Rc` */ }
impl<T: ?Sized> PartialEq for Snapshot<T> { /* `Rc::ptr_eq` */ }
impl<T: ?Sized> std::ops::Deref for Snapshot<T> { type Target = T; }
/// How a store makes one. A new `Rc` is a new reading; the same `Rc` is the
/// same reading.
impl<T: ?Sized> From<std::rc::Rc<T>> for Snapshot<T> {}

/// What a store calls to say it changed. `Clone`, so a store keeps one per
/// reader.
#[derive(Clone)]
pub struct Notify(/* private */);

impl Notify {
    /// Marks the component that subscribed for redraw. Does nothing once that
    /// component has gone away.
    pub fn changed(&self);
}

/// Ends a subscription when it is dropped. A store builds one out of the work
/// it wants done then — the unsubscribe React's `subscribe` returns, with the
/// dropping done for you.
pub struct Subscription(/* private */);

impl Subscription {
    pub fn new(stop: impl FnOnce() + 'static) -> Self;
}

/// Subscribe to a store, and read it.
///
/// Subscribes on mount and unsubscribes on unmount, so `store` must be the
/// same store for the component's life — which it is when it arrives as a
/// prop or from context.
///
/// Re-runs: when the store says it changed and hands back a different
/// `Snapshot`.
/// Panics: P4.1, P4.2.
#[track_caller]
pub fn use_sync_external_store<S: ExternalStore>(
    scope: &mut Scope,
    store: &S,
) -> Snapshot<S::Value>;
```

### 4.3 What the type system reaches, and what it does not

Three layers, earliest first.

1. **A hook cannot be called outside a component.** Every hook takes
   `&mut Scope`, and a `Scope` is only reachable as the parameter `#[component]`
   gave you. A type error, not a runtime one — the rule React cannot enforce.
2. **`scope` cannot be smuggled into a listener.** Listeners are `'static`
   closures; `&mut Scope` is not `'static`. A type error.
3. **Order violations are caught on the first render that diverges**, by the
   discriminant check in `use_hook` plus a count check at the end of every
   non-first render. In a debug build the message names both call sites by file
   and line.

What the type system cannot do is make a conditional hook call a compile error.
The construction that would — a heterogeneous list of hook types threaded
through the component's signature — puts every component's private state in its
public type, which is the disease function components cure. React ships a lint
for this; we ship a panic that names both lines.

`promise` and `observable` are the second thing it cannot do. They read the
effect the runtime is running, the way a listener reads the scope it belongs
to, so calling one outside an effect is P4.6 rather than a type error. Handing
the effect body a `&mut Effect<'_>` would have made it one, at the price of a
parameter React's setup does not have.

---

## 5. Layout

CSS flexbox, in whole cells, minus the parts nothing here uses, plus one rule
CSS has no word for.

Borrowed rather than invented, and borrowed under its own names. A layout model
is a thing readers ask questions of — *two `grow: 1` beside one `Length(40)`,
who wins when it does not fit?* — and every such question about flexbox already
has an answer written down by someone else, checked by four browser engines and
twenty years of use. An invented model answers each one for the first time, in
a conversation, at the moment someone hits it.

### 5.1 What is borrowed, left out, and added

| borrowed | CSS |
|---|---|
| `basis` | `flex-basis` |
| `grow` | `flex-grow` |
| `shrink` | `flex-shrink` |
| `min_width`, `min_height` | `min-width`, `min-height` |
| `max_width`, `max_height` | `max-width`, `max-height` |
| `gap` | `gap` |
| `pad` | `padding` |
| `clip` | `overflow: hidden` |
| `hidden` | `display: none` |
| stretch on the cross axis | `align-items: stretch`, the default |
| children packed from the start | `justify-content: flex-start`, the default |
| the sizing algorithm of §5.4 | the CSS Flexbox spec's *resolve flexible lengths* |

**Left out**, each with a definition to copy the day it is wanted: `flex-wrap`,
`align-content`, `align-items` and `align-self`, `justify-content`, `order`,
`position: absolute`, `aspect-ratio`, baseline alignment, auto margins, and the
`flex` shorthand. Nothing in this program centres anything — even *terminal too
small* is written at column 0.

**Added**, three things:

1. **`too_small`** (§5.5). CSS's answer to "does not fit" is to overflow. This
   program's answer, in six places already, is `return None` — draw a message
   instead of a corrupt frame.
2. **A wider screen never shows less than a narrower one** (R5.5.4). CSS
   promises nothing of the sort; `render::layout` has tested it for months.
3. **Integer tie-breaking** (R5.4.7). CSS is fractional and browsers round
   differently. Whole cells and a fixed rule are what stop columns drifting as
   a pane is resized.

That is the whole distance between this and flexbox: one behaviour, one
guarantee, one rounding rule.

### 5.2 What a node asks for

**R5.2.1** A `Row` lays its children out across; a `Column` lays them down.
That direction is the container's **main axis**; the other is its **cross
axis**. There is no `axis` field: the host name says it.
*test: `a_row_stacks_across_and_a_column_stacks_down`*

**R5.2.2** `basis` is the size a child asks for on its parent's main axis. The
same `Layout` on a child of a `Row` asks about width and on a child of a
`Column` asks about height.
*test: `the_same_basis_means_width_in_a_row_and_height_in_a_column`*

**R5.2.3** `min_width`, `min_height`, `max_width` and `max_height` are cells,
and they name an axis outright rather than following the parent's. A minimum of
"as much as the content wants" says nothing, and a minimum that changed meaning
when its parent changed direction would be worse than none.
*test: `a_minimum_is_a_number_of_cells`*

**R5.2.4** A `Part` has no box of its own. It contributes the box of the node
it returned. A `Fragment` contributes its children, spliced into the parent's
child list in order. An `Empty` contributes nothing.
*test: `a_component_is_not_a_box_of_its_own`*

**R5.2.5** A node with `hidden` is skipped by measure, resolve, paint,
hit-testing and focus order, and keeps its scope and its hooks. `display: none`
that remembers.
*test: `a_hidden_pane_keeps_its_viewport`*

### 5.3 The measure pass — bottom-up

Only `Basis::Auto` needs it, and only `Text` answers.

**R5.3.1** A host with a `measure` function is asked. Only `Text` has one; it
answers `(ratatui::text::Span::styled(text, style).width(), 1)`.
*test: `text_measures_its_own_width_in_cells`*

**R5.3.2** A container measures as the sum of its children along its main axis,
plus `gap` between each pair, plus padding; and as the largest child across it,
plus padding.
*test: `a_row_measures_as_the_sum_of_its_children_and_its_gaps`*

**R5.3.3** A child whose `basis` is `Length` measures as that, not as its
content. A child whose `basis` is `Percent` measures as zero, because what it
is a share of is not known until §5.4.
*test: `a_fixed_child_measures_as_its_size_not_its_content`*

**R5.3.4** A `Canvas` with `Basis::Auto` measures as zero. A canvas paints
whatever it is given; it has no content to ask.
*test: `a_canvas_asks_for_nothing`*

**R5.3.5** Measurement never reads state and never runs a component. It is a
function of the node tree the render pass produced.
*test: `measuring_runs_no_component`*

### 5.4 Resolving the main axis

The CSS Flexbox algorithm *resolve flexible lengths*, with `u16` in place of
`f32`, one line, no wrap.

**R5.4.1 — hypothetical size.** For each visible child: `Auto` takes what it
measured, `Length(n)` takes `n`, `Percent(n)` takes `n`% of the container's
inner main size rounded down. Each is then clamped to that child's minimum and
maximum on the main axis.
*test: `a_percent_child_is_a_share_of_the_inner_size`*

**R5.4.2 — free space.** Inner main size, less the gaps between children, less
the sum of the hypothetical sizes. Positive means room to grow; negative means
too much was asked for.
*test: `free_space_counts_the_gaps`*

**R5.4.3 — growing.** Positive free space is handed to children with
`grow > 0`, in proportion to their `grow`. A child with `grow: 0` keeps its
hypothetical size. Nothing grows past its maximum.
*test: `two_growing_children_split_what_the_fixed_one_left`*

**R5.4.4 — shrinking.** Negative free space is taken back from children with
`shrink > 0`, in proportion to `shrink × hypothetical size` — CSS's scaled
shrink factor, so a wide child gives up more than a narrow one at the same
`shrink`. Nothing shrinks below its minimum.
*test: `a_wide_child_gives_up_more_than_a_narrow_one`*

**R5.4.5 — freezing.** After growing and shrinking, any child outside its
minimum or maximum on the main axis — including one that never moved — is
frozen at that bound, and the pass runs again over the rest with the space that
is left. It repeats at most once per child, because each round freezes at least
one.
*test: `a_child_frozen_at_its_minimum_pushes_the_rest_smaller`*

**R5.4.6 — order.** Children are placed along the main axis in child order,
each `gap` cells after the previous, starting at the inner rectangle's near
edge. Space nobody claimed stays at the far end, with the container, and shows
its `fill`.
*test: `children_tile_the_container_in_order`*

**R5.4.7 — the remainder.** Growing and shrinking divide integers, so a
remainder is left over. It is handed out one cell at a time, to the children
with the largest fractional part, ties to the earlier child. Three equal
children of a hundred cells are 34, 33, 33 — the same three every frame, so a
column does not shift when something elsewhere redraws.
*test: `an_odd_split_is_the_same_two_frames_running`*

**Worked, against the code it replaces.** `render::layout::split` divides the
body between the list and the diff: `MIN_LIST` 8, a 1-cell divider, `MIN_RIGHT`
20, the reader's divider at 40. Run today, it answers:

```
width 80 -> 40/1/39
width 50 -> 29/1/20
width 29 ->  8/1/20
width 28 -> refused
```

As flexbox: the list is `basis: Length(40), shrink: 1, min_width: 8`; the
divider is `basis: Length(1), shrink: 0`; the diff is `grow: 1,
basis: Length(0), min_width: 20`.

*At 80* the hypothetical sizes are 40, 1, 0 and free space is 39, so the diff
grows to 39 (R5.4.3), above its minimum. **40/1/39**.

*At 50* free space is 9, the diff grows to 9, which is below its minimum, so it
freezes at 20 (R5.4.5) and the pass runs again over the rest: 50 − 1 − 20 = 29
for the list, above its own minimum. **29/1/20**.

*At 29* free space is −12, so the list shrinks to 28 (R5.4.4 — it is the only
child with a non-zero scaled shrink factor). The diff is still at 0, below its
minimum, so it freezes at 20 and the pass runs again: 29 − 1 − 20 = 8 for the
list, exactly its minimum. **8/1/20**.

*At 28* the minimums sum to 8 + 1 + 20 = 29, which does not fit, so the row is
too small (§5.6) — the same threshold as `split`'s
`area.width < MIN_LIST + 1 + MIN_RIGHT`.

Four numbers, four hand-written branches replaced by one algorithm somebody
else already debugged.
*test: `the_flex_pass_reproduces_the_split_it_replaces`*

### 5.5 The cross axis, and the rectangles

**R5.5.1** A child's cross size is the container's inner cross size, clamped by
that child's minimum and maximum on that axis. This is `align-items: stretch`,
and it is the only alignment there is.
*test: `a_child_of_a_row_is_as_tall_as_the_row`*

**R5.5.2** A container subtracts `pad` from its rectangle before any of §5.4.
*test: `padding_comes_off_before_the_children_are_measured_against_it`*

**R5.5.3** A child's rectangle is intersected with its parent's clip when the
parent has `clip`. `Layout::default()` has it off; `CanvasProps::default()`
supplies a `Layout` with it on, because a canvas is where unbounded painting
would otherwise happen.
*test: `a_clipping_parent_shrinks_its_children`*

**R5.5.4** `Stack` gives every child the container's whole inner rectangle. It
has no main axis and §5.4 does not run.
*test: `a_stack_gives_every_child_the_same_rectangle`*

**R5.5.5** A rectangle of zero width or zero height is legal. Its subtree is
assigned, painted as nothing, and hit-tested as nothing.
*test: `a_zero_width_pane_paints_nothing_and_does_not_panic`*

### 5.6 Too small

The one behaviour CSS does not have. Everything above is flexbox; this is not.

**R5.6.1** After §5.4, a container compares each child's assigned width and
height against that child's `min_width` and `min_height`. If any child is still
short — because shrinking ran out of room, not because the child asked for less
— the container is too small.
*test: `a_child_below_its_minimum_makes_its_parent_too_small`*

**R5.6.2** A container that is too small assigns nothing below it and paints
its `too_small` node in its own rectangle. If it has no `too_small` node, the
condition passes to *its* parent.
*test: `too_small_climbs_until_someone_answers_for_it`*

**R5.6.3** A `too_small` node is laid out and painted as an ordinary child, and
may itself be too small, which climbs again.
*test: `a_too_small_message_that_does_not_fit_climbs_again`*

**R5.6.4** A wider screen never shows less than a narrower one: a container
that fits at *n* cells fits at *n + 1*. This is `render::layout`'s existing
property, carried up into the framework.
*test: `a_wider_screen_never_shows_less_than_a_narrower_one`*

Where CSS overflows, this refuses. That is the whole difference, and it is
load-bearing: overflow is what R5.6.4 forbids, because a squashed pane at
*n + 1* cells can show less than an unsquashed one at *n*. It replaces
`draw/screen.rs::too_small` and the `bool` `draw/tab.rs` threads back up
through four functions.

### 5.7 What is not here

No wrap, no `align-self`, no absolute positioning, no aspect ratio, no
baseline, no `order`. Each is defined by CSS and can be added under its own
name the day something needs it — which is the point of borrowing the model
rather than inventing one. Until then each is a rule a reader would have to
learn before they could predict a frame.
### 5.8 Layout effects, and the round cap

The pass order inside one `Tree::draw`:

```
1  enter the runtime
2  render round:   reconcile from the root, then drain the redraw set,
                   parents before children; commit each scope's pending state
                   immediately before deciding whether it must run
3  layout round:   measure bottom-up, assign top-down, write each scope's
                   area, then fill in every `ref` a node was given
4  layout effects: cleanups deepest first, then setups shallowest first.
                   A state write here  →  go back to 2
5  paint:          walk the tree, record where every listening node landed
6  effects:        cleanups deepest first, then setups shallowest first
7  leave the runtime
```

**R5.8.1** A `ref` holds `None` until the node it names has a rectangle. The
render that first hands it to a node therefore reads `None`, and the layout
effect after that render reads the handle. A component that measures itself
renders at least twice on the frame it mounts. React's `ref.current` is null
in the same place for the same reason.
*test: `a_component_that_measures_itself_renders_twice_when_it_mounts`*

**R5.8.2** Steps 2 to 4 repeat at most **4** times per frame. On the 4th round
the areas from that round are painted and the frame completes; `Tree::layout_rounds()`
reports 4, which is how a test sees it — `loom` depends on `ratatui`,
`crossterm` and `crokey` and nothing else, so it cannot log. Four is slack, not
a measurement: the longest chain of size-dependent decisions in this program is
one — a pane's height reaches `Viewport::set_height`, which moves `top` and
never its own rectangle.
*test: `a_component_that_changes_size_every_round_still_paints_a_frame`*

**R5.8.3** A component may be re-rendered at most **16** times in one frame.
The 17th panics (P5.1). Sixteen is chosen, not measured: it is far above
anything a settling tree does and far below a number that would hang a
terminal.
*test: `a_component_that_sets_state_on_every_render_panics_rather_than_hanging`*

**R5.8.4** `Tree::draw` is not re-entrant. Calling it from inside a paint
callback panics (P7.1).
*test: `drawing_from_inside_a_painter_is_refused`*

**R5.8.5** A layout effect that writes back the size it just measured settles
in two rounds. The second round measures the same rectangle and writes the same
value, and an equal write clears the mark (R6.3.5). Take that comparison away
and every frame runs to the cap — measured, by taking it away.
*test: `measuring_the_same_size_twice_settles_the_frame`*

Measuring is not a hook here, because React does not make it one either. A
component that needs its own rectangle holds a `ref`, hands it to a node, and
reads it in a layout effect:

```rust
/// The rectangle flex gave this node. `Rect::ZERO` until it has one.
pub fn use_size(scope: &mut Scope, node: Ref<Option<NodeHandle>>) -> Rect {
    let (size, set_size) = use_state(scope, || Rect::ZERO);
    use_layout_effect(scope, Always, move || {
        if let Some(node) = *node.current() {
            set_size(&|_| node.area());
        }
    });
    size
}
```

That is the whole of it, and it lives in `crates/ui/src/hook.rs` — this
program's own, not the framework's. It is the same handful of lines React's
documentation writes over `useLayoutEffect`. `loom` exports the three pieces
and stops there.

What it is for, in this program's own terms: `draw/pane.rs` currently calls
`viewport.set_height(rect.height, …)` *during* the draw, which is a state write
in the paint pass and is forbidden by R7.1.4. The replacement is `use_size` in
the render pass followed by `view.current().set_height(h, rows)`, where `view`
came from `use_ref`. The current frame reads the new height during paint.

---

## 6. Reconciliation

The scope tree is what survives a frame. Reconciliation decides, for each node
this frame produced, which live scope it is — and therefore which hook slots,
which state values and refs, which effect cleanups. That is the whole reason
the framework exists; `use_state` cannot be written without it.

```rust
// scope.rs
struct Mounted {
    name: &'static str,
    type_id: TypeId,
    key: Option<Key>,
    parent: Option<ScopeId>,
    children: Vec<ScopeId>,
    depth: u32,

    /// Last props, so a scope can re-render alone when only its own state
    /// changed.
    props: Rc<dyn Any>,
    render: fn(&dyn Any, &mut Scope) -> Node,
    props_equal: Option<fn(&dyn Any, &dyn Any) -> bool>,

    /// Values offered to descendants, by TypeId, with a version.
    given: Vec<(TypeId, Rc<dyn Any>, u64)>,
    /// Context types read, with the version read, so a memo cannot go stale.
    taken: Vec<(TypeId, u64)>,

    /// Host-only.
    layout: Layout,
    paint: Option<Rc<dyn Fn(&mut Paint<'_>)>>,
    listeners: Listeners,
    focusable: bool,
    /// The `ref` this node was last given, so unmounting can clear it (R6.2.8).
    node_ref: Option<Ref<Option<NodeHandle>>>,
    area: Rect,
    clip: Rect,
}
```

Scopes live in a slab with a free list; `ScopeId` carries the generation of its
entry.

### 6.1 Matching

**R6.1.1** Before matching, the new child list is flattened: `Fragment` splices
its contents into the list and `Empty` contributes nothing. There is no fragment
scope and no empty scope.
*test: `a_fragment_leaves_no_scope_behind`*

**R6.1.2** If **every** flattened child carries a key, matching is keyed. If
**no** child carries one, matching is positional. A mixture panics (P6.1).
*test: `a_list_with_one_key_missing_is_refused`*

**R6.1.3 — positional.** For `i` in `0 .. max(old.len(), new.len())`:

| old[i] | new[i] | |
|---|---|---|
| present | present, same `type_id` | **update** old[i] with new[i] |
| present | present, different `type_id` | **unmount** old[i], **mount** new[i] |
| present | absent | **unmount** old[i] |
| absent | present | **mount** new[i] |

*test: `a_component_at_the_same_place_keeps_its_state`*
*test: `a_different_component_at_the_same_place_starts_fresh`*

**R6.1.4 — keyed.** Build `HashMap<(TypeId, Key), ScopeId>` from the old
children. For each new child in the new order: found → update it and remove it
from the map; not found → mount. Whatever is left in the map is unmounted. The
parent's child list becomes the new order. No move operations are emitted,
because paint walks the tree fresh every frame.
*test: `reordering_a_keyed_list_carries_each_row_state_with_it`*

**R6.1.5** Two siblings with the same key panic (P6.2).
*test: `two_children_with_one_key_are_refused`*

**R6.1.6** A host node matches a host node of the same `name`; a host against a
part, or a host of another name, is unmount-and-mount.
*test: `a_row_that_becomes_a_column_is_a_new_node`*

Type-based matching is what preserves state, and it is the rule that makes
`if split { DiffPane } else { Blank }` behave the way a reader of React expects:
the same type at the same place keeps its scope, a different type destroys it.

### 6.2 Update, mount, unmount

**R6.2.1 — update.** The scope's props are replaced. If `same` is `Some(eq)`,
`eq(old, new)` holds, the scope is not marked for redraw, and no context it
recorded has changed version, the whole subtree is left untouched and its
children are not visited.
*test: `a_memo_component_with_equal_props_does_not_run`*
*test: `a_memo_component_whose_context_changed_runs_anyway`*
*test: `a_memo_component_whose_own_state_changed_runs_anyway`*

**R6.2.2 — update, otherwise.** `hooks.index` is set to 0, the scope is lifted
out of the slab, `render` is called, the returned node is reconciled against
the scope's children. On return the scope goes back into the slab, and if
`hooks.index != hooks.slots.len()` the render panics (P4.2).
*test: `a_render_that_skips_a_hook_is_refused`*

**R6.2.3 — update, host.** A matched host scope takes the new node's `layout`,
`paint`, `listeners` and `focusable`, and its children are reconciled in place.
A host has no hooks and no render of its own, so there is nothing to compare
and nothing to skip: every visit replaces all four. This is what makes a
listener the one the current frame built. The closures captured that render's
state snapshots and stable `SetState` or `Ref` handles; a later render replaces
them with closures carrying the later snapshots.
*test: `a_listener_is_the_one_the_last_render_built`*

**R6.2.4 — the skipped keep what they have.** A subtree left untouched by
R6.2.1 is not visited, so its host scopes keep the `layout`, `paint`,
`listeners` and `focusable` they already hold. They are still laid out,
painted, hit-tested and bubbled through, because nothing about them changed.
R6.3.3 says the same thing for a scope the redraw set never reached.
*test: `a_memoised_subtree_keeps_the_listeners_it_had`*

**R6.2.5 — mount.** A slab entry is allocated, `render` runs, and its hooks are
created on the way through. Then its children are mounted, in order.
*test: `mounting_runs_the_parent_before_the_child`*

**R6.2.6 — unmount.** Depth-first, children before parent. For each scope, in
this order: its effect tasks are closed, so an outstanding reply is refused;
its effect cleanups are called, deepest first; its context offers are dropped;
its focus and hit registrations are removed; its hook slots are dropped; the
slab entry is freed and its generation bumped, so every `SetState<T>`,
`Ref<T>` and `Resolver<T>` naming it now fails its check (P4.3, R9.3.1).
*test: `unmounting_runs_the_deepest_cleanup_first`*
*test: `a_reply_for_a_component_that_went_away_is_refused`*

**R6.2.7** If the focused scope unmounts, focus moves to the next focusable in
paint order, or to the previous one if there is no next, or to nothing.
*test: `closing_the_focused_pane_moves_focus_rather_than_losing_it`*

**R6.2.8** A `ref` a node was given is set to `Some(handle)` when the node is
laid out and back to `None` when it unmounts, before that scope's cleanups run.
Detaching is the same: a node that stops being given the ref clears it. React
nulls a ref at both of those moments too.
*test: `a_ref_is_cleared_when_its_node_goes_away`*

**R6.2.9** Two live nodes given the same `ref` panic (P6.3). React lets the
second silently win; a terminal has no devtools to notice that with.
*test: `handing_one_ref_to_two_nodes_is_refused`*

### 6.3 The redraw set

**R6.3.1** Marks are drained in `(depth, index)` order, parents before
children, so a parent's re-render subsumes its children's marks.
*test: `a_parent_and_child_both_marked_run_the_parent_once`*

**R6.3.2** A mark on a scope that a parent has just unmounted is dropped.
*test: `a_mark_on_a_scope_that_went_away_is_dropped`*

**R6.3.3** A scope that is not marked and whose parent did not re-render keeps
its last child list, its rectangle and its listeners. It is still walked by
paint.
*test: `a_clean_component_is_painted_without_being_run`*

**R6.3.4** A write runs its closure at once, against the value the slot will
hold when the next render starts: the pending value if there is one, the
committed value otherwise. The answer becomes pending and marks the owner.
Writes therefore compose in call order, and a write sees the render snapshot
only if the caller passes it in.
*test: `two_writes_compose_in_call_order`*
*test: `a_write_reads_the_pending_value`*

**R6.3.5** The pending value is compared with the committed one. Equal clears
the mark. This lets a child report a status value during render; repeating the
same value settles the frame.
*test: `writing_the_same_value_marks_nothing`*
*test: `the_status_line_settles_in_one_frame`*

**R6.3.6** Writing through `Ref::current` never marks a scope. A caller that
changed something the frame describes bumps a state value alongside it. This
is the same line React draws between `useRef` and `useState`.
*test: `editing_a_ref_does_not_redraw`*

### 6.4 Memo, effect and layout-effect slots

Both hold deps and compare them the same way, so the rules that differ are
about *when* the work happens, not about which work is skipped.

**R6.4.1** Deps are compared with `PartialEq`, on the value. React compares
references with `Object.is`, so a rebuilt array is always a new dependency;
here a rebuilt list holding the same files is the same deps and the slot is
skipped. What that costs is the walk, not a re-run.
*test: `a_rebuilt_list_with_the_same_files_is_the_same_deps`*

**R6.4.2** An `Rc<T>` compared with itself stops at the pointer when `T: Eq`,
and reads the contents otherwise. This is the cheap way to carry a large value
through deps, and the reason a memo hands one back.
*test: `an_rc_compared_with_itself_reads_no_contents`*

**R6.4.3** `Always` never equals itself, so its slot runs every render. `()`
always equals itself, so its slot runs once. These are React's omitted
dependency array and its empty one.
*test: `always_deps_run_every_render`*
*test: `unit_deps_run_once`*

**R6.4.4** A memo computes on the render that mounts it and on the first
render where its deps differ. Otherwise `compute` is not called and the same
`Rc` is returned.
*test: `a_memo_with_equal_deps_is_not_recomputed`*

**R6.4.5** A memo's `Rc` is kept for as long as the component lives and its
deps hold. Nothing else drops it. React reserves the right to throw its cache
away and says to treat `useMemo` as a performance hint; this does not, so the
identity may be relied on — including as another slot's deps.
*test: `a_memo_survives_an_unrelated_state_write`*

**R6.4.6** An effect's `run` is recorded during the render and called after
the frame is painted, never during it. A state write from `run` marks its
scope for the next frame, which is why an effect that writes on every run
never settles.
*test: `an_effect_runs_after_the_frame_that_queued_it`*

**R6.4.7** When deps change, the old cleanup is called before the new `run`.
An effect that returns `()` has nothing to call.
*test: `changing_the_deps_cleans_up_before_it_runs_again`*

**R6.4.8** A layout effect's slot follows every rule above, with one word
changed: its `run` is called after layout and before paint rather than after
it. A state write from `run` therefore sends the frame round again (R5.8.2)
instead of waiting for the next one — which is what it is for, and what it
costs.
*test: `a_layout_effect_runs_before_the_frame_it_belongs_to`*

**R6.4.9** Within one scope, every layout effect runs before any ordinary
effect, whatever order the hooks were called in. Across scopes both queues are
cleanups deepest first, then setups shallowest first.
*test: `layout_effects_run_before_effects_in_the_same_component`*

---

## 7. Paint

Paint is unconditional. Reconciliation never decides *what* is painted, only
*who* is painting. `ratatui::Terminal` diffs the previous and current cell
buffers and writes only what changed, which is where "emit fewer writes"
already happens and where it should stay.

### 7.1 The walk

**R7.1.1** Depth-first, in child order, parent before children. Later wins:
where two nodes overlap, the one painted later is what the reader sees, and the
one painted later takes the click (R8.1.2).
*test: `a_later_sibling_paints_over_an_earlier_one`*

**R7.1.2** A node with `layout.fill` fills its own rectangle with that style
before its children are painted.
*test: `a_container_fills_before_its_children`*

**R7.1.3** Each node is painted with `area` (its own rectangle) and `clip`
(`area` intersected with every clipping ancestor). A node whose `clip` is empty
is not painted and its children are not visited.
*test: `a_node_clipped_to_nothing_is_skipped`*

**R7.1.4** Paint writes cells and nothing else. A state write called from
inside a paint callback panics (P7.2). Captured state snapshots
and `Ref::current` are legal. A write through a ref is not caught — one guard
serves both — and every painter after it in the walk reads a value the layout
never saw.
*test: `setting_state_while_painting_is_refused`*

**R7.1.5** The paint walk records, for every node that has a listener or is
focusable, its scope, its rectangle, its clip and its paint order. This
replaces `draw/screen_map.rs`, which the application built by hand and then had
to clear in one place, fill in three and read in a fourth.
*test: `every_listening_node_is_recorded_where_it_landed`*

**R7.1.6** Focus order is paint order.
*test: `tab_moves_focus_in_paint_order`*

### 7.2 The `Canvas` contract

A `Canvas` is handed `Paint<'_>` and may write anywhere inside `clip()`.
Everything in `crates/ui/src/render/` stays exactly as it is — raw painting
functions taking `&mut Cells`, called from inside a canvas — and so do
`draw/buffer/side_by_side.rs`, `inline.rs`, `single_file.rs` and
`explorer/`.

**R7.2.1** A canvas must not write outside `clip()`. In a debug build the
runtime enforces it: the cell grid is snapshotted before the callback and every
changed cell is checked afterwards, and a cell outside `clip()` panics (P7.3).
In a release build the contract is stated and not checked.
*test: `a_canvas_that_paints_outside_its_clip_is_caught`* — the test paints one
cell one column to the left of its area and expects the panic.

**R7.2.2** A canvas is not asked to measure. It is given a rectangle and paints
into it (R5.2.4).
*test: `a_canvas_asks_for_nothing`*

**R7.2.3** A canvas is `Rc<dyn Fn(&mut Paint<'_>)>`, rebuilt every render. It
captures that render's small state snapshots and `Copy` refs to large models,
so rebuilding it clones no buffer or store.
*test: `a_canvas_closure_reads_this_frames_state`*

### 7.3 `Piece::droppable` stays inside a `Canvas`

`crates/ui/src/draw/buffer/explorer/view_line.rs` drops row pieces by priority
when the row is too narrow, then truncates the widest survivor with an ellipsis,
measuring in terminal cells through `line-index`.

**It stays in the canvas. It does not become a layout feature.** A layout pass
hands out rectangles; this decides *which text to write* — it deletes a whole
priority level at a time so a count never keeps its opening bracket and loses
its closing one, and it cuts a wide glyph rather than through it. A layout
engine that could express that would have to own text, a grapheme walker and a
cell-width table, all of which live in `render` and `line-index` and are tested
there. The same goes for `draw/status.rs::name`, which drops the directory
before the file name for the same reason.

*test (unchanged, in `view_line.rs`): `the_lowest_priority_goes_first`*
*test (unchanged, in `status.rs`): `the_directory_goes_before_the_file_name_does`*

---

## 8. Events

Three payloads, one walk. A key starts at focus; a mouse event starts at the
deepest node under the pointer; a focus change starts at the scope that gained
or lost it. All three then climb.

### 8.1 Hit-testing

**R8.1.1** The target of a mouse event is the node whose rectangle *and* clip
both contain the point.
*test: `a_click_lands_in_the_pane_it_is_over`*

**R8.1.2** When several nodes contain the point, the one painted last wins, so
an overlay takes the click.
*test: `an_overlay_takes_the_click_from_what_is_under_it`*

**R8.1.3** A hidden node is not hit.
*test: `a_hidden_pane_takes_no_clicks`*

**R8.1.4** `Mouse::local` is the point relative to the target's `area.x`,
`area.y`. This is what `ScreenMap::TextArea::to_pos` computes by hand today.
*test: `local_position_is_relative_to_the_node`*

**R8.1.5** A move is delivered whether or not a button is held; `Mouse::button`
is the difference, and is `None` when none is. There is no drag event of its
own, which is how the DOM does it too.
*test: `a_move_with_no_button_still_reaches_the_node`*

### 8.2 Focus

**R8.2.1** A node is focusable when its `focusable` flag is set, which is the
DOM's `tabindex`. Focus is one `ScopeId`, or none. A scope records only whether
it *is* focusable; `NodeHandle::has_focus` and
`Paint::has_focus` are comparisons against that single `ScopeId`, so two scopes
cannot both believe they hold focus and no flag has to be cleared when it moves.
*test: `only_one_node_holds_focus`*

**R8.2.2** `focus_next` moves to the next focusable in paint order,
wrapping; `focus_previous` moves back. Both are no-ops when nothing is
focusable. A browser does this itself on Tab, off the same `tabindex` flag; a
terminal has no such convention, so a key listener calls them.
*test: `focus_wraps_rather_than_running_off_the_end`*

**R8.2.3** A left mouse-down focuses the nearest focusable node at or above the
target, unless a listener between them returned `Bubble::Stop` first.
*test: `clicking_a_pane_focuses_it`*

**R8.2.4** `on_blur` is called on the scope losing focus and `on_focus` on the
scope gaining it, during the dispatch that moved focus — not on the next frame.
Blur runs first, as in a browser.
*test: `losing_focus_is_reported_before_the_next_frame`*

### 8.3 Bubbling

**R8.3.1** A key goes to the focused scope, then to its parent, then to its
parent, up to the root, stopping at the first listener that returns
`Bubble::Stop`. With no focus it starts at the root.
*test: `an_unhandled_key_reaches_the_root`*

**R8.3.2** A mouse down, move or up goes to the hit node and then climbs.
A wheel event is routed by position and climbs the same way.
*test: `a_wheel_over_an_unlistening_child_reaches_the_pane`*

**R8.3.3** `on_focus` climbs from the scope that gained focus, `on_blur` from
the scope that lost it, so a pane hears that something inside it took focus. A
browser's own `focus` and `blur` do not climb; React's do, and this follows
React.
*test: `a_pane_hears_that_something_inside_it_took_focus`*

**R8.3.4** A scope that has been unmounted during the same dispatch is skipped
by the rest of the walk.
*test: `a_listener_that_closes_its_own_pane_does_not_climb_into_a_ghost`*

**R8.3.5** `Tree::press` and `Tree::mouse` return whether a listener stopped the
event.
*test: `an_unbound_key_reports_that_nobody_took_it`*

The climb is exactly the layering `input/keymap.rs::live()` walks by hand today
— buffer, then pane, then tab, then view, then program, innermost first, inner
shadowing outer. **The tree is that layering.** The keymap tables stay `const`
and printable, and `Resolver` stays a pure function of its own state plus one
key, so `lint-arch`'s clock rule over `crates/ui/src/input` still holds.

### 8.4 Pointer capture

**R8.4.1** `capture_pointer()` inside a mouse listener routes every subsequent
mouse event to that scope until `release_pointer()`, the button comes up, or the
scope unmounts.
*test: `a_drag_that_leaves_the_column_still_belongs_to_it`*

**R8.4.2** A captured event still reports `local`, which may be negative on
screen and is therefore saturated to the node's edge.
*test: `dragging_above_a_pane_reports_its_top_row`*

There is no capture phase. Its one use in a browser is intercepting before the
target sees an event, and nothing in this program does that.

---

## 9. Workers

`loom` never names a worker, a request or a response. It provides two ways of
being answered later, and the rules under which an answer is refused. The
application builds the requests, owns the threads, and hands the answers back.

### 9.1 Asking, and being answered

An asynchronous call in JavaScript is three parts, and this program already has
all three:

| a promise | here |
|---|---|
| the work runs somewhere else | a worker thread |
| the answer lands on a queue | `Sender<Event>` |
| one loop drains the queue and calls the handler | the `rx.recv()` loop of §15.2 |

So there is no executor to add and nothing to poll. A component asks through a
handle it reads from context, and attaches a handler to what comes back:

```rust
diffs.open(file).then(move |response| { … });
spans.colour(requests).subscribe(move |response| { … });
```

The handler does not run inside the render that asked, or inside the effect. It
runs from the loop, between frames, with the owning scope entered — the same
guarantee JavaScript gives by running a promise handler in a microtask rather
than in the middle of the function that started the work.

```rust
Promise<T>     one answer, then the address is spent     the file worker
Observable<T>  answers in pieces, until complete         the syntax worker
```

Each is opened as a pair inside an effect body: the asker keeps the `Promise`
or `Observable`, and whoever sent the request keeps the `Resolver` or
`Observer`. Both halves carry `(ScopeId, slot: u16, generation: u64)` plus a
`Weak` to the runtime. A `Weak` rather than the thread-local, because `Session`
delivers an answer from outside a frame; `resolve` and `next` enter the runtime
for the duration of the handler and leave it afterwards.

**R9.1.1** `Resolver::resolve` runs the handler with the owning scope entered,
marks the address spent, and returns `true`. A second call cannot happen —
`resolve` takes `self`.
*test: `a_promise_is_resolved_once`*

**R9.1.2** `Observer::next` runs the handler with the owning scope entered and
returns `true`. It may be called any number of times until `complete`.
*test: `an_observable_delivers_every_piece`*

**R9.1.3** A handler may set state and ask for more work. It may not call a
hook — it holds no `Scope`.
*test: `an_answer_handler_may_set_state`*

**R9.1.4** `Observer<T>` is `Clone`, so one request answered in pieces can be
delivered from several places. `Resolver<T>` is not. `complete` takes `self`
and closes the address for every clone; dropping one clone closes nothing
until the last of them goes.
*test: `an_observer_can_be_held_twice`*

**R9.1.5** A `Promise` or `Observable` dropped without `then` or `subscribe`
closes its address, and the answer is refused. Both are `#[must_use]`, so
forgetting is a warning before it is a silence.
*test: `a_promise_nobody_handled_refuses_its_answer`*

### 9.2 What the application carries

`pipeline` and `syntax` are untouched. No framework type crosses into them, and
no `u64` token is added to `pipeline::file::Request`.

The reason: the file worker already has one replaceable slot and its `Response`
already carries the `File` it answers, so a token would be an address only `ui`
could read, living in a crate that cannot read it. The syntax worker already
carries `key` and `version` for the same purpose.

The address cannot travel either — it names a scope in a thread-local runtime,
so it is not `Send`. Something on this side has to hold it until the thread
answers. That something is one handle per worker, offered as context: the shape
a web application gets from a `QueryClient` or an `ApolloClient`, and an
Angular one from an injected service. Each handle owns its channel and the
addresses it has not answered yet.

```rust
// crates/ui/src/app/worker.rs
/// The file worker, as a component sees it.
pub struct Diffs {
    worker: RefCell<pipeline::file::FileWorker>,
    waiting: RefCell<Option<loom::Resolver<pipeline::file::Response>>>,
}

impl Diffs {
    /// Asks for a diff. An earlier ask is dropped, because the worker has one
    /// replaceable slot; its promise is then refused (R9.1.5).
    pub fn open(&self, file: file_types::File)
        -> loom::Promise<pipeline::file::Response>
    {
        let (resolver, promise) = loom::promise();
        *self.waiting.borrow_mut() = Some(resolver);
        self.worker.borrow_mut().send(file);
        promise
    }

    /// What the loop calls when the thread answers. One value, then done —
    /// so this is `resolve`, the name JavaScript gives the kept end of a
    /// promise.
    pub fn resolve(&self, response: pipeline::file::Response) {
        self.worker.borrow_mut().received(&response);
        if let Some(resolver) = self.waiting.borrow_mut().take() {
            resolver.resolve(response);
        }
    }
}

/// The syntax worker. Several requests are in flight at once and each is
/// answered in pieces, so the observers wait under the key the response
/// carries — the key the request already had.
pub struct Spans {
    worker: RefCell<syntax::Syntax>,
    waiting: RefCell<HashMap<String, loom::Observer<syntax::SyntaxResponse>>>,
}

impl Spans {
    /// Asks for colour. Every request in the batch answers the one
    /// observable, so the observer is cloned once per key.
    pub fn colour(&self, requests: Vec<syntax::SyntaxRequest>)
        -> loom::Observable<syntax::SyntaxResponse>;

    /// What the loop calls when a piece arrives. More may follow, so this is
    /// `next`, the name RxJS gives the kept end of a stream. Drops that key's
    /// observer on the piece saying `more == false`; the pane hears the end
    /// when the last of them goes (R9.1.4).
    pub fn next(&self, response: syntax::SyntaxResponse);
}

/// The worktree listing. Nothing asks this one a question — it changes on its
/// own, when the watcher sees the disk move — so it is a store rather than a
/// handle, and a component reads it with `use_sync_external_store`.
pub struct Files {
    worker: RefCell<pipeline::list::ListWorker>,
    root: PathBuf,
    current: RefCell<loom::Snapshot<[File]>>,
    /// Keyed, so the `Subscription` `subscribe` hands back knows which entry
    /// to take out again.
    readers: RefCell<HashMap<u64, loom::Notify>>,
    next_reader: Cell<u64>,
}

impl loom::ExternalStore for Files {
    type Value = [File];
    fn subscribe(&self, notify: loom::Notify) -> loom::Subscription;
    fn snapshot(&self) -> loom::Snapshot<[File]>;
}

impl Files {
    /// What the loop calls when the watcher fires. Asks the worker to rescan.
    pub fn changed(&self);

    /// What the loop calls when the worker answers. Always makes a new
    /// `Snapshot`, even for a listing equal to the last: what moved is the
    /// disk, and whether a diff really changed is the file worker's answer to
    /// give (R9.5.3).
    pub fn listed(&self, files: Rc<[File]>);
}
```

`RefCell` because context hands out `Rc<T>` and a component holds it by shared
reference. No borrow is held across a call into `loom`: `resolve` takes the
resolver out, drops the borrow, and only then resolves, and `listed` swaps the
snapshot in and drops that borrow before it notifies anybody.

The loop's half is four lines, and none of them knows what a diff is or which
pane wanted one:

```
Event::FileReady(response) => diffs.resolve(response)
Event::Coloured(response)  => spans.next(response)
Event::ListRefreshed(list) => files.listed(list)
Event::FsChanged(_)        => files.changed()
```

The verbs are the shapes. `resolve` is one value and the question is closed;
`next` is one piece and more may follow. A reader who knows
`Promise.withResolvers()` and RxJS's `Subject` already knows which is which,
and that is worth more here than lines that rhyme.

Quit, suspend and rebuild never were worker requests. They shared this path
only because it was the one way out of a component. They are a callback the
application hands `Screen` as a prop, which is the pattern §3.7 describes.

### 9.3 Generation rules

**R9.3.1** An answer is refused when the scope's slab generation has moved on —
the component unmounted.
*test: `a_reply_for_a_component_that_went_away_is_refused`*

**R9.3.2** An answer is refused when the slot no longer holds an effect, or
holds a different hook — the component's hook order changed.
*test: `a_reply_into_a_slot_that_changed_shape_is_refused`*

**R9.3.3** An answer is refused when the effect's generation has moved on — the
effect's deps changed and it ran again, so this address belongs to a question
nobody is asking any more. This is what makes
`if self.selected.as_ref() != Some(&response.file) { return false }` in
`app/workers.rs` disappear: the address is stale, not the value.
*test: `a_diff_for_a_file_the_reader_left_is_refused`*

**R9.3.4** A refused answer is not an error. Returning `false` is the whole
report; the value is dropped.
*test: `a_refused_reply_is_dropped_quietly`*

**R9.3.5** An effect's cleanup closes every address it opened, before the
cleanup function is called. This is what RxJS asks for by handing back a
`Subscription` to `unsubscribe()`, done by the runtime instead.
*test: `changing_the_file_closes_the_previous_diff_address`*

### 9.4 Syntax spans, arriving in pieces

The syntax worker answers a request with several `SyntaxResponse`s, oldest
first, each carrying `from`, `spans` and `more`. The question is how a pane
re-renders when spans arrive for lines it shows, without every pane re-rendering
on every piece.

**The mechanism is two parts: an addressed observable, and an overlap test.**

*Addressed:* the answer reaches one scope, because the `Observer` names one
scope's effect slot. No other component is told anything. A pane that is not
showing that file has no address waiting and hears nothing.

*Overlap test:* the handler installs the piece into the shared store and then
redraws **only if the piece covers a line the pane is showing**:

```rust
let from = response.from;
let taken = store.current().install(response);
if taken && from < visible.end {
    took_spans(&|n| n + 1);
}
```

`visible.end` is the last visible **view** line. It is a sound upper bound on
the last visible **file** line, because a filler consumes a view line without
consuming a file line, so the file line at any position is never greater than
the view line there. The test therefore never skips a redraw that was needed,
and at worst redraws once for a piece that turned out to be just off screen.

**R9.4.1** A piece the store refuses — wrong version, or not starting where the
last one ended — causes no redraw.
*test: `a_stale_span_batch_changes_nothing`*

**R9.4.2** A piece entirely below the visible range is installed and causes no
redraw, so scrolling to it later finds it already there. The worker reads ahead
by a margin, and reading ahead must not cost a frame.
*test: `a_batch_for_lines_below_the_fold_does_not_redraw`*

**R9.4.3** A piece covering a visible line marks exactly one scope. Every other
pane keeps its last child tree and is painted without being run (R6.3.3).
*test: `a_span_batch_runs_one_component`* — asserted with
`Harness::render_count() == 1`.

**R9.4.4** The store is one `use_ref(scope, Store::new)` at the root, provided
as context by its `Ref<Store>` handle, which is `Copy`. Reading it costs no
clone and writing it is silent, so installing spans never by itself redraws
anything.
*test: `installing_spans_does_not_redraw_by_itself`*

**R9.4.5** The pane asks for more by depending on how much has arrived. Its
effect's deps are

```rust
(key_original, key_modified, version, visible.end, coloured_original, coloured_modified)
```

where `coloured_*` is `Store::get_lines_coloured`. Installing a piece changes
that number, the effect re-runs, and it sends the next request from where the
last one stopped. This is the reactive form of the loop's current
`send_colour_request()` after every event, and it stops on its own when the
store already holds enough — which is `colour::request`'s existing "sends
nothing when the store already holds enough" rule, unchanged.
*test: `a_long_file_keeps_asking_until_the_visible_range_is_coloured`*

**R9.4.6** No frame is drawn when nothing was marked. A batch that fails the
overlap test leaves `Tree::needs_draw()` false and the loop draws nothing.
*test: `a_batch_off_screen_draws_no_frame`*

### 9.5 Reading the open file again

The loop asks the file worker on every turn — `send_file_request()` is called
after every event in `app/mod.rs`, and it sends the selected file whether or
not anything changed. Most of those answers are the same diff read again, and
the one thing that arrangement buys is that a file edited on disk is picked up
without anyone having to notice.

An effect keyed on the chosen file alone would lose that. The reader stays on
`a.rs`, the file changes underneath them, `chosen` is the same `File`, the
deps compare equal and nothing is read again.

The listing belongs outside the tree. It changes because the disk changed,
which is the definition of an external store, and React has a hook for reading
one: `useSyncExternalStore`. `Files` is the store, `Screen` subscribes, and
the snapshot it gets back is a value that moves when the disk does.

**R9.5.1** `Screen` reads the listing with `use_sync_external_store`. The
effect that reads the open file has deps `(chosen, files)`, where `files` is
the snapshot, so a change on disk moves the deps on and the file is read
again.
*test: `a_file_edited_on_disk_is_read_again`*

**R9.5.2** A snapshot that moves while an earlier read is in flight refuses the
earlier answer, by R9.3.3 — the deps changed, so the address is stale. The
reader sees the newer content, never the older answer landing after it.
*test: `a_second_disk_change_refuses_the_first_answer`*

**R9.5.3** A change on disk that leaves the worktree listing identical still
produces a new snapshot. `Snapshot` compares by identity, the way React
compares `getSnapshot`'s result with `Object.is`, so a fresh listing is a
different value even when it holds the same paths. Deciding whether a diff
really changed is the worker's answer, not a guess made before asking.
*test: `a_touched_file_is_read_again`*

This turns "ask every turn, and filter the answers" into "ask when something
changed". The number of reads drops from one per event to one per change, and
the request goes out from the component that shows the result.

---

## 10. Context

A context is a type you declare. That one type is the key a reader names, the
element a provider writes, and the home of the default value — React's
`createContext`, spelled as a declaration.

```rust
context! {
    /// The palette every painter reads.
    pub ThemeContext: Theme = Theme::dark();
}
```

```rust
rsx! {
    ThemeContext { value: dark,
        ExplorerPane { files }
    }
}
```

```rust
let theme = use_context::<ThemeContext>(scope);
```

Those three are `createContext(Theme.dark())`, `<ThemeContext value={dark}>`
and `useContext(ThemeContext)`, in that order — one argument each, and the
`Context` suffix included, because that is what React's own documentation
calls it.

**R10.1.1** `context!` declares a marker type and a props struct with the
fields `value` and `children`, and implements `Context`, `Component` and
`Element` for the marker. The expansion is `#[component]`'s (§11.4) plus the
`Context` impl; rendering it offers `props.value` and returns `props.children`.
*test: `a_declared_context_is_both_an_element_and_a_key`*

**R10.1.2** `use_context::<C>(scope)` walks `parent` until it reaches a scope
that offered `C`, and returns a clone of the value. A nearer provider shadows
a further one.
*test: `a_nearer_provider_shadows_a_further_one`*

**R10.1.3** With no provider above it, `use_context` returns
`C::default_value()`. Reading a context is never an error, so there is nothing
to catch and no fallible form.
*test: `a_read_with_no_provider_is_the_default`*

**R10.1.4** A context is identified by its marker type, not by the type of its
value. Two contexts carrying one value type stay apart, so `Title` and
`Subtitle` can both be `Rc<str>` without a newtype between them.
*test: `two_contexts_of_one_value_type_are_two_contexts`*

**R10.1.5** A provider offers on its own scope. The value reaches its children
and nothing else: a component that writes two providers gives two subtrees two
values, and neither reaches a sibling. A component does not see the value it
offers — it reads what its own ancestors offered.
*tests: `a_provider_does_not_reach_its_sibling`,
`one_component_gives_two_subtrees_two_values`,
`a_component_does_not_see_its_own_offer`*

**R10.1.6** Each offer carries a version. A provider whose `value` satisfies
`C::same` against the value it offered last leaves the version where it is;
otherwise the version moves. `context!` fills `same` in with `==`, so an
ordinary declaration says no more than `createContext(default)` does.
*tests: `an_equal_value_leaves_the_version_alone`,
`a_changed_value_moves_the_version`*

**R10.1.7** A read is recorded on the reading scope as `(TypeId, version)`,
replacing the entry already there for that type if there is one. A memoised
scope whose recorded version is behind is re-rendered even though its props did
not change. Without this, memoisation is a correctness bug rather than an
optimisation.
*tests: `a_memo_component_whose_context_changed_runs_anyway`,
`a_second_read_replaces_the_version_it_recorded`*

**R10.1.8** `use_context` obeys §4.3 like every other hook: top level of a
component, never inside an `if`, a `match`, a loop or a closure. It holds no
hook slot, so the runtime check cannot see it and only `#[component]` catches a
misplaced call; the rule stands because a reader who knows React expects it.
*test: `use_context_in_a_branch_does_not_compile`*

React compares context values with `Object.is`, which works on anything because
every JavaScript value has one representation. Rust has two kinds of sameness —
same bits and same allocation — and no one trait covers both for every type. So
`context!` uses `==`, and a value that has no `==` names the comparison it does
have:

```rust
context! {
    pub OpenContext: Rc<dyn Fn(File)> = Rc::new(|_| {}), same = Rc::ptr_eq;
}
```

`Rc::ptr_eq` is `Object.is` exactly, and it asks nothing of what the `Rc`
holds. Four of this program's seven contexts need no `same`; the three that do
are the two callbacks and the `Rc<Spans>`, none of which can be compared any
other way.

Storage is a `Vec<(TypeId, Rc<dyn Any>, u64)>` on the scope, not a `HashMap`:
no component in a terminal program offers more than a handful of values, and a
linear scan of a handful beats hashing one.

The contexts this application has, and where they are offered:

| context | value | offered by | read by |
|---|---|---|---|
| `ThemeContext` | `theme::Theme` (`Copy`) | `Screen` | every component that paints |
| `SyntaxContext` | `Ref<syntax::Store>` (`Copy`) | `Screen` | `DiffPane` |
| `InputContext` | `Ref<input::Resolver>` (`Copy`) | `Screen` | `ExplorerPane`, `DiffPane` |
| `StatusContext` | `SetState<Status>` (`Copy`) | `Screen` | `ExplorerPane`, `DiffPane` |
| `SpansContext` | `Rc<worker::Spans>` | `Screen` | `DiffPane` |
| `OpenContext` | `Rc<dyn Fn(File)>` | `Screen` | `ExplorerPane` |
| `RunContext` | `Rc<dyn Fn(Command)>` | `Screen` | `ExplorerPane`, `DiffPane` |

`Screen` writes those seven as seven nested providers, which is what the same
program looks like in React.

### Talking to an ancestor

A child that has something to tell an ancestor calls a function the ancestor
gave it. A parent passes it as a prop; anything deeper reads it from context.
Both are ordinary values, and a callback context declares
`same = |a, b| Rc::ptr_eq(a, b)` so that handing down the same closure twice
does not move the version.

There is no upward-routed payload. A framework one would let a child announce
something into the air and let whichever ancestor happens to be listening take
it, which reads well in a demo and fails silently when nobody is: no compiler
error, no panic, nothing on screen. A callback read from a context with no
provider is `C::default_value()`, which for `OpenContext` is a closure that does
nothing — so declare that default as one that logs, and a missing provider says
so at run time instead of vanishing. A callback that is not passed as a prop
does not compile.

What it costs is that every level between the two must carry the value. Context
is the answer to that, and it is the same answer React gives.

---

## 11. `rsx!` grammar

A proc macro. `macro_rules!` cannot do `#[component]` at all — an attribute
macro must be a proc macro — gives error spans pointing at the macro rather than
at your typo, and cannot parse `Name { prop: expr, Child {} }` without a
token-eating recursion that hits the recursion limit on a tree of any depth.

### 11.1 EBNF

```ebnf
rsx        = node* ;

node       = element | text | block | branch | loop ;

element    = path , "{" , { entry , [ "," ] } , "}" ;
entry      = prop | rest | node ;
prop       = ident , ":" , expr ;
rest       = ".." ;

text       = string-literal ;
block      = "{" , expr , "}" ;

branch     = if-chain | match-arms ;
if-chain   = "if" , ( expr | let-cond ) , "{" , rsx , "}" ,
             { "else" , "if" , ( expr | let-cond ) , "{" , rsx , "}" } ,
             [ "else" , "{" , rsx , "}" ] ;
let-cond   = "let" , pattern , "=" , expr ;
match-arms = "match" , expr , "{" , { pattern , [ "if" , expr ] , "=>" ,
             "{" , rsx , "}" , [ "," ] } , "}" ;

loop       = "for" , pattern , "in" , expr , "{" , rsx , "}" ;

path       = ident , { "::" , ident } ;
```

`key` is a reserved prop name. The macro consumes it; it never reaches the props
struct.

`ref` is the other reserved name, spelled the way React spells it. It is a Rust
keyword, so `ident` here means `syn`'s `Ident::parse_any` rather than its
strict `Ident` — measured, because the strict one answers ``expected
identifier, found keyword `ref` ``. Unlike `key` it does reach the props
struct, as the field `node_ref`.

The comma between entries is optional, so children read as a list of elements
rather than as a list of arguments. `..` may be written anywhere among the
entries; the expansion always places it last, where Rust requires it.

### 11.2 What each form expands to

| written | becomes |
|---|---|
| `Name { a: x, b: y }` | `<Name as Element>::build(NameProps { a: x, b: y }, None)` |
| `Name { key: k, a: x }` | `<Name as Element>::build(NameProps { a: x }, Some(Key::from(k)))` |
| `Name { a: x, .. }` | `<Name as Element>::build(NameProps { a: x, ..Default::default() }, None)` |
| `Name {}` | `<Name as Element>::build(NameProps::default(), None)` |
| `Name { Child {} }` | `NameProps { children: vec![ /* the child */ ], .. }` |
| `"text"` | `<Text as Element>::build(TextProps { text: "text".into(), ..Default::default() }, None)` |
| `{ expr }` | `Node::from(expr)` |
| `if c { A } else { B }` | `if c { Node::Fragment(vec![A]) } else { Node::Fragment(vec![B]) }` |
| `if c { A }` | `if c { Node::Fragment(vec![A]) } else { Node::Empty }` |
| `for x in xs { A }` | `Node::Fragment(xs.into_iter().map(\|x\| A).collect())` |
| two or more top-level nodes | `Node::Fragment(vec![…])` |
| no nodes | `Node::Empty` |

### 11.3 Rules

**Props are a plain struct literal.** That single decision buys every error
message the macro would otherwise have to invent:

| mistake | message |
|---|---|
| `focusd: true` | ``struct `ExplorerPaneProps` has no field named `focusd` `` |
| omitted `files` | ``missing field `files` in initializer of `ExplorerPaneProps` `` |
| `focused: 1` | ``expected `bool`, found integer`` |

No typestate builder beats that, and `typed-builder` does not try — it produces
"the method `build` exists but its trait bounds were not satisfied".

**`..` means the defaults, and writing no props implies it.** One sentence:
*write no props and you get the defaults; write `..` and you get the rest of
them.* Built-in hosts derive `Default`; your components normally do not, so
their props are all required, which is what you want.

**Children are the props field named `children`.** A component that does not
declare one and is given children gets ``no field `children` ``; one that
requires them and is given none gets ``missing field `children` ``. Both free.

**A `for` body must carry a key.** Checked at expansion when the macro can see
the element, and at reconciliation otherwise (R6.1.2).

**Lowercase is not special.** `Row` and `ExplorerPane` are both paths and both
expand the same way. There is no separate grammar for built-in hosts.

**`ref` only goes on a host.** Every built-in host's props carry `node_ref`;
your components' do not, so `ref` on one is ``struct `ExplorerPaneProps` has no
field named `node_ref` ``. React forwards a ref through a component; there is
nothing here to forward it to, because a component owns no rectangle.

### 11.4 `#[component]`

```rust
#[component]
pub fn StatusBar(scope: &mut Scope, file: Option<Rc<File>>, view_line: u32) -> Node { … }
```

expands to

```rust
pub struct StatusBar;

pub struct StatusBarProps {
    pub file: Option<Rc<File>>,
    pub view_line: u32,
}

impl ::loom::Component for StatusBar {
    type Props = StatusBarProps;
    const NAME: &'static str = "StatusBar";
    fn render(props: &StatusBarProps, scope: &mut ::loom::Scope) -> ::loom::Node {
        let StatusBarProps { file, view_line } = props;   // by reference
        { /* your body */ }
    }
}

impl ::loom::Element for StatusBar {
    type Props = StatusBarProps;
    fn build(props: StatusBarProps, key: Option<::loom::Key>) -> ::loom::Node {
        ::loom::Node::part::<Self>(props, key)
    }
}
```

The first parameter must be `scope: &mut Scope`; every other parameter becomes a
props field, in order, with its own type. A parameter named `children:
Children` receives the element's children. That is the whole contract.

`#[component(memo)]` additionally requires `Props: PartialEq` and fills in
`Part::same`. `PartialEq` is not a bound on props in general, because
`Rc<Alignment>` has no meaningful equality and should not be made to invent one.
Memoisation is opt-in, as in React.

`#[component]` also refuses a direct `use_*` call inside an `if`, a `match`, a
loop, a closure or after an early `return`, at compile time, in the body it can
see. Custom hooks — ordinary functions taking `&mut Scope` — are covered by the
runtime check instead (§4.3).

### 11.5 `context!`

```rust
context! {
    /// The palette every painter reads.
    pub ThemeContext: Theme = Theme::dark();
}
```

expands to

```rust
/// The palette every painter reads.
pub struct ThemeContext;

pub struct ThemeContextProps {
    pub value: Theme,
    pub children: ::loom::Children,
}

impl ::loom::Context for ThemeContext {
    type Value = Theme;
    fn default_value() -> Theme { Theme::dark() }
    fn same(old: &Theme, new: &Theme) -> bool { old == new }
}

impl ::loom::Component for ThemeContext {
    type Props = ThemeContextProps;
    const NAME: &'static str = "ThemeContext";
    fn render(props: &Self::Props, scope: &mut ::loom::Scope) -> ::loom::Node {
        ::loom::offer::<Self>(scope, &props.value);
        ::loom::Node::Fragment(props.children.clone())
    }
}

impl ::loom::Element for ThemeContext {
    type Props = ThemeContextProps;
    fn build(props: ThemeContextProps, key: Option<::loom::Key>) -> ::loom::Node {
        ::loom::Node::part::<Self>(props, key)
    }
}
```

Everything after `#[component]`'s own expansion is the same, which is the
point: a provider is a component, so `rsx!` needs no rule for it and §11.1's
`path` needs no generics.

`same = expr` after the default replaces that one line, for a value with no
`==`:

```rust
context! {
    pub OpenContext: Rc<dyn Fn(File)> = Rc::new(|_| {}), same = Rc::ptr_eq;
}
```

It is a proc macro for one reason: `macro_rules!` cannot build the identifier
`ThemeContextProps` from `ThemeContext`, and §11.2 finds a props type by that
name.

---

## 12. Panics

Exhaustive. Everything else in `loom` answers with `Option`, `Result` or `bool`.

`{…}` is filled in at the panic site. Each is asserted by a test with
`#[should_panic(expected = …)]` naming the fixed part of the message.

**P4.1 — a hook slot changed shape between renders.**

```text
StatusBar: hook 2 was a State at crates/ui/src/status.rs:14, and is an Effect
here at crates/ui/src/status.rs:19. Hooks must run in the same order every
render — none inside an if, a loop, or after an early return.
```

Call sites are named in a debug build; in a release build the two `at …`
clauses are omitted.

**P4.2 — a render returned early, or a hook sits behind a condition.**

```text
StatusBar: this render called 3 hooks and the last one called 5. Hooks must run
in the same order every render.
```

**P4.3 — a setter or ref was captured into something that outlived its component.**

```text
a SetState was used after ExplorerPane was removed
```

**P4.4 — a setter or ref was used with no runtime entered.**

```text
state setters and refs may only be used while loom is running a component, listener, effect, worker reply or painter
```

**P4.5 — `current` on a ref that is already lent out.** One guard per ref at a
time. Different refs in one expression are legal, and are how a canvas reads a
buffer, a viewport and a store at once.

```text
a Ref was borrowed from inside its own borrow
```

**P4.6 — `promise` or `observable` outside an effect body.** Both read the
effect the runtime is running, so there is no slot to answer.

```text
a promise may only be opened while loom is running an effect
```

**P5.1 — R5.8.3.**

```text
DiffPane ran 17 times in one frame — a component that sets state on every render never settles
```

**P6.1 — R6.1.2.**

```text
Screen: child 2 has a key and child 3 does not — key every child of a list or none of them
```

**P6.2 — R6.1.5.**

```text
ExplorerPane: two children share the key "src/main.rs"
```

**P6.3 — R6.2.9.**

```text
DiffPane: one ref was given to both a Canvas and a Row — a ref names one node
```

**P7.1 — R5.8.4.**

```text
draw was called from inside a paint callback
```

**P7.2 — R7.1.4.** A state write while painting is forbidden;
captured snapshots and `Ref::current` are legal.

```text
DiffPane changed component data while painting — paint callbacks may only write cells
```

**P7.3 — R7.2.1. Debug builds only.**

```text
ExplorerPane painted at (39, 4), outside its clip 0,0 40x9
```

**P14.1 — `Harness::screen_row` out of range.** A test helper: a wrong index is a
broken test, not a condition to handle.

```text
row 9 is outside the 8-row screen
```

### 12.1 What does not panic

| operation | answer instead |
|---|---|
| `use_context::<C>` with no provider | `C::default_value()` |
| `Resolver::resolve`, `Observer::next` | `false` when refused |
| `Notify::changed` after the component has gone | no-op |
| `Tree::press`, `Tree::mouse` | `false` when nobody stopped it |
| a `ref` read before its node has a rectangle | `None` |
| `NodeHandle::area` on an unmounted node | `Rect::ZERO` |
| `NodeHandle::focus`, `focus_next`, `focus_previous` with nothing focusable | no-op |
| a state write ending at an equal value | commits the value, runs no component |
| a container too small for its children | paints its `too_small` node (R5.4) |
| a rectangle of zero width or height | painted as nothing (R5.3.5) |
| `Harness::area_of` for an unknown name | `None` |
| a canvas writing outside the cell grid | dropped by `Buffer::cell_mut`, which answers `Option` |
| a `for` over an empty iterator | `Node::Fragment(vec![])`, which flattens to nothing |
| a component returning `Node::Empty` | a scope with no children |
| the 5th layout round in one frame | painted with round 4's rectangles; `Tree::layout_rounds()` reports 4 (R5.8.2) |
| a reply for a component that unmounted | refused, value dropped (R9.3.1) |
| a promise nobody attached a handler to | refused, value dropped (R9.1.5) |
| a span batch the store refuses | no redraw (R9.4.1) |

Note what is **not** a panic: the root component is never unmounted, so there is
no "the root went away". `Tree::set_props` on a type other than the root's is a
type error, not a panic. And a component that panics is not caught: a panic in a
painter belongs in a backtrace with the terminal restored, which `Screen`'s
`Drop` already arranges.

---

## 13. Invariants

What must hold, and the test that proves it. Each test is written by breaking
the code on purpose first and watching it fail; a test that has never failed
proves nothing.

| # | invariant | test | how to break it and see it fail |
|---|---|---|---|
| **I1** | The screen is a function of state. Two draws with nothing between them produce identical cells. | `two_draws_with_nothing_between_them_agree` | make a component read a counter it increments in its own body |
| **I2** | A component's state survives a re-render of its parent. | `a_component_at_the_same_place_keeps_its_state` | match children by index only, ignoring `type_id` |
| **I3** | A different component at the same place starts fresh. | `a_different_component_at_the_same_place_starts_fresh` | drop the `type_id` comparison in R6.1.3 |
| **I4** | Every cell a canvas writes lies inside its clip. | `a_canvas_that_paints_outside_its_clip_is_caught` | paint one column to the left of the area |
| **I5** | Every rectangle handed to a child lies inside its parent's inner rectangle. | `every_child_rectangle_is_inside_its_parent` | remove the `intersection` in R5.3.3 |
| **I6** | Siblings on one axis do not overlap. | `siblings_do_not_overlap` | set `spacing` to a negative-equivalent by subtracting gap twice |
| **I7** | Children tile the container in order, apart from space no child claimed. | `children_tile_the_container_in_order` | swap `Flex::Start` for `Flex::SpaceAround` |
| **I8** | A wider screen never shows less than a narrower one. | `a_wider_screen_never_shows_less_than_a_narrower_one` | make `too_small` trigger on the *container's* size rather than the child's |
| **I9** | No reply is ever applied to a component that did not ask for it. | `a_diff_for_a_file_the_reader_left_is_refused` | drop the generation from `Resolver` |
| **I10** | Hook slots are read in call order, and a divergence is caught on the render that diverges. | `a_render_that_skips_a_hook_is_refused` | remove the count check at the end of a render |
| **I11** | An effect's cleanup runs before its next setup, and deepest first on unmount. | `unmounting_runs_the_deepest_cleanup_first` | run cleanups shallowest first |
| **I12** | A frame runs a component only when its props changed, its own state changed, or its parent ran. | `a_clean_component_is_painted_without_being_run` | mark every scope on every frame |
| **I13** | `loom` names no application crate. | `cargo xtask lint-arch` | add `use ui::Theme;` to `crates/loom/src/paint/text.rs` |
| **I14** | Every file in `loom` and `loom-macros` is under the 300-line soft cap. | `cargo xtask lint-size` | concatenate `reconcile.rs` and `scope.rs` |
| **I15** | `Session::draw_into(&mut Cells, Rect)` keeps its signature through every migration phase, and `crates/ui/tests/explorer/*` (1,505 lines across six files) and `crates/codediff/tests/screens.rs` pass unchanged. | the existing suites, run at the end of every phase | change any phase's screen output by one cell |
| **I16** | One render sees one state snapshot, and writes compose in call order against the pending value. | `two_writes_compose_in_call_order` | run a write against the render snapshot instead of the pending value |
| **I17** | Mutating a ref never schedules a component by itself. | `editing_a_ref_does_not_redraw` | mark the owner in `Ref::current` |
| **I18** | A memo hands back the same `Rc` until its deps differ, and nothing else drops it. | `a_memo_survives_an_unrelated_state_write` | clear memo slots when a scope is marked |
| **I19** | A `ref` names one node, and holds `None` whenever that node has no rectangle. | `a_ref_is_cleared_when_its_node_goes_away` | leave a stale handle behind after unmount |
| **I20** | A layout effect runs before the frame it belongs to is painted. | `a_layout_effect_runs_before_the_frame_it_belongs_to` | queue it with the ordinary effects |
| **I21** | A component reading an external store re-renders when the store hands back a different snapshot, and stops hearing from it once it unmounts. | `a_store_that_changes_after_unmount_reaches_nobody` | keep the `Notify` alive past the subscription |

### 13.1 The test that is the net

`Session::draw_into` is the seam every migration phase crosses. It draws one
frame into a cell grid with no terminal, and `crates/ui/tests/explorer/common.rs`
turns that grid into `Vec<String>`:

```rust
pub fn screen(session: &mut TestSession, width: u16, height: u16) -> Vec<String> {
    let mut cells = Cells::empty(Rect::new(0, 0, width, height));
    session.draw_into(&mut cells, Rect::new(0, 0, width, height));
    (0..height).map(|y| /* … row as text, trailing blanks trimmed … */).collect()
}
```

Nothing about that changes. Every phase in §15 ends with that suite green, which
is what makes each phase independently revertable.

### 13.2 How a component is unit-tested

Two levels, and the framework provides both.

**One component, in isolation.** `Harness` mounts it with its props and any
context it reads, draws into a grid of a stated size, and reads the screen back
as text — the same shape of assertion the explorer tests already make, one
component down:

```rust
#[test]
fn a_narrow_status_bar_drops_the_summary_rather_than_overlapping_the_path() {
    let mut screen = Harness::new::<StatusBar>(
        StatusBarProps {
            file: Some(Rc::new(File::unchanged_path(at("src/main.rs"), revs()))),
            view_line: 0, view_lines: 100, changes: 3, change: None,
            timed_out: false, exhausted: None, notice: None,
        },
        16, 1,
    )
    .provide::<ThemeContext>(Theme::DARK);

    assert_eq!(screen.draw().row(0).chars().count(), 16);
    assert!(!screen.row(0).contains("changes"));
}
```

**A behaviour, through events.** The harness sends keys and clicks, and answers
questions about the tree:

```rust
#[test]
fn clicking_a_row_moves_the_cursor_and_opens_the_file() {
    let mut screen = Harness::new::<ExplorerPane>(props(), 40, 10)
        .provide::<ThemeContext>(Theme::DARK);
    screen.draw().click(3, 4);
    assert_eq!(screen.row(4).trim_start().split(' ').next(), Some("app.rs"));
}

#[test]
fn a_span_batch_runs_one_component() {
    let mut screen = Harness::new::<Screen>(props(), 80, 24);
    screen.draw();
    // … deliver a batch through the pending …
    assert_eq!(screen.render_count(), 1, "only the pane showing the file");
}
```

The three questions a framework test needs that a screen cannot answer —
"did this component run?", "which scope holds focus?", "where did this node
land?" — are `renders_of(name)`, `focused_name()` and `area_of(name)`. Those
three plus `rows()` are enough for every rule in this document. `tree()` prints
the scope tree as indented text for when an assertion fails and the reader wants
to see why.

---

## 14. Module map

Every file, what it is responsible for, and an estimated length. **These are
estimates. Nothing is built.** The comparable column names a file in this
repository doing a job of similar shape, with its length as `wc -l` reports it
today — that is a measurement; the estimate beside it is not.

### 14.1 `crates/loom`

| file | responsibility | est. | comparable (measured) |
|---|---|---:|---|
| `src/lib.rs` | crate doc, the whole public surface in one screen | 60 | `crates/ui/src/lib.rs`, 27 |
| `src/node.rs` | `Node`, `Host`, `Part`, `Key`, `Element`, `NodeHandle`, the `From` impls | 180 | |
| `src/component.rs` | `Component`, the erased render pointer, the props comparison pointer | 80 | |
| `src/scope.rs` | `Scope`, `ScopeId`, `Mounted`, the slab and its free list, parent and child walking | 190 | |
| `src/tree.rs` | `Tree` — the object the application owns | 200 | |
| `src/frame.rs` | the seven steps of §5.8, the round caps | 180 | |
| `src/reconcile.rs` | flattening, positional and keyed matching, mount, update, unmount | 260 | |
| `src/current.rs` | the thread-local, the guard that enters and leaves it | 100 | |
| `src/hook/mod.rs` | `Slot`, `Hooks`, `use_hook`, order checking | 130 | |
| `src/hook/state.rs` | state snapshots, `SetState<T>`, the per-slot writer | 230 | |
| `src/hook/reference.rs` | `Ref<T>` and the guard `current` hands back | 120 | |
| `src/hook/memo.rs` | `use_memo` | 80 | |
| `src/hook/effect.rs` | `use_effect`, `use_layout_effect`, `Cleanup`, `Always`, the effect queues | 200 | |
| `src/hook/context.rs` | the `Context` trait, `use_context`, `offer`, versions | 110 | |
| `src/hook/worker.rs` | `Promise`, `Resolver`, `Observable`, `Observer`, `promise`, `observable`, the generation checks | 180 | |
| `src/hook/store.rs` | `ExternalStore`, `Snapshot`, `Notify`, `Subscription`, `use_sync_external_store` | 90 | |
| `src/layout/mod.rs` | `Layout`, `Basis`, `Edges` | 110 | |
| `src/layout/flex.rs` | measure, the §5.4 resolve, the cross axis, `too_small` | 250 | `crates/ui/src/render/layout.rs`, 437 with tests |
| `src/paint/mod.rs` | the walk, clipping, `Paint`, the debug clip guard | 160 | `draw/screen.rs` 97 + `tab.rs` 73 + `pane.rs` 66 |
| `src/paint/host.rs` | `Row`, `Column`, `Stack`, `Gap`, `Divider`, `Canvas` and their props | 220 | |
| `src/paint/text.rs` | `Text`, and the one `measure` in the crate | 110 | |
| `src/event/mod.rs` | `Bubble`, `Mouse`, `Focus`, `Listeners`, focus traversal | 150 | |
| `src/event/hit.rs` | where each node landed, hit-testing, pointer capture | 140 | `draw/screen_map.rs`, 176 |
| `src/event/route.rs` | key, mouse and wheel routing; focus order | 180 | `app/mouse.rs` 125 + `app/keys.rs` 61 |
| `src/testing.rs` | `Harness`, `Probe` | 200 | `crates/ui/src/testing.rs`, 118 |
| | **`loom`** | **≈ 3,910** | |

### 14.2 `crates/loom-macros`

| file | responsibility | est. |
|---|---|---:|
| `src/lib.rs` | the three entry points, `rsx!`, `#[component]` and `context!` | 70 |
| `src/component.rs` | the props struct, the two impls, the hook-position check | 180 |
| `src/context.rs` | the marker, its props, the three impls | 90 |
| `src/rsx/mod.rs` | the two halves, and the error type they share | 40 |
| `src/rsx/parse.rs` | the grammar of §11.1 | 260 |
| `src/rsx/expand.rs` | the table of §11.2 | 220 |
| | **`loom-macros`** | **≈ 860** |

Every planned file is under the 300-line soft cap `cargo xtask lint-size`
enforces. `reconcile.rs` and `rsx/parse.rs` are the two to watch; both split by
noun if they grow — `reconcile.rs` into `keyed.rs` and `mount.rs`,
`rsx/parse.rs` into `element.rs` and `control.rs`.

### 14.3 Dependencies

```toml
# crates/loom/Cargo.toml
[dependencies]
loom-macros = { path = "../loom-macros" }
ratatui = { workspace = true }
crossterm = { workspace = true }
crokey = { workspace = true }

[lints]
workspace = true
```

No taffy, no `generational-box`, no async runtime, no new workspace
dependency. `unsafe_code = "forbid"` holds. `SetState<T>` and `Ref<T>` are checked slot
handles.
### 14.4 The `lint_arch` entries

Added to `xtask/src/lint_arch/rules.rs`. They make "the framework must not learn
the application" a build failure rather than a convention.

```rust
// FORBIDDEN_EDGES
(
    "loom",
    "ui",
    "a framework that names its one application is not a framework",
),
(
    "loom",
    "pipeline",
    "loom paints a cell grid; what is in it is the application's business",
),
(
    "loom",
    "align",
    "the same, for the diff model — a layout pass has no opinion about a view line",
),
(
    "loom",
    "syntax",
    "the same, for colour — a `Style` arrives as a prop, it is not asked for",
),
```

```rust
// PURE_CRATES — loom writes into a cell grid and opens nothing
"loom",
```

```rust
// NON_BLOCKING_DIRS — reached on every key and every frame
"crates/loom/src",
```

`lint-arch` reports an edge rule whose crate does not exist yet as *pending*,
so all four can be added in phase 0 before a line of `loom` is written and
cannot quietly stay dead.

`loom-macros` needs no entry: a proc-macro crate depending on `ui` would be a
cycle, which cargo already refuses.

---

## 15. Migration

Ten phases. Every one compiles, ships, and ends with `crates/ui/tests/explorer/*`
and `crates/codediff/tests/screens.rs` green — those assert the screen as text
through `Session::draw_into`, which never changes signature, so they are the net
under every step (I15). Each phase is independently revertable.

### Phase 0 — the crates exist, nothing uses them

Add `crates/loom` and `crates/loom-macros` to the workspace, and the four
`lint_arch` entries from §14.4. `loom` ships its own tests: every rule in §5
through §11, plus the panics in §12 asserted with `#[should_panic(expected = …)]`.
Nothing in `ui` changes.

**Green when:** `cargo test -p loom -p loom-macros`, `cargo xtask lint-arch`,
`cargo xtask lint-size`.

### Phase 1 — the status bar

`ui` gains a `loom::Tree` used for one rectangle. `Session::draw_into` calls
`draw::render` as it does today, then hands the status row to
`tree.draw(cells, status_area)`. `StatusBar` becomes a component (Appendix A.1).
`draw/status.rs`'s `Status` struct becomes its props; `name()` becomes a canvas.

This proves the whole pipeline — macro, reconcile, layout, paint — on the
smallest surface in the program. If it is wrong, one row is wrong.

**Green when:** `draw/status.rs`'s own nine tests pass unchanged, plus the
screen suites.

### Phase 2 — the explorer pane

`ExplorerPane` as Appendix A.2, with `draw::buffer::explorer::draw` inside a
canvas, unchanged. `draw/pane.rs` calls the tree for that one pane.
`crates/ui/tests/explorer/*` is 1,505 lines of assertions about this pane; they
stay as they are and stay green.

**Green when:** the explorer suite passes with no edit.

### Phase 3 — the diff panes

`DiffPane` as Appendix A.3. `SideBySide`, `Inline` and `SingleFile` each become
a canvas around today's function body. Nothing in `render/` is touched.
`render::layout::columns` keeps its property tests, including *a wider screen
never shows less than a narrower one*, which no flexbox solver reproduces for
you.

### Phase 4 — the root

`Screen` as Appendix A.4. Delete `draw/screen.rs`, `draw/tab.rs`,
`draw/pane.rs` and `draw/mod.rs`'s `Look`. `view::Layout` and `Tab::resize`
become `use_state` in `Screen`. `draw/screen.rs::too_small` becomes the layout
rule of §5.4. **At the end of this phase `Session::draw_into` is one line.**

Measured, on the other side of the ledger — `wc -l`, including tests:

```
draw/screen.rs  97   draw/tab.rs 73   draw/pane.rs 66   draw/mod.rs 40
```

### Phase 5 — the mouse

Listeners on the components that own the rectangles. Delete
`draw/screen_map.rs` (176 lines) and `app/mouse.rs` (125). `PendingSelection`
becomes `use_state` in the diff pane, and pointer capture (R8.4) replaces the
"is this drag still mine" reasoning.

### Phase 6 — the keyboard

Each pane resolves keys through the `Ref<Resolver>` it reads from context and
handles the actions it owns; everything else goes to the `Run` callback the
root put in context. Bubbling (R8.3) replaces `keymap::live()`'s hand-written
innermost-first walk.
`input::keymap`'s `const` tables and `input::Resolver` are untouched, so
`lint-arch`'s clock rule over `crates/ui/src/input` still holds. `app/keys.rs`
(61 lines) shrinks to the program-level bindings on the root: quit, suspend,
rebuild.

### Phase 7 — state moves in

`View`, `Tab` and `Pane` dissolve into state snapshots for small declarative
values and refs for `Buffer`, `Viewport`, `Resolver` and `Store`. The worker
handles stay `Rc`s the session also holds, because the session is what answers
into them. Those model types stay exactly as they are — the framework has no
opinion about them. `BufferId` and `PaneId` disappear, because the indirection
they exist to provide — a pane cannot hold `&mut Buffer` without making `View`
self-referential — is what `ScopeId` plus a checked `Ref<T>` provides now.

Measured: `view/mod.rs` 284, `view/tab.rs` 245, `view/pane.rs` 23.

### Phase 8 — workers

`Diffs`, `Spans` and `Files` as §9.2. `Session::send_file_request` and
`send_colour_request` disappear: a component asks the handle it read from
context, and the effect's deps decide when. `Session::files` and
`update_explorer`'s push from outside go the same way: the listing becomes a
store, and `Screen` reads it with `use_sync_external_store` (§9.5).
`ui::view::buffer::colour` gains a function that returns the `SyntaxRequest`s
instead of sending them, so the pane can build them in its effect (R9.4.5);
everything else in that file is unchanged.

**Green when:** `crates/codediff/tests/syntax.rs` and `pipeline.rs` pass, and
the new tests R9.4.1 through R9.4.6, R9.5.1 through R9.5.3.

### Phase 9 — the leftovers

Delete `draw/mod.rs`'s `TextRects`, `Session::selected`,
`Session::pending_selection` and `ScreenMap` from `ui`'s public API. Move
`draw/buffer/*` under whatever name it deserves once nothing above it is called
`draw` — as a move, in its own change, with nothing renamed in the same diff.

### 15.1 What `Session` becomes

```rust
/// One review session — the terminal, the workers, and the tree.
pub struct Session {
    tree: loom::Tree,
    workers: Workers,
    /// Quit, suspend or rebuild, if a component asked for one this frame.
    program: Rc<RefCell<Option<ProgramAction>>>,
}

/// The threads, and the three handles a component reaches them by.
pub struct Workers {
    diffs: Rc<worker::Diffs>,
    spans: Rc<worker::Spans>,
    files: Rc<worker::Files>,
    _watcher: Option<watcher::Watcher>,
}

impl Session {
    pub fn new(theme: Theme, root: PathBuf, tx: Sender<Event>) -> Self {
        let workers = Workers::spawn(&root, tx);
        let program = Rc::new(RefCell::new(None));

        // Both sides hold the same handle: the tree calls `open` on it, the
        // loop calls `resolve` on it. Cloning the `Rc` is the whole wiring.
        let tree = loom::Tree::new::<Screen>(ScreenProps {
            theme,
            files: workers.files.clone(),
            diffs: workers.diffs.clone(),
            spans: workers.spans.clone(),
            program: {
                let slot = program.clone();
                Rc::new(move |action| *slot.borrow_mut() = Some(action))
            },
        });

        Self { tree, workers, program }
    }

    pub fn draw_into(&mut self, cells: &mut Cells, area: Rect) {
        self.tree.draw(cells, area);
        self.drain();
    }

    pub fn draw<B: Backend>(&mut self, terminal: &mut Terminal<B>) -> Result<(), B::Error> {
        terminal.draw(|frame| {
            let area = frame.area();
            self.tree.draw(frame.buffer_mut(), area);
        })?;
        self.drain();
        Ok(())
    }

    /// Reports what the loop should do next. The requests have already gone
    /// out: a component sends its own through the handle it read from context,
    /// while `Tree::draw` is running its effects.
    pub fn drain(&mut self) -> Flow { /* the program action, if a pane set one */ }
}
```

Every field is something the tree cannot own: two threads, a terminal, and a
slot for the one answer a component gives back. The worktree listing is a
store, subscribed to by the component that shows it (§9.5), so `Session` never
learns what a file is. React's root is the same shape — `render` and
`unmount`, and nothing to read.

`draw_into` keeps the signature `crates/ui/tests/explorer/*` calls (I15), and
calling `drain` inside it keeps the existing test flow — draw, then the worker
has been asked — working with no edit to `TestSession`.

Its responsibilities are exactly: own the terminal, own the worker threads,
normalise crossterm events, hand answers to the handles that are waiting for
them, call `Tree::draw`, and answer quit, suspend and rebuild. That is what a
`Session` should have been.

### 15.2 What the loop becomes

```rust
loop {
    let Ok(event) = rx.recv() else { return Ok(Exit::Quit) };
    match event {
        Event::Terminal(CrosstermEvent::Key(_)) => {
            if let Some(key) = input::press(&event) { session.tree.press(key); }
        }
        Event::Terminal(CrosstermEvent::Mouse(mouse)) => { session.tree.mouse(mouse); }
        Event::Terminal(CrosstermEvent::Resize(..)) => session.tree.redraw_all(),
        Event::FileReady(response) => session.workers.diffs.resolve(response),
        Event::Coloured(response) => session.workers.spans.next(response),
        Event::ListRefreshed(files) => session.workers.files.listed(files),
        Event::FsChanged(_) => session.workers.files.changed(),
        #[cfg(unix)]
        Event::Signal(sig) => { terminal::restore(); std::process::exit(128 + sig) }
        _ => {}
    }
    match session.drain() {
        Flow::Quit => return Ok(Exit::Quit),
        Flow::Rebuild => return Ok(Exit::Rebuild),
        Flow::Suspend => screen.suspend()?,
        Flow::Continue => {}
    }
    if session.tree.needs_draw() {
        screen.draw(|t| session.draw(t))?;
    }
}
```

No async runtime, no executor, no polling thread. `mpsc` and threads, exactly as
now. `loom` contains no `spawn`, so `THREAD_FILES` gains no entry.

---

# Appendix A — four components, from real code

Real types throughout: `Theme`, `Viewport`, `Explorer`, `Buffer`, `File`,
`Resolver`, `Store`, `Alignment`.

Three small application types, all in `ui`:

```rust
// crates/ui/src/app/context.rs
use std::rc::Rc;
use loom::{Ref, SetState, context};

use crate::app::worker::Spans;
use crate::app::status::Status;
use crate::input::{Command, Resolver};
use crate::theme::Theme;
use file_types::File;

context! {
    /// The palette every painter reads.
    pub ThemeContext: Theme = Theme::dark();
    /// The parsed-file cache the diff reads.
    pub SyntaxContext: Ref<syntax::Store> = Ref::dangling();
    /// The keymap whichever pane holds focus asks.
    pub InputContext: Ref<Resolver> = Ref::dangling();
    /// Where a focused pane writes what the status line should say.
    pub StatusContext: SetState<Status> = SetState::nowhere();

    /// The syntax worker, for whichever pane wants its lines coloured.
    pub SpansContext: Rc<Spans> = Rc::new(Spans::closed()), same = Rc::ptr_eq;
    /// Open this file. Called by the explorer when the reader lands on a row.
    pub OpenContext: Rc<dyn Fn(File)> = Rc::new(|_| {}), same = Rc::ptr_eq;
    /// Carry out a command this pane does not own. Called by whichever pane
    /// resolved the key.
    pub RunContext: Rc<dyn Fn(Command)> = Rc::new(|_| {}), same = Rc::ptr_eq;
}
```

The `Context` suffix is React's own naming, and it leaves the value types their
plain names: the context is `ThemeContext`, the palette it carries is `Theme`.

The first four say what `createContext` says and no more. The last three carry
something with no `==` of its own, so each names `Rc::ptr_eq`, which is what
`Object.is` does for a JavaScript function.

`SyntaxContext` and `InputContext` are both `Ref<_>`, and `OpenContext` and
`RunContext` are both `Rc<dyn Fn(_)>`; each pair stays apart because the
identity is the marker type, not the value's (R10.1.4). None of them needs a
newtype.

Every default does nothing rather than failing: an unprovided `OpenContext` is
a closure that ignores the file, and an unprovided `SpansContext` is a `Spans`
with no thread behind it, whose observable is never answered. A component
reading a context is therefore never a place a panic can come from.

```rust
// crates/ui/src/app/status.rs
/// What the status line says.
///
/// `draw::status::Status`, owned rather than borrowed, because props are
/// `'static`. Whichever pane holds focus writes this during its own render;
/// the root reads the next snapshot and hands it to `StatusBar`. A write
/// compares values, so the second, identical write runs nothing and the frame
/// settles (R6.3.5).
#[derive(Clone, PartialEq, Default)]
pub struct Status {
    pub file: Option<std::rc::Rc<file_types::File>>,
    pub view_line: u32,
    pub view_lines: u32,
    pub changes: usize,
    pub change: Option<usize>,
    pub timed_out: bool,
    pub exhausted: Option<crate::view::Direction>,
}
```

`Diffs` and `Spans` are §9.2.

## A.1 `StatusBar`

Replaces `draw/status.rs`'s hand-placed offsets. `summary()` and `name()` move
across unchanged; `name()` becomes the canvas, because it drops the directory
before the file name and that is text fitting, not layout (§7.3).

```rust
use std::rc::Rc;

use file_types::File;
use loom::{Basis, Canvas, CanvasProps, Layout, Node, Row, RowProps, Scope, Text, TextProps,
           component, rsx, use_context, use_ref};
use ratatui::buffer::Buffer as Cells;

use crate::app::context::ThemeContext;
use crate::hook::use_size;
use crate::render::cells;
use crate::theme::Theme;
use crate::view::Direction;

/// The narrowest a file name is worth showing beside a position.
///
/// Chosen, not measured — the same order as `render::layout::MIN_LIST`.
const ROOM_FOR_A_NAME: u16 = 8;

#[component]
pub fn StatusBar(
    scope: &mut Scope,
    file: Option<Rc<File>>,
    view_line: u32,
    view_lines: u32,
    changes: usize,
    change: Option<usize>,
    timed_out: bool,
    exhausted: Option<Direction>,
    notice: Option<Rc<str>>,
) -> Node {
    let theme = use_context::<ThemeContext>(scope);
    let bar = use_ref(scope, || None);
    let width = use_size(scope, bar).width;

    // Today's `summary`, taking the fields it reads rather than a borrowed
    // `Status`, because props are `'static`. Its body is unchanged.
    let right: Rc<str> = summary(*view_line, *view_lines, *changes, *change,
                                 file.is_some(), *exhausted).into();
    let needed = right.chars().count() as u16 + ROOM_FOR_A_NAME + 3;
    let show_right = needed <= width;

    let base = theme.status;
    let row = Layout {
        basis: Basis::Length(1),
        shrink: 0,
        pad: loom::Edges::sides(1),
        gap: 2,
        fill: Some(base),
        ..Default::default()
    };

    let shown = file.clone();
    let dim = theme.divider;
    let path = theme.status_path;

    rsx! {
        Row { layout: row, ref: bar, ..,
            if let Some(why) = notice.clone() {
                Text { layout: Layout { grow: 1, ..Default::default() },
                       text: why, style: base.patch(theme.warning) }
            } else if let Some(file) = shown {
                // `name` from draw/status.rs, moved and not otherwise touched:
                // it drops the rename source, then the directory, and never
                // the file name.
                Canvas {
                    layout: Layout { grow: 1, ..Default::default() },
                    paint: Rc::new(move |paint: &mut loom::Paint<'_>| {
                        let area = paint.area();
                        name(paint.cells(), area, &file, area.width, base, path, dim);
                    }),
                    ..
                }
            } else {
                Text { layout: Layout { grow: 1, ..Default::default() },
                       text: "changed files".into(), style: base.patch(path) }
            }

            if *timed_out {
                // Loud. A diff the engine abandoned is not a diff, and a
                // reviewer who mistakes one for a complete one will approve
                // code they have not seen.
                Text { text: "PARTIAL — diff timed out".into(),
                       style: base.patch(theme.warning), .. }
            }

            if show_right {
                Text { text: right, style: base, .. }
            }
        }
    }
}
```

What went away: `area.width.saturating_sub(right.chars().count() as u16 + 3)`,
the running `x` threaded through four writes, and `if at > x + 1`. What stayed:
every one of `draw/status.rs`'s nine tests, and the priority dropping inside
`name`.

## A.2 `ExplorerPane`

`draw::buffer::explorer::draw` is unchanged, inside a canvas.

```rust
use std::rc::Rc;

use crokey::KeyCombination;
use crossterm::event::MouseButton;
use file_types::File;
use loom::{Bubble, Canvas, CanvasProps, Layout, Listeners, Mouse, Node, Ref, Scope,
           component, rsx, use_context, use_effect, use_ref, use_state};

use crate::app::context::{InputContext, OpenContext, RunContext, StatusContext,
                          ThemeContext};
use crate::hook::use_size;
use crate::input::{Action, BufferAction, KeymapType, Resolution, Resolver, ViewAction};
use crate::theme::Theme;
use crate::view::{Buffer, BufferType, Viewport};
use crate::app::status::Status;

#[component]
pub fn ExplorerPane(scope: &mut Scope, files: Snapshot<[File]>) -> Node {
    let theme = use_context::<ThemeContext>(scope);
    let keys = use_context::<InputContext>(scope);
    let status = use_context::<StatusContext>(scope);
    let open = use_context::<OpenContext>(scope);
    let run = use_context::<RunContext>(scope);

    let buffer = use_ref(scope, || Buffer::explorer(files.to_vec()));
    let view = use_ref(scope, Viewport::new);
    let pane = use_ref(scope, || None);
    let area = use_size(scope, pane);
    let (focused, set_focused) = use_state(scope, || false);

    // Both models above are refs, so writing one is silent. Counting the
    // writes is what asks for the next frame — React's function components
    // force an update the same way, and §2 already calls this a redraw.
    let (_, redraw) = use_state(scope, || 0u32);

    // A new list from the watcher. Rebuild the arrangement and keep the reader
    // on the file they were on — `reshape_around` already does that.
    // This is `View::update_explorer`, moved to the thing that owns the list.
    let listed = files.clone();
    use_effect(scope, files.clone(), move || {
        let files = listed.to_vec();
        let cursor = view.current().cursor();
        let mut buffer = buffer.current();
        let BufferType::Explorer(explorer) = buffer.buffer_type_mut() else { return };
        let landing = explorer.reshape_around(cursor, |e| e.refresh(files));
        buffer.update_line_count();
        view.current().place(landing, buffer.view_lines());
        redraw(&|n| n + 1);
    });

    // The height the frame is about to be painted at, recorded silently:
    // the frame that will read it is the one being prepared.
    let rows = buffer.current().view_lines();
    view.current().set_height(u32::from(area.height), rows);

    // What the status line says while this pane has focus. A list of changed
    // files is not a diff, so it has no changes to count.
    if focused {
        status(&|_| Status {
            file: None,
            view_line: view.current().cursor(),
            view_lines: rows,
            ..Status::default()
        });
    }

    let chose = move || {
        let cursor = view.current().cursor();
        match buffer.current().buffer_type() {
            BufferType::Explorer(explorer) => explorer.file(cursor).cloned(),
            _ => None,
        }
    };

    let open_key = open.clone();
    let listeners = Listeners::new()
        .on_key(move |key: KeyCombination| {
            let Resolution::Run(command) = keys.current().key(key, KeymapType::Explorer)
            else {
                return Bubble::Stop;   // a count, a prefix, or nothing bound
            };
            match command.action {
                Action::Buffer(action) => {
                    let moved = matches!(action, BufferAction::Motion(_));
                    buffer.current().apply(action, command.repeat(), &mut view.current());
                    redraw(&|n| n + 1);
                    if moved && let Some(file) = chose() {
                        open_key(file);
                    }
                    Bubble::Stop
                }
                Action::View(ViewAction::Open) => {
                    let cursor = view.current().cursor();
                    let folded = buffer.current().activate(cursor);
                    if folded {
                        let rows = buffer.current().view_lines();
                        view.current().place(cursor.min(rows.saturating_sub(1)), rows);
                    } else if let Some(file) = chose() {
                        open_key(file);
                    }
                    redraw(&|n| n + 1);
                    Bubble::Stop
                }
                // Tab, view and program actions belong further out. The root
                // gave us the function that carries them out.
                _ => { run(command); Bubble::Stop }
            }
        })
        .on_wheel(move |lines| {
            let rows = buffer.current().view_lines();
            view.current().scroll(lines, rows);
            redraw(&|n| n + 1);
            Bubble::Stop
        })
        .on_focus(move |_| { set_focused(&|_| true); Bubble::Continue })
        .on_blur(move |_| { set_focused(&|_| false); Bubble::Continue })
        .on_mouse_down(move |mouse: Mouse| {
            if mouse.button != Some(MouseButton::Left) {
                return Bubble::Continue;
            }
            if let Some(node) = *pane.current() {
                node.focus();
            }
            let rows = buffer.current().view_lines();
            let line = view.current().top() + u32::from(mouse.local.y);
            if line < rows {
                view.current().place(line, rows);
                redraw(&|n| n + 1);
                if let Some(file) = chose() {
                    open(file);
                }
            }
            Bubble::Stop
        });

    let has_focus = focused;
    let paint = Rc::new(move |paint: &mut loom::Paint<'_>| {
        let area = paint.area();
        let buffer = buffer.current();
        let BufferType::Explorer(explorer) = buffer.buffer_type() else { return };
        crate::draw::buffer::explorer::draw(
            paint.cells(), area, explorer, &view.current(), &theme, has_focus,
        );
    });

    rsx! {
        Canvas {
            layout: Layout {
                grow: 1, min_width: 8, clip: true, ..Default::default()
            },
            ref: pane,
            focusable: true,
            listeners,
            paint,
        }
    }
}
```

What went away: `screen_map.text_areas`, `self.view.tab_mut().pane_mut(id).viewport`,
`View::update_explorer`, and the mouse handler's four-level reach through
`View → Tab → Pane → Buffer`. What stayed: `Buffer`, `Viewport`, `Explorer`,
`Resolver`, the keymap tables, and every line of `draw::buffer::explorer`.

## A.3 `DiffPane`

The whole point of the `Canvas` escape hatch. `render/column.rs`, `line.rs`,
`gutter.rs`, `cells.rs` and `selection.rs` are the best-tested code in the
repository and none of it is touched.

```rust
use std::rc::Rc;

use loom::{Bubble, Canvas, CanvasProps, Layout, Listeners, Node, Ref, Scope,
           component, rsx, use_context, use_effect,
           use_ref, use_state};
use syntax::{Store, SyntaxResponse, Version};

use crate::app::context::{InputContext, RunContext, SpansContext, StatusContext,
                          SyntaxContext, ThemeContext};
use crate::app::status::Status;
use crate::draw::Look;
use crate::hook::use_size;
use crate::input::{Action, KeymapType, Resolution, Resolver};
use crate::theme::Theme;
use crate::view::{Buffer, Viewport};

/// Read ahead of the screen, so scrolling finds colour already there.
const MARGIN: u32 = 2_000;

#[component]
pub fn DiffPane(scope: &mut Scope, buffer: Ref<Option<Buffer>>, version: Version) -> Node {
    let theme = use_context::<ThemeContext>(scope);
    let store = use_context::<SyntaxContext>(scope);
    let keys = use_context::<InputContext>(scope);
    let spans = use_context::<SpansContext>(scope);
    let status = use_context::<StatusContext>(scope);
    let run = use_context::<RunContext>(scope);

    let view = use_ref(scope, Viewport::new);
    let pane = use_ref(scope, || None);
    let area = use_size(scope, pane);
    let (focused, set_focused) = use_state(scope, || false);

    // `view` and the shared `store` are both refs, so writing either is
    // silent. This counts the writes worth a frame.
    let (_, redraw) = use_state(scope, || 0u32);

    let rows = buffer.current().as_ref().map_or(0, Buffer::view_lines);
    view.current().set_height(u32::from(area.height), rows);
    let visible = view.current().visible(rows);
    let keymap = buffer.current().as_ref()
        .map_or(KeymapType::default(), Buffer::keymap_type);

    if focused {
        // `draw::screen::summary`, moved to the pane that knows the answers.
        // One read of the cursor: a second while the first is still in hand
        // is P4.5.
        let cursor = view.current().cursor();
        status(&|_| buffer.current().as_ref().map_or_else(Status::default, |b| Status {
            file: b.file().cloned().map(Rc::new),
            view_line: cursor,
            view_lines: b.view_lines(),
            changes: b.blocks().len(),
            change: b.block_at(cursor),
            timed_out: b.hit_timeout(),
            exhausted: b.exhausted(),
        }));
    }

    // Colour. The deps say what has arrived as well as what is needed, so
    // installing a piece asks for the next one and nothing else does. R9.4.5.
    let coloured = buffer.current().as_ref()
        .map_or((0, 0), |b| b.coloured_lines(&store.current()));
    let end = visible.end;
    let version = *version;
    use_effect(scope, (version, end, coloured), move || {
        let requests = buffer.current().as_ref()
            .map(|b| b.colour_requests(&mut store.current(), version, end + MARGIN))
            .unwrap_or_default();
        if requests.is_empty() {
            return;
        }
        spans.colour(requests).subscribe(move |response: SyntaxResponse| {
            let from = response.from;
            let taken = store.current().install(response);
            // Only a piece covering a line this pane is showing is worth a
            // frame. `end` is a view line, which is never below the file line
            // at the same place, so this never skips a redraw that was needed.
            if taken && from < end {
                redraw(&|n| n + 1);
            }
        });
    });

    let listeners = Listeners::new()
        .on_key(move |key| {
            let Resolution::Run(command) = keys.current().key(key, keymap) else {
                return Bubble::Stop;
            };
            match command.action {
                Action::Buffer(action) => {
                    if let Some(b) = buffer.current().as_mut() {
                        b.apply(action, command.repeat(), &mut view.current());
                    }
                    redraw(&|n| n + 1);
                    Bubble::Stop
                }
                _ => { run(command); Bubble::Stop }
            }
        })
        .on_wheel(move |lines| {
            let rows = buffer.current().as_ref().map_or(0, Buffer::view_lines);
            view.current().scroll(lines, rows);
            redraw(&|n| n + 1);
            Bubble::Stop
        })
        .on_focus(move |_| { set_focused(&|_| true); Bubble::Continue })
        .on_blur(move |_| { set_focused(&|_| false); Bubble::Continue })
        .on_mouse_down(move |_| {
            if let Some(node) = *pane.current() {
                node.focus();
            }
            // A drag that leaves the column still belongs to it. R8.4.1.
            loom::capture_pointer();
            Bubble::Stop
        });

    let has_focus = focused;
    let paint = Rc::new(move |paint: &mut loom::Paint<'_>| {
        let area = paint.area();
        let buffer = buffer.current();
        let Some(buffer) = buffer.as_ref() else { return };
        let store = store.current();
        // Byte for byte, today's `draw::buffer::draw` call.
        let look = Look { theme: &theme, syntax: true, store: &store };
        crate::draw::buffer::draw(
            paint.cells(), area, buffer, &view.current(), look, has_focus,
        );
    });

    rsx! {
        Canvas {
            layout: Layout {
                grow: 1, min_width: 20, clip: true, ..Default::default()
            },
            ref: pane,
            focusable: true,
            listeners,
            paint,
        }
    }
}
```

Note which storage lives where, and why it matters. The `Viewport` is the
pane's ref, keyed by file name in the root (below), so re-opening a file the
reader was on keeps their cursor and opening a different one starts at its top
— which is `View::show`'s `keep` and `Tab::set_right_pane`'s fresh pane, both
for free. The `Buffer` is a root ref because that is where the worker's reply
lands. Scrolling edits the pane's ref and redraws that pane; opening a file
updates root state once.

## A.4 `Screen`, the root

Replaces `draw/screen.rs`, `draw/tab.rs`, `draw/pane.rs`, `view/tab.rs`,
`view/pane.rs` and most of `app/mod.rs`.

```rust
use std::rc::Rc;

use file_types::File;
use loom::{Column, ColumnProps, Divider, DividerProps, Layout,
           Basis, Node, Row, RowProps, Scope, Text, TextProps,
           component, focus_next, focus_previous, rsx,
           use_effect, use_memo, use_ref, use_state};
use syntax::{Store, Version};

use crate::app::context::{
    InputContext, InputContextProps, OpenContext, OpenContextProps,
    RunContext, RunContextProps, SpansContext, SpansContextProps,
    StatusContext, StatusContextProps, SyntaxContext, SyntaxContextProps,
    ThemeContext, ThemeContextProps,
};
use crate::app::worker::{Diffs, Spans};
use crate::app::status::Status;
use crate::input::{Action, Command, ProgramAction, Resolver, TabAction, ViewAction};
use crate::theme::Theme;
use crate::view::Buffer;

/// Columns the list gets when the screen first splits. `view/tab.rs`'s
/// `DEFAULT_LEFT`, moved to the thing that owns the split.
const DEFAULT_LEFT: u16 = 40;
/// The narrowest and widest that border may go. `view/tab.rs`'s `MIN_LEFT`
/// and `MAX_LEFT`.
const MIN_LEFT: u16 = 12;
const MAX_LEFT: u16 = 100;

#[component]
pub fn Screen(
    scope: &mut Scope,
    files: Rc<Files>,
    theme: Theme,
    diffs: Rc<Diffs>,
    spans: Rc<Spans>,
    program: Rc<dyn Fn(ProgramAction)>,
) -> Node {
    // The worktree listing changes because the disk changed, so it is read
    // from the store rather than pushed in as a prop. R9.5.1.
    let listing = use_sync_external_store(scope, &*files);

    let store = use_ref(scope, Store::new);
    let keys = use_ref(scope, Resolver::new);
    let opened = use_ref(scope, || None::<Buffer>);

    let (status, set_status) = use_state(scope, Status::default);
    let (notice, set_notice) = use_state(scope, || None::<Rc<str>>);
    let (chosen, set_chosen) = use_state(scope, || None::<File>);
    let (version, set_version) = use_state(scope, || Version(1));
    let (left, set_left) = use_state(scope, || DEFAULT_LEFT);

    // The two things a pane asks the root to do. Setters and refs are `Copy`
    // slot handles, so rebuilding these callbacks costs one allocation apiece.
    // `use_memo` keeps the same `Rc` across renders, so `Rc::ptr_eq` in the
    // context's `same` finds it unchanged and no consumer is disturbed
    // (R10.1.6).
    let ask_program = program;
    let open: Rc<dyn Fn(File)> = use_memo(scope, (), move |_| {
        move |file: File| set_chosen(&|_| Some(file.clone()))
    });
    let run: Rc<dyn Fn(Command)> = use_memo(scope, (), move |_| {
        move |command: Command| match command.action {
            Action::Tab(TabAction::FocusNext) => {
                // Focus order is paint order, so "the next pane" needs no table
                // of its own. R8.2.2.
                focus_next();
            }
            Action::Tab(TabAction::FocusPrev) => focus_previous(),
            Action::Tab(TabAction::WidenLeft) => {
                set_left(&|n| (n + 2).min(MAX_LEFT));
            }
            Action::Tab(TabAction::NarrowLeft) => {
                set_left(&|n| n.saturating_sub(2).max(MIN_LEFT));
            }
            Action::View(ViewAction::ToggleLayout) => {
                let mut opened = opened.current();
                if let Some(buffer) = opened.take() {
                    *opened = Some(buffer.switch_diff_layout());
                }
                set_version(&|v| Version(v.0 + 1));
            }
            Action::Program(action) => ask_program(action),
            _ => {}
        }
    });

    // Ask for the file the reader chose. The promise belongs to this effect,
    // so an answer for a file they have since left is refused rather than
    // shown — R9.3.3, which is what `apply_file_response`'s `if
    // self.selected.as_ref() != Some(&response.file)` was for. The listing is
    // in the deps so a change on disk reads the file again (R9.5.1).
    use_effect(scope, (chosen.clone(), listing.clone()), move || {
        let Some(file) = chosen.clone() else { return };
        diffs.open(file).then(move |response: pipeline::file::Response| {
            match response.content {
                Ok(content) => {
                    *opened.current() = Some(Buffer::diff(content));
                    set_notice(&|_| None);
                    set_version(&|v| Version(v.0 + 1));
                }
                Err(why) => set_notice(&|_| Some(why.clone().into())),
            }
        });
    });

    // The key is the file, so re-opening the same file keeps the same scope
    // and therefore the same viewport, while a different file gets a fresh
    // one. R6.1.4.
    let shown = opened.current().as_ref()
        .and_then(Buffer::file)
        .map(|f| f.path().as_str().to_owned());
    let split = shown.is_some();

    // The list asks for the width the reader chose and gives it back when the
    // diff needs its minimum — R5.4.4 and R5.4.5, which is what
    // `render::layout::split`'s clamp did by hand.
    let explorer = Layout {
        basis: if split { Basis::Length(left) } else { Basis::Auto },
        grow: if split { 0 } else { 1 },
        shrink: 1,
        min_width: 8,
        ..Default::default()
    };
    rsx! {
        // Seven providers, nested. React looks the same, and each one's value
        // reaches only what is inside it (R10.1.5).
        ThemeContext { value: theme,
        SyntaxContext { value: store,
        InputContext { value: keys,
        StatusContext { value: set_status,
        SpansContext { value: spans,
        OpenContext { value: open,
        RunContext { value: run,

        Column {
            layout: Layout { grow: 1, ..Default::default() },
            too_small: Some(rsx! {
                Text { text: "terminal too small".into(), style: theme.normal, .. }
            }),
            ..,

            Row { layout: Layout { grow: 1, ..Default::default() }, ..,
                Column { layout: explorer, ..,
                    ExplorerPane { files: listing.clone() }
                }

                if let Some(name) = shown {
                    Divider {
                        layout: Layout { basis: Basis::Length(1), shrink: 0,
                                         ..Default::default() },
                        symbol: "│",
                        style: theme.normal.patch(theme.divider),
                    }
                    Column {
                        layout: Layout { grow: 1, basis: Basis::Length(0),
                                         min_width: 20, ..Default::default() },
                        ..,
                        DiffPane { key: name, buffer: opened, version }
                    }
                }
            }

            StatusBar {
                file: status.file, view_line: status.view_line,
                view_lines: status.view_lines, changes: status.changes,
                change: status.change, timed_out: status.timed_out,
                exhausted: status.exhausted, notice: notice.clone(),
            }
        }

        }}}}}}}
    }
}
```

`Screen` reads none of the seven itself: a component does not see the value it
offers (R10.1.5), and it does not need to — it already holds each one as state,
a prop or a ref. What it offers is what everything below it reads.

`render::layout::split`'s clamp — *a wider screen never shows less than a
narrower one* — becomes R5.4.4 with the same test name, and
`draw/tab.rs`'s "a pane that cannot draw falls back to one pane rather than
failing the whole screen" becomes the `too_small` climb of R5.4.2.

### A.5 Where each piece of `View` went

| today | tomorrow |
|---|---|
| `View::buffers` | `use_ref` in the component that shows each one |
| `View::tabs`, `Tab::panes`, `Tab::layout` | the node tree |
| `Tab::focus`, `focus_next` | `focusable` on a node, `focus_next`, paint order (R8.2.2) |
| `Tab::resize` | `left` plus `set_left: SetState<u16>` in `Screen` |
| `Pane::viewport` | `use_ref(scope, Viewport::new)` in each pane |
| `BufferId`, `PaneId` | `ScopeId` |
| `View::selection`, `PendingSelection` | `use_state` in `DiffPane`, plus pointer capture |
| `View::version` | a `Version` snapshot plus `SetState<Version>` in `Screen` |
| `View::request` | the syntax effect in `DiffPane` (R9.4.5) |
| `View::update_explorer` | the `files` effect in `ExplorerPane` |
| `View::selected_file` | a chosen-file snapshot plus setter in `Screen`, set through the `Open` callback |
| `View::keymap_type` | each pane's own `KeymapType`, at the point it resolves a key |
| `ScreenMap` | the paint walk's record (R7.1.5) |
| `Look` | `Theme` and `Ref<Store>` in context |
