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
| hooks | `use_state`, `use_memo`, `use_effect`, `use_size`, `use_focus`, context |
| layout | one axis per container, integer terminal cells, two passes |
| paint | a top-down walk that writes into `ratatui::buffer::Buffer` |
| events | hit-test by rectangle, focus by scope, bubbling, pointer capture |
| worker replies | `Completion<T>` and `Subscription<T>`, rejected when stale |
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
| `use_callback`, `useId`, `useReducer`, suspense, portals, hot reload, SSR | no user here |
| struct components, borrowed props, GATs | locked out by decisions 7 and 8 |
| text wrapping | no pane in this program wraps |

### 1.3 Settled disagreements

The two prior designs disagreed on these. The choice is made; the reason is one
sentence.

| question | choice | why |
|---|---|---|
| crate name | `loom`, macros in `loom-macros` | one plain English word, like `align` and `syntax`; both crates are `publish = false` so the name on crates.io is a coincidence |
| layout model | CSS flexbox, implemented here in whole cells | the model is proven and documented; the crate is not the model. `f32` rounded back to cells is where column drift comes from, and CSS cannot say "if this does not fit, do not draw it" — so we take the algorithm and replace overflow with refusal (§5.6) |
| state access in listeners | thread-local runtime, no `cx` parameter | `cursor.set(cursor.cloned() + 1)` is the line that makes this feel like React rather than like Rust arguing with you |
| worker replies | typed handles with a generation check | a `TaskId` token says where to deliver; a generation says whether it is still wanted |
| render parameter | `scope` | `cx` reads as "context", and context is a different thing here |
| state handle | `State<T> = { scope, slot }`, `Copy`, no arena | the workspace forbids `unsafe`, and the hook slot is already the storage |
| a second, quiet state type | none — `State::edit_without_redraw` instead | one idea, one word; a `Ref<T>` beside `std::cell::Ref` is two words for one idea |
| context storage | parent walk keyed by `TypeId`, read recorded as `(TypeId, version)` | there is no provider node to reconcile, and the recorded version is the twenty lines that keep `memo` honest |
| provider syntax | `provide_context(scope, value)` at the top of a render | it adds no node to the tree; a `Provider` component can be written on top of it in eight lines |
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
| **completion** | a one-shot reply address | task |
| **subscription** | a many-shot reply address | stream |
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
pub use event::{Bubble, Focus, Listeners, Mouse, capture_pointer, release_pointer};
pub use hook::{
    Completion, Effect, State, Subscription, context, provide_context, redraw, try_context,
    use_effect, use_focus, use_memo, use_size, use_state,
};
pub use layout::{Basis, Edges, Layout};
pub use node::{Children, Element, Key, Node};
pub use paint::{Canvas, CanvasProps, Column, ColumnProps, Divider, DividerProps, Gap, GapProps,
    Paint, Row, RowProps, Stack, StackProps, Text, TextProps};
pub use scope::{Scope, ScopeId};
pub use tree::Tree;

pub use loom_macros::{component, rsx};

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
    /// Painted instead of the children when they cannot meet their minimums.
    pub too_small: Option<Box<Node>>,
    pub children: Vec<Node>,
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
    /// Run this component again on the next frame.
    pub fn redraw(&mut self);
}
```

### 3.4 State

```rust
// hook/state.rs
use std::marker::PhantomData;

/// A place a component keeps something between frames.
///
/// Two integers naming a hook slot, so it is `Copy` and `'static` and can be
/// moved into as many listeners as you like.
pub struct State<T: 'static> {
    scope: ScopeId,
    slot: u16,
    value: PhantomData<fn() -> T>,
}

impl<T> Clone for State<T> { fn clone(&self) -> Self { *self } }
impl<T> Copy for State<T> {}
impl<T> PartialEq for State<T> { /* scope and slot */ }
impl<T> Eq for State<T> {}

impl<T: 'static> State<T> {
    /// Reads. The closure keeps the borrow short and typed.
    pub fn read<R>(self, look: impl FnOnce(&T) -> R) -> R;
    /// Reads a copy, for the small case.
    pub fn cloned(self) -> T where T: Clone;
    /// Writes, and redraws the owning component.
    pub fn set(self, value: T);
    /// Writes, and redraws the owning component only if the value changed.
    ///
    /// React's bail-out, made explicit because Rust can check it: a component
    /// that recomputes the same value every render must not mark anything.
    pub fn set_if_changed(self, value: T) where T: PartialEq;
    /// Writes in place, and redraws the owning component.
    pub fn edit<R>(self, change: impl FnOnce(&mut T) -> R) -> R;
    /// Writes in place without redrawing, for a value the frame is already
    /// about to read — a viewport height, a syntax cache.
    pub fn edit_without_redraw<R>(self, change: impl FnOnce(&mut T) -> R) -> R;
    /// Whether the owning component is still mounted.
    pub fn is_mounted(self) -> bool;
}
```

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

The built-in hosts, each with a props struct the macro fills in:

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
use crossterm::event::{MouseButton, MouseEventKind};
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
    pub kind: MouseEventKind,
    pub button: Option<MouseButton>,
    /// Where on the screen.
    pub at: Position,
    /// Where within this node's rectangle.
    pub local: Position,
}

/// Every listener one host can carry.
#[derive(Clone, Default)]
pub struct Listeners { /* private */ }

impl Listeners {
    pub fn new() -> Self;
    pub fn on_key(self, listen: impl Fn(KeyCombination) -> Bubble + 'static) -> Self;
    pub fn on_mouse(self, listen: impl Fn(Mouse) -> Bubble + 'static) -> Self;
    /// Positive is down. Separate from `mouse` because it is the one mouse
    /// event routed by position that is not a click.
    pub fn on_wheel(self, listen: impl Fn(i32) -> Bubble + 'static) -> Self;
    pub fn on_focus(self, listen: impl Fn(bool) + 'static) -> Self;
}

/// Registered by `use_focus`.
#[derive(Clone, Copy)]
pub struct Focus {
    scope: ScopeId,
    pub has: bool,
}

impl Focus {
    /// Ask for focus.
    pub fn request(self);
    /// Move focus to the next focusable in paint order, wrapping.
    pub fn move_next(self);
    pub fn move_previous(self);
}

/// Route every mouse event to this node until the button comes up or
/// `release_pointer` is called. Called from inside a mouse listener.
pub fn capture_pointer();
pub fn release_pointer();
```

A child tells an ancestor something by calling a function the ancestor gave it
— as a prop, or through context when it sits several levels down. That is
React's pattern and it needs nothing from the framework: `Rc<dyn Fn(T)>` is
already `Clone + 'static`, so it is a context value like any other. §10 shows
both ends.

### 3.8 Worker replies

```rust
// hook/worker.rs
/// A one-shot reply address.
///
/// Created inside an effect. Carries the owning scope, the effect's slot and
/// the effect's generation, so a reply that arrives after the deps changed or
/// the component went away is refused rather than applied.
pub struct Completion<T: 'static> { /* private; holds a Weak to the runtime */ }

impl<T: 'static> Completion<T> {
    /// Delivers. Returns whether it was taken.
    pub fn complete(self, value: T) -> bool;
    pub fn is_wanted(&self) -> bool;
}

/// A many-shot reply address, for a worker that answers in pieces.
pub struct Subscription<T: 'static> { /* private */ }

impl<T: 'static> Clone for Subscription<T> {}

impl<T: 'static> Subscription<T> {
    /// Delivers one piece. Returns whether it was taken.
    pub fn deliver(&self, value: T) -> bool;
    pub fn is_wanted(&self) -> bool;
    pub fn close(self);
}

/// What an effect body is handed.
pub struct Effect<'a> { /* private */ }

impl Effect<'_> {
    pub fn completion<T: 'static>(&mut self, take: impl FnOnce(T) + 'static) -> Completion<T>;
    pub fn subscription<T: 'static>(&mut self, take: impl FnMut(T) + 'static) -> Subscription<T>;
}
```

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
    /// How many render-and-layout rounds the last `draw` took. One, unless
    /// `use_size` changed something. Capped at 4 (R5.8.2).
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
    pub fn provide<T: Clone + 'static>(self, value: T) -> Self;
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
    /// How many layout rounds the last `draw` took. One, unless `use_size`
    /// changed something.
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
    /// The value, behind its own cell so two different states can be read at
    /// once without the runtime itself staying borrowed.
    State(std::rc::Rc<std::cell::RefCell<dyn std::any::Any>>),
    Memo(MemoSlot),
    Effect(EffectSlot),
    Context(ContextSlot),
    Size,
    Focus,
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
/// A value that survives a render.
///
/// Re-runs: never — `first` is called once, when the component mounts.
/// Panics: P4.1, P4.2.
#[track_caller]
pub fn use_state<T: 'static>(scope: &mut Scope, first: impl FnOnce() -> T) -> State<T>;
```

```rust
// hook/memo.rs
/// A value recomputed only when `deps` changes.
///
/// Re-runs: when `deps != previous deps`. Returns the same `Rc` otherwise.
/// Panics: P4.1, P4.2.
#[track_caller]
pub fn use_memo<D, T>(scope: &mut Scope, deps: D, compute: impl FnOnce(&D) -> T) -> std::rc::Rc<T>
where
    D: PartialEq + 'static,
    T: 'static;
```

```rust
// hook/effect.rs
/// Work to do after the frame is painted.
///
/// Re-runs: after the paint of the first frame in which `deps != previous
/// deps`. The value the closure returns is dropped when the deps change again
/// or the component goes away — that is the cleanup, and it composes: an
/// effect that starts something returns it, and dropping it stops it.
/// Panics: P4.1, P4.2.
#[track_caller]
pub fn use_effect<D, C>(
    scope: &mut Scope,
    deps: D,
    run: impl FnOnce(&D, &mut Effect<'_>) -> C + 'static,
) where
    D: PartialEq + 'static,
    C: 'static;
```

```rust
// hook/context.rs
/// Offer `value` to everything below this component.
///
/// Re-runs: every render. A later call in the same component replaces an
/// earlier one of the same type.
/// Panics: none.
pub fn provide_context<T: Clone + 'static>(scope: &mut Scope, value: T);

/// The nearest ancestor's `T`.
///
/// Re-runs: every render; the read is recorded so a memoised component cannot
/// go stale.
/// Panics: P10.1 when nothing above provided one.
#[track_caller]
pub fn context<T: Clone + 'static>(scope: &mut Scope) -> T;

/// The same, answering `None` instead of panicking.
pub fn try_context<T: Clone + 'static>(scope: &mut Scope) -> Option<T>;
```

```rust
// hook/screen.rs
/// The rectangle this component's node occupied.
///
/// Re-runs: `Rect::ZERO` before the first layout; afterwards, the rectangle
/// from the last layout of this frame. A change re-runs the component before
/// the frame is painted, so a pane can size itself in the frame it is resized
/// in rather than one frame later. See R5.6.
/// Panics: P4.1, P4.2.
#[track_caller]
pub fn use_size(scope: &mut Scope) -> ratatui::layout::Rect;

/// Registers this component as focusable and says whether it holds focus.
///
/// Re-runs: every render. The registration lasts until the component unmounts.
/// Panics: P4.1, P4.2.
#[track_caller]
pub fn use_focus(scope: &mut Scope) -> Focus;
```

```rust
// hook/mod.rs
/// Run the component that is speaking again on the next frame.
///
/// Legal inside a listener, an effect and a delivered worker reply, where the
/// runtime knows whose turn it is.
/// Panics: P4.4 outside all three.
pub fn redraw();
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
### 5.8 `use_size`, and the round cap

The pass order inside one `Tree::draw`:

```
1  enter the runtime
2  render round:   reconcile from the root, then drain the redraw set,
                   parents before children
3  layout round:   measure bottom-up, assign top-down, write each scope's area
4  if any scope that called use_size was assigned a rectangle other than the
   one use_size returned in step 2  →  mark it, go back to 2
5  paint:          walk the tree, record where every listening node landed
6  effects:        cleanups deepest first, then setups shallowest first
7  leave the runtime
```

**R5.8.1** `use_size` answers `Rect::ZERO` on the render in which its component
mounts, and the rectangle from the previous layout round of the same frame
afterwards. A component that calls it therefore renders at least twice on the
frame it mounts.
*test: `a_component_that_asks_its_size_renders_twice_when_it_mounts`*

**R5.8.2** Steps 2 and 3 repeat at most **4** times per frame. On the 4th round
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

Why `use_size` exists at all, in this program's own terms: `draw/pane.rs`
currently calls `viewport.set_height(rect.height, …)` *during* the draw, which
is a state write in the paint pass and is forbidden by R7.1.4. The replacement
is `use_size` in the render pass followed by `view.edit_without_redraw(|v|
v.set_height(h, rows))` — silent, because the frame about to be painted is the
one that will read it, so there is nothing to redraw for.

---

## 6. Reconciliation

The scope tree is what survives a frame. Reconciliation decides, for each node
this frame produced, which live scope it is — and therefore which hook slots,
which `use_state` values, which effect cleanups. That is the whole reason the
framework exists; `use_state` cannot be written without it.

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
listener the one the current frame built. The closures captured last frame's
`State` handles, which are still valid — but they may also have captured props
or local values that have since changed.
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
its effect cleanups are dropped, deepest first; its context offers are dropped;
its focus and hit registrations are removed; its hook slots are dropped; the
slab entry is freed and its generation bumped, so every `State<T>` and every
`Completion<T>` naming it now fails its check (P4.3, R9.3.1).
*test: `unmounting_runs_the_deepest_cleanup_first`*
*test: `a_reply_for_a_component_that_went_away_is_refused`*

**R6.2.7** If the focused scope unmounts, focus moves to the next focusable in
paint order, or to the previous one if there is no next, or to nothing.
*test: `closing_the_focused_pane_moves_focus_rather_than_losing_it`*

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

**R6.3.4** `State::set_if_changed` marks the owner only when the value is not
equal to the one already there. `set` and `edit` always mark. This is what lets
a child report something upward — a status line's contents — during its own
render without the two of them marking each other for ever: the second write is
equal, so nothing is marked, and the frame settles.
*test: `writing_the_same_value_marks_nothing`*
*test: `the_status_line_settles_in_one_frame`*

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

**R7.1.4** Paint writes cells and nothing else. `State::set`, `State::edit` and
`redraw()` called from inside a paint callback panic (P7.2). `State::read`,
`State::cloned` and `State::edit_without_redraw` are legal.
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
captures `Copy` state handles, so rebuilding it costs no clone of any model.
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

Two payloads, one walk. A key starts at focus; a mouse event starts at the
deepest node under the pointer; both then climb.

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

### 8.2 Focus

**R8.2.1** `use_focus` registers a scope as focusable. Focus is one `ScopeId`,
or none. A scope records only whether it *is* focusable; `Focus::has` and
`Paint::has_focus` are comparisons against that single `ScopeId`, so two scopes
cannot both believe they hold focus and no flag has to be cleared when it moves.
*test: `only_one_node_holds_focus`*

**R8.2.2** `Focus::move_next` moves to the next focusable in paint order,
wrapping; `Focus::move_previous` moves back. Both are no-ops when nothing is
focusable.
*test: `focus_wraps_rather_than_running_off_the_end`*

**R8.2.3** A left mouse-down focuses the nearest focusable node at or above the
target, unless a listener between them returned `Bubble::Stop` first.
*test: `clicking_a_pane_focuses_it`*

**R8.2.4** A `focus` listener is called with `true` when the scope gains focus
and `false` when it loses it, during the dispatch that moved focus — not on the
next frame.
*test: `losing_focus_is_reported_before_the_next_frame`*

### 8.3 Bubbling

**R8.3.1** A key goes to the focused scope, then to its parent, then to its
parent, up to the root, stopping at the first listener that returns
`Bubble::Stop`. With no focus it starts at the root.
*test: `an_unhandled_key_reaches_the_root`*

**R8.3.2** A mouse event goes to the hit node and then climbs the same way.
A wheel event is routed by position and climbs the same way.
*test: `a_wheel_over_an_unlistening_child_reaches_the_pane`*

**R8.3.3** A scope that has been unmounted during the same dispatch is skipped
by the rest of the walk.
*test: `a_listener_that_closes_its_own_pane_does_not_climb_into_a_ghost`*

**R8.3.4** `Tree::press` and `Tree::mouse` return whether a listener stopped the
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

`loom` never names a worker, a request or a response. It provides two reply
addresses and the rules under which a reply is refused. The application builds
the requests, owns the threads, and carries the addresses across.

### 9.1 The two addresses

```rust
Completion<T>   one reply, then the address is spent      the file worker
Subscription<T> many replies, until closed                the syntax worker
```

Both are created inside an effect body and both carry
`(ScopeId, slot: u16, generation: u64)` plus a `Weak` to the runtime. A `Weak`
rather than the thread-local, because `Session` delivers a reply from outside a
frame; `complete` and `send` enter the runtime for the duration of the handler
and leave it afterwards.

**R9.1.1** `Completion::complete` runs the handler with the owning scope
entered, marks the address spent, and returns `true`. A second call cannot
happen — `complete` takes `self`.
*test: `a_completion_delivers_once`*

**R9.1.2** `Subscription::send` runs the handler with the owning scope entered
and returns `true`. It may be called any number of times until `close`.
*test: `a_subscription_delivers_every_piece`*

**R9.1.3** A handler may set state, call `redraw()`, and send further requests.
It may not call a hook — it holds no `Scope`.
*test: `a_reply_handler_may_set_state`*

**R9.1.4** `Subscription<T>` is `Clone`, so one worker request can be answered
by several pieces held in different places in the pending. `Completion<T>` is
not.
*test: `a_subscription_can_be_held_twice`*

### 9.2 What the application carries

`pipeline` and `syntax` are untouched. No framework type crosses into them, and
no `u64` token is added to `pipeline::file::Request`.

The reason: the file worker already has one replaceable slot and its `Response`
already carries the `File` it answers, so a token would be an address only `ui`
could read, living in a crate that cannot read it. The syntax worker already
carries `key` and `version` for the same purpose.

The pending is in `ui`:

```rust
// crates/ui/src/app/pending.rs
/// What a component asks the loop to do. Posted to the outbox during a render
/// or an effect, drained by `Session` after the frame.
pub enum Request {
    Open {
        file: file_types::File,
        reply: loom::Completion<pipeline::file::Response>,
    },
    Colour {
        requests: Vec<syntax::SyntaxRequest>,
        reply: loom::Subscription<syntax::SyntaxResponse>,
    },
    /// Quit, suspend, rebuild — the three things only the loop can do.
    Program(crate::input::ProgramAction),
}

/// Requests a component has raised and the loop has not yet sent.
///
/// An `Rc<RefCell<Vec<_>>>` rather than a channel: components and the loop are
/// the same thread, and a channel here would mean a `Send` bound on a reply
/// address that names a scope in a thread-local runtime.
#[derive(Default)]
pub struct Outbox(std::cell::RefCell<Vec<Request>>);

impl Outbox {
    pub fn send(&self, request: Request);
    pub fn drain(&self) -> Vec<Request>;
}

/// Replies the loop is still holding an address for.
#[derive(Default)]
pub struct PendingReplies {
    file: Option<loom::Completion<pipeline::file::Response>>,
    colour: std::collections::HashMap<String, loom::Subscription<syntax::SyntaxResponse>>,
}
```

The loop's half:

```
Request::Open      →  pending.file = Some(reply);  workers.files.send(file)
Event::FileReady   →  workers.files.received(&response);
                      pending.file.take().is_some_and(|reply| reply.complete(response))

Request::Colour    →  for request in &requests { pending.colour.insert(request.key.clone(), reply.clone()) }
                      for request in requests  { workers.syntax.send(request) }
Event::Coloured    →  workers.syntax.received(&response);
                      let last = !response.more;
                      let key = response.key.clone();
                      pending.colour.get(&key).is_some_and(|reply| reply.send(response));
                      if last { pending.colour.remove(&key); }

Request::Program   →  the Flow the loop returns from `Session::drain`
```

### 9.3 Generation rules

**R9.3.1** A reply is refused when the scope's slab generation has moved on —
the component unmounted.
*test: `a_reply_for_a_component_that_went_away_is_refused`*

**R9.3.2** A reply is refused when the slot no longer holds an effect, or holds
a different hook — the component's hook order changed.
*test: `a_reply_into_a_slot_that_changed_shape_is_refused`*

**R9.3.3** A reply is refused when the effect's generation has moved on — the
effect's deps changed and it ran again, so this address belongs to a question
nobody is asking any more. This is what makes
`if self.selected.as_ref() != Some(&response.file) { return false }` in
`app/workers.rs` disappear: the address is stale, not the value.
*test: `a_diff_for_a_file_the_reader_left_is_refused`*

**R9.3.4** A refused reply is not an error. Returning `false` is the whole
report; the value is dropped.
*test: `a_refused_reply_is_dropped_quietly`*

**R9.3.5** An effect's cleanup closes every address it created, before the
cleanup value is dropped.
*test: `changing_the_file_closes_the_previous_diff_address`*

### 9.4 Syntax spans, arriving in pieces

The syntax worker answers a request with several `SyntaxResponse`s, oldest
first, each carrying `from`, `spans` and `more`. The question is how a pane
re-renders when spans arrive for lines it shows, without every pane re-rendering
on every piece.

**The mechanism is two parts: an addressed subscription, and an overlap test.**

*Addressed:* the reply reaches one scope, because the `Subscription` names one
scope's effect slot. No other component is told anything. A pane that is not
showing that file has no address in the pending and hears nothing.

*Overlap test:* the handler installs the piece into the shared store and then
redraws **only if the piece covers a line the pane is showing**:

```rust
let from = response.from;
let taken = store.edit_without_redraw(|store| store.install(response));
if taken && from < visible.end {
    redraw();
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

**R9.4.4** The store is one `use_state(scope, Store::new)` at the root, provided
as context by its `State<Store>` handle, which is `Copy`. Reading it costs no
clone and writing it is `edit_without_redraw`, so installing spans never by itself
redraws anything.
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

---

## 10. Context

Type-keyed, walked up the parent chain, stopping at the first ancestor that
offers the type.

**R10.1.1** `provide_context(scope, value)` stores `(TypeId::of::<T>(), value)`
on the providing scope, with a version. A second call for the same type in the
same component replaces the first.
*test: `the_later_offer_in_one_component_wins`*

**R10.1.2** `context::<T>(scope)` walks `parent` until it finds a scope offering
`T`, and returns a clone. A nearer provider shadows a further one.
*test: `a_nearer_provider_shadows_a_further_one`*

**R10.1.3** With no provider, `context` panics (P10.1) and `try_context`
answers `None`.
*test: `a_missing_context_names_the_type_and_the_component`*

**R10.1.4** A read is recorded on the reading scope as `(TypeId, version)`. A
provider's version increases on every render of the providing component. A
memoised scope whose recorded version is behind is re-rendered even though its
props did not change. Without this, memoisation is a correctness bug rather
than an optimisation.
*test: `a_memo_component_whose_context_changed_runs_anyway`*

**R10.1.5** The version is not compared by value. A context type is
`Clone + 'static` and nothing more — requiring `PartialEq` would put a bound on
`Theme`, `Rc<Outbox>` and every future context for the sake of a comparison
that only matters below a memoised component, of which this program has none. A
component that wants to skip work when a context value did not change memoises
on the value with `use_memo`, where `PartialEq` is already required and is
required of the value rather than of the type.
*test: `a_provider_that_re_renders_re_runs_its_memoised_consumers`*

**R10.1.6** Two contexts of the same underlying type are one context. An
application that wants two wraps one in a newtype.
*test: `two_offers_of_one_type_are_one_context`*

Storage is a `Vec<(TypeId, Rc<dyn Any>, u64)>` on the scope, not a `HashMap`:
no component in a terminal program offers more than a handful of values, and a
linear scan of a handful beats hashing one.

There is no `Provider` component. `provide_context(scope, theme)` at the top of
a render does the same job without adding a node to the tree, and a `Provider`
wrapper can be written on top of it in eight lines if anyone misses the shape.

The contexts this application has, and where they are offered:

| type | offered by | read by |
|---|---|---|
| `Theme` (`Copy`) | `Screen` | every component that paints |
| `State<syntax::Store>` (`Copy`) | `Screen` | `DiffPane` |
| `State<input::Resolver>` (`Copy`) | `Screen` | `ExplorerPane`, `DiffPane` |
| `Rc<Outbox>` | `Screen` | `Screen`, `DiffPane` |
| `Open` | `Screen` | `ExplorerPane` |
| `Run` | `Screen` | `ExplorerPane`, `DiffPane` |

### Talking to an ancestor

A child that has something to tell an ancestor calls a function the ancestor
gave it. A parent passes it as a prop; anything deeper reads it from context.
Both are ordinary values — a callback context is
`#[derive(Clone)] pub struct Open(pub Rc<dyn Fn(File)>)`, and R10.1.6 is why it
is a newtype rather than a bare `Rc<dyn Fn(File)>`.

There is no upward-routed payload. A framework one would let a child announce
something into the air and let whichever ancestor happens to be listening take
it, which reads well in a demo and fails silently when nobody is: no compiler
error, no panic, nothing on screen. A callback that is not provided panics at
`context` naming the type and the component (P10.1), and a callback that is not
passed does not compile.

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

**P4.3 — a state handle was captured into something that outlived its component.**

```text
a State was used after ExplorerPane was removed
```

**P4.4 — a state handle was used with no runtime entered.**

```text
state may only be used inside a component, a listener, an effect or a worker reply
```

**P4.5 — re-entrant `with`, `edit` or `edit_without_redraw` on one state.** Nesting
them on *different* states is legal and is how a canvas reads a buffer, a
viewport and a store at once.

```text
a State was edited from inside its own edit
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

**P7.1 — R5.8.4.**

```text
draw was called from inside a paint callback
```

**P7.2 — R7.1.4.** Only `set`, `set_if_changed`, `edit` and `redraw()`; `read`,
`cloned` and `edit_without_redraw` are legal while painting.

```text
DiffPane: state was set while painting — paint reads, it does not write
```

**P7.3 — R7.2.1. Debug builds only.**

```text
ExplorerPane painted at (39, 4), outside its clip 0,0 40x9
```

**P10.1 — `context::<T>` with no provider. `try_context` answers `None` instead.**

```text
no ui::theme::Theme was provided above StatusBar
```

**P14.1 — `Harness::screen_row` out of range.** A test helper: a wrong index is a
broken test, not a condition to handle.

```text
row 9 is outside the 8-row screen
```

### 12.1 What does not panic

| operation | answer instead |
|---|---|
| `try_context::<T>` | `None` |
| `Completion::complete`, `Subscription::send` | `false` when refused |
| `Tree::press`, `Tree::mouse` | `false` when nobody stopped it |
| `use_size` before the first layout | `Rect::ZERO` |
| `Focus::request`, `Focus::move_next`, `Focus::move_previous` with nothing focusable | no-op |
| `State::set_if_changed` with an equal value | writes, marks nothing |
| a container too small for its children | paints its `too_small` node (R5.4) |
| a rectangle of zero width or height | painted as nothing (R5.3.5) |
| `Harness::area_of` for an unknown name | `None` |
| a canvas writing outside the cell grid | dropped by `Buffer::cell_mut`, which answers `Option` |
| a `for` over an empty iterator | `Node::Fragment(vec![])`, which flattens to nothing |
| a component returning `Node::Empty` | a scope with no children |
| the 5th layout round in one frame | painted with round 4's rectangles; `Tree::layout_rounds()` reports 4 (R5.8.2) |
| a reply for a component that unmounted | refused, value dropped (R9.3.1) |
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
| **I9** | No reply is ever applied to a component that did not ask for it. | `a_diff_for_a_file_the_reader_left_is_refused` | drop the generation from `Completion` |
| **I10** | Hook slots are read in call order, and a divergence is caught on the render that diverges. | `a_render_that_skips_a_hook_is_refused` | remove the count check at the end of a render |
| **I11** | An effect's cleanup runs before its next setup, and deepest first on unmount. | `unmounting_runs_the_deepest_cleanup_first` | run cleanups shallowest first |
| **I12** | A frame runs a component only when its props changed, its own state changed, or its parent ran. | `a_clean_component_is_painted_without_being_run` | mark every scope on every frame |
| **I13** | `loom` names no application crate. | `cargo xtask lint-arch` | add `use ui::Theme;` to `crates/loom/src/paint/text.rs` |
| **I14** | Every file in `loom` and `loom-macros` is under the 300-line soft cap. | `cargo xtask lint-size` | concatenate `reconcile.rs` and `scope.rs` |
| **I15** | `Session::draw_into(&mut Cells, Rect)` keeps its signature through every migration phase, and `crates/ui/tests/explorer/*` (1,505 lines across six files) and `crates/codediff/tests/screens.rs` pass unchanged. | the existing suites, run at the end of every phase | change any phase's screen output by one cell |

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
    .provide(Theme::DARK);

    assert_eq!(screen.draw().row(0).chars().count(), 16);
    assert!(!screen.row(0).contains("changes"));
}
```

**A behaviour, through events.** The harness sends keys and clicks, and answers
questions about the tree:

```rust
#[test]
fn clicking_a_row_moves_the_cursor_and_opens_the_file() {
    let mut screen = Harness::new::<ExplorerPane>(props(), 40, 10).provide(Theme::DARK);
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
| `src/node.rs` | `Node`, `Host`, `Part`, `Key`, `Element`, the `From` impls | 150 | |
| `src/component.rs` | `Component`, the erased render pointer, the props comparison pointer | 80 | |
| `src/scope.rs` | `Scope`, `ScopeId`, `Mounted`, the slab and its free list, parent and child walking | 190 | |
| `src/tree.rs` | `Tree` — the object the application owns | 200 | |
| `src/frame.rs` | the seven steps of §5.6, the round caps | 180 | |
| `src/reconcile.rs` | flattening, positional and keyed matching, mount, update, unmount | 260 | |
| `src/current.rs` | the thread-local, the guard that enters and leaves it | 100 | |
| `src/hook/mod.rs` | `Slot`, `Hooks`, `use_hook`, order checking, `redraw` | 130 | |
| `src/hook/state.rs` | `State<T>` and its five verbs | 190 | |
| `src/hook/memo.rs` | `use_memo` | 80 | |
| `src/hook/effect.rs` | `use_effect`, the effect queue, cleanup by `Drop` | 170 | |
| `src/hook/context.rs` | `provide_context`, `context`, `try_context`, versions | 110 | |
| `src/hook/worker.rs` | `Completion`, `Subscription`, `Effect`, the generation checks | 170 | |
| `src/hook/screen.rs` | `use_size`, `use_focus` | 110 | |
| `src/layout/mod.rs` | `Layout`, `Basis`, `Edges` | 110 | |
| `src/layout/flex.rs` | measure, the §5.4 resolve, the cross axis, `too_small` | 250 | `crates/ui/src/render/layout.rs`, 437 with tests |
| `src/paint/mod.rs` | the walk, clipping, `Paint`, the debug clip guard | 160 | `draw/screen.rs` 97 + `tab.rs` 73 + `pane.rs` 66 |
| `src/paint/host.rs` | `Row`, `Column`, `Stack`, `Gap`, `Divider`, `Canvas` and their props | 220 | |
| `src/paint/text.rs` | `Text`, and the one `measure` in the crate | 110 | |
| `src/event/mod.rs` | `Bubble`, `Mouse`, `Listeners`, `Focus` | 140 | |
| `src/event/hit.rs` | where each node landed, hit-testing, pointer capture | 140 | `draw/screen_map.rs`, 176 |
| `src/event/route.rs` | key, mouse and wheel routing; focus order | 180 | `app/mouse.rs` 125 + `app/keys.rs` 61 |
| `src/testing.rs` | `Harness`, `Probe` | 200 | `crates/ui/src/testing.rs`, 118 |
| | **`loom`** | **≈ 3,670** | |

### 14.2 `crates/loom-macros`

| file | responsibility | est. |
|---|---|---:|
| `src/lib.rs` | the two entry points, `rsx!` and `#[component]` | 60 |
| `src/component.rs` | the props struct, the two impls, the hook-position check | 180 |
| `src/rsx/mod.rs` | the two halves, and the error type they share | 40 |
| `src/rsx/parse.rs` | the grammar of §11.1 | 260 |
| `src/rsx/expand.rs` | the table of §11.2 | 220 |
| | **`loom-macros`** | **≈ 760** |

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
dependency. `unsafe_code = "forbid"` holds, which is the constraint that
produced `State<T>`'s design.

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

Each pane resolves keys through the `State<Resolver>` it reads from context and
handles the actions it owns; everything else goes to the `Run` callback the
root put in context. Bubbling (R8.3) replaces `keymap::live()`'s hand-written
innermost-first walk.
`input::keymap`'s `const` tables and `input::Resolver` are untouched, so
`lint-arch`'s clock rule over `crates/ui/src/input` still holds. `app/keys.rs`
(61 lines) shrinks to the program-level bindings on the root: quit, suspend,
rebuild.

### Phase 7 — state moves in

`View`, `Tab` and `Pane` dissolve into `use_state` in the components that own
them. `Buffer` and `Viewport` stay exactly as they are — they are a good model
and the framework has no opinion about them. `BufferId` and `PaneId` disappear,
because the indirection they exist to provide — a pane cannot hold `&mut Buffer`
without making `View` self-referential — is what `ScopeId` provides now.

Measured: `view/mod.rs` 284, `view/tab.rs` 245, `view/pane.rs` 23.

### Phase 8 — workers

`Outbox`, `Request` and `PendingReplies` as §9.2. `Session::send_file_request` and
`send_colour_request` become one `Session::drain`. `ui::view::buffer::colour`
gains a function that returns the `SyntaxRequest`s instead of sending them, so
the pane can build them in its effect (R9.4.5); everything else in that file is
unchanged.

**Green when:** `crates/codediff/tests/syntax.rs` and `pipeline.rs` pass, and
the new tests R9.4.1 through R9.4.6.

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
    /// Requests components raised and the loop has not yet sent.
    outbox: Rc<Outbox>,
    /// Reply addresses the loop is holding.
    pending: PendingReplies,
}

impl Session {
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

    /// Sends everything components asked for, remembers where each reply goes,
    /// and reports what the loop should do next.
    pub fn drain(&mut self) -> Flow { /* §9.2, plus Request::Program */ }
}
```

`draw_into` keeps the signature `crates/ui/tests/explorer/*` calls (I15), and
draining inside it keeps the existing test flow — draw, then the worker has
been asked — working with no edit to `TestSession`.

Its responsibilities are exactly: own the terminal, own the worker threads,
normalise crossterm events, hand replies to their addresses, call `Tree::draw`,
and answer quit, suspend and rebuild. That is what a `Session` should have been.

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
        Event::FileReady(response) => session.deliver_file(response),
        Event::Coloured(response) => session.deliver_colour(response),
        Event::ListRefreshed(files) => session.set_files(files),
        Event::FsChanged(_) => session.workers.list_worker.send(list::Request::worktree(root)),
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
// crates/ui/src/app/callback.rs
use std::rc::Rc;

/// Open this file. Provided by the root, called by the explorer when the
/// reader lands on a row.
#[derive(Clone)]
pub struct Open(pub Rc<dyn Fn(file_types::File)>);

/// Carry out a command this pane does not own. Provided by the root, called by
/// whichever pane resolved the key.
#[derive(Clone)]
pub struct Run(pub Rc<dyn Fn(crate::input::Command)>);
```

Newtypes rather than bare `Rc<dyn Fn(_)>` because context is keyed by type
(R10.1.6), and because the name says which way the value goes.

```rust
// crates/ui/src/app/status.rs
/// What the status line says.
///
/// `draw::status::Status`, owned rather than borrowed, because props are
/// `'static`. Whichever pane holds focus writes this during its own render;
/// the root reads it and hands it to `StatusBar`. `set_if_changed` rather than
/// `set`, so the second, identical write marks nothing and the frame settles
/// (R6.3.4).
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

`Request`, `Outbox` and `PendingReplies` are §9.2.

## A.1 `StatusBar`

Replaces `draw/status.rs`'s hand-placed offsets. `summary()` and `name()` move
across unchanged; `name()` becomes the canvas, because it drops the directory
before the file name and that is text fitting, not layout (§7.3).

```rust
use std::rc::Rc;

use file_types::File;
use loom::{Basis, Canvas, CanvasProps, Layout, Node, Row, RowProps, Scope, Text, TextProps,
           component, context, rsx, use_size};
use ratatui::buffer::Buffer as Cells;

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
    let theme = context::<Theme>(scope);
    let width = use_size(scope).width;

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
        Row { layout: row, ..,
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
use crossterm::event::{MouseButton, MouseEventKind};
use file_types::File;
use loom::{Bubble, Canvas, CanvasProps, Layout, Listeners, Mouse, Node, Scope, State,
           component, context, rsx, use_effect, use_focus, use_size, use_state};

use crate::input::{Action, BufferAction, KeymapType, Resolution, Resolver, ViewAction};
use crate::theme::Theme;
use crate::view::{Buffer, BufferType, Viewport};
use crate::app::callback::{Open, Run};
use crate::app::status::Status;

#[component]
pub fn ExplorerPane(scope: &mut Scope, files: Rc<[File]>) -> Node {
    let theme = context::<Theme>(scope);
    let keys = context::<State<Resolver>>(scope);
    let status = context::<State<Status>>(scope);
    let Open(open) = context::<Open>(scope);
    let Run(run) = context::<Run>(scope);

    let buffer = use_state(scope, || Buffer::explorer(files.to_vec()));
    let view = use_state(scope, Viewport::new);
    let focus = use_focus(scope);
    let area = use_size(scope);

    // A new list from the watcher. Rebuild the arrangement and keep the reader
    // on the file they were on — `reshape_around` already does that.
    // This is `View::update_explorer`, moved to the thing that owns the list.
    use_effect(scope, files.clone(), move |files, _| {
        let files = files.to_vec();
        let landing = buffer.edit(|buffer| {
            let cursor = view.read(Viewport::cursor);
            let BufferType::Explorer(explorer) = buffer.buffer_type_mut() else {
                return None;
            };
            let landing = explorer.reshape_around(cursor, |e| e.refresh(files));
            buffer.update_line_count();
            Some((landing, buffer.view_lines()))
        });
        if let Some((landing, rows)) = landing {
            view.edit(|v| v.place(landing, rows));
        }
    });

    // The height the frame is about to be painted at, recorded silently:
    // the frame that will read it is the one being prepared.
    let rows = buffer.read(Buffer::view_lines);
    view.edit_without_redraw(|v| v.set_height(u32::from(area.height), rows));

    // What the status line says while this pane has focus. A list of changed
    // files is not a diff, so it has no changes to count.
    if focus.has {
        status.set_if_changed(Status {
            file: None,
            view_line: view.read(Viewport::cursor),
            view_lines: rows,
            ..Status::default()
        });
    }

    let chose = move || {
        let cursor = view.read(Viewport::cursor);
        buffer.read(|buffer| match buffer.buffer_type() {
            BufferType::Explorer(explorer) => explorer.file(cursor).cloned(),
            _ => None,
        })
    };

    let open_key = open.clone();
    let listeners = Listeners::new()
        .on_key(move |key: KeyCombination| {
            let Resolution::Run(command) = keys.edit(|r| r.key(key, KeymapType::Explorer)) else {
                return Bubble::Stop;   // a count, a prefix, or nothing bound
            };
            match command.action {
                Action::Buffer(action) => {
                    let moved = matches!(action, BufferAction::Motion(_));
                    buffer.edit(|b| view.edit(|v| b.apply(action, command.repeat(), v)));
                    if moved && let Some(file) = chose() {
                        open_key(file);
                    }
                    Bubble::Stop
                }
                Action::View(ViewAction::Open) => {
                    let cursor = view.read(Viewport::cursor);
                    let folded = buffer.edit(|b| b.activate(cursor));
                    if folded {
                        let rows = buffer.read(Buffer::view_lines);
                        view.edit(|v| v.place(cursor.min(rows.saturating_sub(1)), rows));
                    } else if let Some(file) = chose() {
                        open_key(file);
                    }
                    Bubble::Stop
                }
                // Tab, view and program actions belong further out. The root
                // gave us the function that carries them out.
                _ => { run(command); Bubble::Stop }
            }
        })
        .on_wheel(move |lines| {
            let rows = buffer.read(Buffer::view_lines);
            view.edit(|v| v.scroll(lines, rows));
            Bubble::Stop
        })
        .on_mouse(move |mouse: Mouse| {
            if mouse.kind != MouseEventKind::Down(MouseButton::Left) {
                return Bubble::Continue;
            }
            focus.request();
            let rows = buffer.read(Buffer::view_lines);
            let line = view.read(Viewport::top) + u32::from(mouse.local.y);
            if line < rows {
                view.edit(|v| v.place(line, rows));
                if let Some(file) = chose() {
                    open(file);
                }
            }
            Bubble::Stop
        });

    let has_focus = focus.has;
    let paint = Rc::new(move |paint: &mut loom::Paint<'_>| {
        let area = paint.area();
        buffer.read(|buffer| {
            let BufferType::Explorer(explorer) = buffer.buffer_type() else { return };
            view.read(|view| {
                crate::draw::buffer::explorer::draw(
                    paint.cells(), area, explorer, view, &theme, has_focus,
                );
            });
        });
    });

    rsx! {
        Canvas {
            layout: Layout {
                grow: 1, min_width: 8, clip: true, ..Default::default()
            },
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

use loom::{Bubble, Canvas, CanvasProps, Layout, Listeners, Node, Scope, State,
           component, context, redraw, rsx, use_effect, use_focus, use_size,
           use_state};
use syntax::{Store, SyntaxResponse, Version};

use crate::app::pending::{Outbox, Request};
use crate::app::callback::Run;
use crate::app::status::Status;
use crate::draw::Look;
use crate::input::{Action, KeymapType, Resolution, Resolver};
use crate::theme::Theme;
use crate::view::{Buffer, Viewport};

/// Read ahead of the screen, so scrolling finds colour already there.
const MARGIN: u32 = 2_000;

#[component]
pub fn DiffPane(scope: &mut Scope, buffer: State<Option<Buffer>>, version: Version) -> Node {
    let theme = context::<Theme>(scope);
    let store = context::<State<Store>>(scope);
    let keys = context::<State<Resolver>>(scope);
    let post = context::<Rc<Outbox>>(scope);
    let status = context::<State<Status>>(scope);
    let Run(run) = context::<Run>(scope);

    let view = use_state(scope, Viewport::new);
    let focus = use_focus(scope);
    let area = use_size(scope);

    let rows = buffer.read(|b| b.as_ref().map_or(0, Buffer::view_lines));
    view.edit_without_redraw(|v| v.set_height(u32::from(area.height), rows));
    let visible = view.read(|v| v.visible(rows));
    let keymap = buffer.read(|b| b.as_ref().map_or(KeymapType::default(), Buffer::keymap_type));

    if focus.has {
        // `draw::screen::summary`, moved to the pane that knows the answers.
        status.set_if_changed(buffer.read(|b| b.as_ref().map_or_else(Status::default, |b| Status {
            file: b.file().cloned().map(Rc::new),
            view_line: view.read(Viewport::cursor),
            view_lines: b.view_lines(),
            changes: b.blocks().len(),
            change: b.block_at(view.read(Viewport::cursor)),
            timed_out: b.hit_timeout(),
            exhausted: b.exhausted(),
        })));
    }

    // Colour. The deps say what has arrived as well as what is needed, so
    // installing a piece asks for the next one and nothing else does. R9.4.5.
    let coloured = store.read(|store| buffer.read(|b| {
        b.as_ref().map_or((0, 0), |b| b.coloured_lines(store))
    }));
    let end = visible.end;
    use_effect(scope, (*version, end, coloured), move |_, effect| {
        let reply = effect.subscription(move |response: SyntaxResponse| {
            let from = response.from;
            let taken = store.edit_without_redraw(|store| store.install(response));
            // Only a piece covering a line this pane is showing is worth a
            // frame. `end` is a view line, which is never below the file line
            // at the same place, so this never skips a redraw that was needed.
            if taken && from < end {
                redraw();
            }
        });
        let requests = store.edit_without_redraw(|store| {
            buffer.read(|b| b.as_ref()
                .map(|b| b.colour_requests(store, *version, end + MARGIN))
                .unwrap_or_default())
        });
        if requests.is_empty() {
            reply.close();
        } else {
            post.send(Request::Colour { requests, reply });
        }
    });

    let listeners = Listeners::new()
        .on_key(move |key| {
            let Resolution::Run(command) = keys.edit(|r| r.key(key, keymap)) else {
                return Bubble::Stop;
            };
            match command.action {
                Action::Buffer(action) => {
                    buffer.edit(|b| {
                        if let Some(b) = b { view.edit(|v| b.apply(action, command.repeat(), v)) }
                    });
                    Bubble::Stop
                }
                _ => { run(command); Bubble::Stop }
            }
        })
        .on_wheel(move |lines| {
            let rows = buffer.read(|b| b.as_ref().map_or(0, Buffer::view_lines));
            view.edit(|v| v.scroll(lines, rows));
            Bubble::Stop
        })
        .on_mouse(move |mouse| {
            focus.request();
            // A drag that leaves the column still belongs to it. R8.4.1.
            if matches!(mouse.kind, crossterm::event::MouseEventKind::Down(_)) {
                loom::capture_pointer();
            }
            Bubble::Stop
        });

    let has_focus = focus.has;
    let paint = Rc::new(move |paint: &mut loom::Paint<'_>| {
        let area = paint.area();
        buffer.read(|buffer| {
            let Some(buffer) = buffer else { return };
            store.read(|store| {
                view.read(|view| {
                    // Byte for byte, today's `draw::buffer::draw` call.
                    let look = Look { theme: &theme, syntax: true, store };
                    crate::draw::buffer::draw(
                        paint.cells(), area, buffer, view, look, has_focus,
                    );
                });
            });
        });
    });

    rsx! {
        Canvas {
            layout: Layout {
                grow: 1, min_width: 20, clip: true, ..Default::default()
            },
            focusable: true,
            listeners,
            paint,
        }
    }
}
```

Note which state lives where, and why it matters. The `Viewport` is the pane's
own, keyed by file name in the root (below), so re-opening a file the reader
was on keeps their cursor and opening a different one starts at its top —
which is `View::show`'s `keep` and `Tab::set_right_pane`'s fresh pane, both for
free. The `Buffer` belongs to the root, because that is where the worker's
reply lands. Scrolling therefore marks one component; opening a file marks the
root, once.

## A.4 `Screen`, the root

Replaces `draw/screen.rs`, `draw/tab.rs`, `draw/pane.rs`, `view/tab.rs`,
`view/pane.rs` and most of `app/mod.rs`.

```rust
use std::rc::Rc;

use file_types::File;
use loom::{Column, ColumnProps, Divider, DividerProps, Layout,
           Basis, Node, Row, RowProps, Scope, Text, TextProps, component,
           provide_context, rsx, use_effect, use_state};
use syntax::{Store, Version};

use crate::app::pending::{Outbox, Request};
use crate::app::callback::{Open, Run};
use crate::app::status::Status;
use crate::input::{Action, Command, Resolver, TabAction, ViewAction};
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
pub fn Screen(scope: &mut Scope, files: Rc<[File]>, theme: Theme, post: Rc<Outbox>) -> Node {
    let store = use_state(scope, Store::new);
    let keys = use_state(scope, Resolver::new);
    let status = use_state(scope, Status::default);

    let opened = use_state(scope, || None::<Buffer>);
    let notice = use_state(scope, || None::<Rc<str>>);
    let chosen = use_state(scope, || None::<File>);
    let version = use_state(scope, || Version(1));
    let left = use_state(scope, || DEFAULT_LEFT);

    // The two things a pane asks the root to do. Every capture is a `State`
    // handle, which is `Copy` and two integers wide, so rebuilding these each
    // render costs one allocation apiece.
    let post_program = post.clone();
    provide_context(scope, Open(Rc::new(move |file: File| chosen.set(Some(file)))));
    provide_context(scope, Run(Rc::new(move |command: Command| match command.action {
        Action::Tab(TabAction::FocusNext | TabAction::FocusPrev) => {
            // Focus order is paint order, so "the next pane" needs no table of
            // its own. R8.2.2.
        }
        Action::Tab(TabAction::WidenLeft) => left.edit(|n| *n = (*n + 2).min(MAX_LEFT)),
        Action::Tab(TabAction::NarrowLeft) => {
            left.edit(|n| *n = n.saturating_sub(2).max(MIN_LEFT));
        }
        Action::View(ViewAction::ToggleLayout) => opened.edit(|b| {
            if let Some(buffer) = b.take() {
                *b = Some(buffer.switch_diff_layout());
            }
        }),
        Action::Program(action) => post_program.send(Request::Program(action)),
        _ => {}
    })));

    provide_context(scope, theme);
    provide_context(scope, post.clone());
    provide_context(scope, store);
    provide_context(scope, keys);
    provide_context(scope, status);

    // Ask for the file the reader chose. The reply address is created here,
    // so a reply for a file they have since left is refused rather than
    // shown — R9.3.3, which is what `apply_file_response`'s `if
    // self.selected.as_ref() != Some(&response.file)` was for.
    let post_open = post.clone();
    use_effect(scope, chosen.cloned(), move |chosen, effect| {
        let Some(file) = chosen.clone() else { return };
        let reply = effect.completion(move |response: pipeline::file::Response| {
            match response.content {
                Ok(content) => {
                    opened.set(Some(Buffer::diff(content)));
                    notice.set(None);
                    version.edit(|v| v.0 += 1);
                }
                Err(why) => notice.set(Some(why.into())),
            }
        });
        post_open.send(Request::Open { file, reply });
    });

    // The key is the file, so re-opening the same file keeps the same scope
    // and therefore the same viewport, while a different file gets a fresh
    // one. R6.1.4.
    let shown = opened.read(|b| b.as_ref()
        .and_then(Buffer::file)
        .map(|f| f.path().as_str().to_owned()));
    let split = shown.is_some();

    // The list asks for the width the reader chose and gives it back when the
    // diff needs its minimum — R5.4.4 and R5.4.5, which is what
    // `render::layout::split`'s clamp did by hand.
    let explorer = Layout {
        basis: if split { Basis::Length(left.cloned()) } else { Basis::Auto },
        grow: if split { 0 } else { 1 },
        shrink: 1,
        min_width: 8,
        ..Default::default()
    };
    let shown_status = status.cloned();

    rsx! {
        Column {
            layout: Layout { grow: 1, ..Default::default() },
            too_small: Some(rsx! {
                Text { text: "terminal too small".into(), style: theme.normal, .. }
            }),
            ..,

            Row { layout: Layout { grow: 1, ..Default::default() }, ..,
                Column { layout: explorer, ..,
                    ExplorerPane { files: files.clone() }
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
                        DiffPane { key: name, buffer: opened, version: version.cloned() }
                    }
                }
            }

            StatusBar {
                file: shown_status.file, view_line: shown_status.view_line,
                view_lines: shown_status.view_lines, changes: shown_status.changes,
                change: shown_status.change, timed_out: shown_status.timed_out,
                exhausted: shown_status.exhausted, notice: notice.cloned(),
            }
        }
    }
}
```

`render::layout::split`'s clamp — *a wider screen never shows less than a
narrower one* — becomes R5.4.4 with the same test name, and
`draw/tab.rs`'s "a pane that cannot draw falls back to one pane rather than
failing the whole screen" becomes the `too_small` climb of R5.4.2.

### A.5 Where each piece of `View` went

| today | tomorrow |
|---|---|
| `View::buffers` | `use_state` in the component that shows each one |
| `View::tabs`, `Tab::panes`, `Tab::layout` | the node tree |
| `Tab::focus`, `focus_next` | `use_focus`, `Focus::move_next`, paint order (R8.2.2) |
| `Tab::resize` | `left: State<u16>` in `Screen` |
| `Pane::viewport` | `use_state(scope, Viewport::new)` in each pane |
| `BufferId`, `PaneId` | `ScopeId` |
| `View::selection`, `PendingSelection` | `use_state` in `DiffPane`, plus pointer capture |
| `View::version` | `version: State<Version>` in `Screen` |
| `View::request` | the syntax effect in `DiffPane` (R9.4.5) |
| `View::update_explorer` | the `files` effect in `ExplorerPane` |
| `View::selected_file` | `chosen: State<Option<File>>` in `Screen`, set through the `Open` callback |
| `View::keymap_type` | each pane's own `KeymapType`, at the point it resolves a key |
| `ScreenMap` | the paint walk's record (R7.1.5) |
| `Look` | `Theme` and `State<Store>` in context |

