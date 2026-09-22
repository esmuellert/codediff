//! Builds frames without holding runtime borrows across callbacks.

use std::collections::HashMap;

use ratatui::buffer::Buffer as Cells;
use ratatui::layout::Rect;

use crate::event::Listeners;
use crate::layout::{Axis, Basis, Item, assign};
use crate::node::NodeHandle;
use crate::paint::Paint;
use crate::reconcile::{Fiber, HostDesc, RuntimeRef};
use crate::runtime::Runtime;
use crate::scope::ScopeId;

/// A laid-out host in paint order.
#[derive(Clone)]
pub(crate) struct FrameNode {
    pub scope: ScopeId,
    /// Which host within that scope, in paint order.
    pub nth: u32,
    pub parent: Option<usize>,
    pub area: Rect,
    pub clip: Rect,
    /// The rectangle before scroll transforms.
    pub content: Rect,
    /// The content-space origin after every scroll transform.
    pub origin: (i64, i64),
    pub host_desc: std::rc::Rc<HostDesc>,
    pub listeners: Listeners,
    pub focusable: bool,
    pub auto_focus: bool,
}

/// Maximum redraw rounds caused by layout effects.
const ROUNDS: usize = 4;

/// Reconcile, lay out, run layout effects, paint, run effects.
pub(crate) fn draw(held: &RuntimeRef, cells: &mut Cells, area: Rect) {
    let Some(root) = held.borrow().root else {
        return;
    };
    held.borrow_mut().renders = 0;

    let mut rounds = 0;
    loop {
        rounds += 1;
        {
            let mut rt = held.borrow_mut();
            commit_state(&mut rt);
            rt.dirty.clear();
        }

        let tree = crate::reconcile::frame(held, root);

        let mut state = LayoutState {
            placed: Vec::new(),
            nth: HashMap::new(),
            metrics_changed: false,
        };
        for node in &tree {
            lay_out(node, area, area, None, Transform::ZERO, &mut state);
        }
        let metrics_changed = state.metrics_changed;
        held.borrow_mut().placed = state.placed;
        if metrics_changed {
            held.borrow_mut().mark(root);
        }

        // Refs are written before layout effects run.
        write_refs(held);
        auto_focus(held);
        run_effects(held, true);

        // State writes from layout effects trigger another round.
        if !held.borrow().needs_draw() || rounds >= ROUNDS {
            break;
        }
    }

    held.borrow_mut().rounds = rounds;

    let count = held.borrow().placed.len();
    for at in 0..count {
        paint_one(held, at, cells);
    }

    // Post-frame effects schedule the next draw.
    run_effects(held, false);
}

/// Moves every pending state value into its slot, so the next render reads it.
fn commit_state(rt: &mut Runtime) {
    for hooks in rt.hooks.values_mut() {
        for slot in &mut hooks.slots {
            if let crate::hook::Slot::State(state) = slot {
                state.commit();
            }
        }
    }
}

/// A signed translation from content coordinates into screen coordinates.
#[derive(Clone, Copy, Debug, Default)]
struct Transform {
    x: i64,
    y: i64,
}

impl Transform {
    const ZERO: Self = Self { x: 0, y: 0 };

    fn subtract(self, offset: crate::scroll::ScrollOffset) -> Self {
        Self {
            x: self.x.saturating_sub(i64::from(offset.x)),
            y: self.y.saturating_sub(i64::from(offset.y)),
        }
    }

    fn add(self, offset: crate::scroll::ScrollOffset) -> Self {
        Self {
            x: self.x.saturating_add(i64::from(offset.x)),
            y: self.y.saturating_add(i64::from(offset.y)),
        }
    }
}

struct LayoutState {
    placed: Vec<FrameNode>,
    nth: HashMap<ScopeId, u32>,
    metrics_changed: bool,
}

/// Lays one host out, then its children, appending to `placed`.
///
/// Returns whether this node cannot fit its children. A container uses its
/// `too_small` node or passes the condition to its parent.
fn lay_out(
    node: &Fiber,
    area: Rect,
    clip: Rect,
    parent: Option<usize>,
    transform: Transform,
    state: &mut LayoutState,
) -> bool {
    let layout = node.host_desc.layout;
    if layout.hidden {
        return false;
    }

    let visible_area = translate(area, transform);
    let visible_clip = clip.intersection(visible_area);
    let host_nth = state.nth.entry(node.scope).or_insert(0);
    let host_nth_value = *host_nth;
    *host_nth = host_nth.saturating_add(1);
    let here = state.placed.len();
    state.placed.push(FrameNode {
        scope: node.scope,
        nth: host_nth_value,
        parent,
        area: visible_area,
        clip: visible_clip,
        content: area,
        origin: (
            i64::from(area.x).saturating_add(transform.x),
            i64::from(area.y).saturating_add(transform.y),
        ),
        host_desc: std::rc::Rc::clone(&node.host_desc),
        listeners: node.host_desc.listeners.clone(),
        focusable: node.host_desc.focusable,
        auto_focus: node.host_desc.auto_focus,
    });

    // Remove padding before laying out children. Layout itself stays in
    // content coordinates; only placed rectangles receive the transform.
    let inner = inset(area, layout.pad);
    if node.host_desc.scroll.is_some() {
        lay_out_scroll(node, inner, visible_clip, here, transform, state);
        return false;
    }

    // Clip children to the parent's inner area.
    let visible_inner = translate(inner, transform);
    let inner_clip = if layout.clip {
        visible_clip.intersection(visible_inner)
    } else {
        visible_clip
    };

    let items: Vec<Item> = node
        .children
        .iter()
        .map(|child| Item {
            layout: child.host_desc.layout,
            measured: measure(child, node.host_desc.axis, inner, node.host_desc.axis),
        })
        .collect();

    let out = assign(node.host_desc.axis, inner, layout.gap, &items);

    // Everything below this node, so it can be taken back if the subtree
    // turns out not to fit.
    let below = state.placed.len();

    let mut short = out.too_small;
    if !short {
        for (child, child_area) in node.children.iter().zip(out.areas) {
            // Child rectangles stay inside the parent.
            let child_area = child_area.intersection(inner);
            let visible_child = translate(child_area, transform);
            short |= lay_out(
                child,
                child_area,
                inner_clip.intersection(visible_child),
                Some(here),
                transform,
                state,
            );
        }
    }

    if !short {
        return false;
    }

    // Replace children with the `too_small` fallback when they do not fit.
    state.placed.truncate(below);
    let Some(message) = &node.too_small else {
        return true;
    };
    for child in message.iter() {
        lay_out(child, inner, inner_clip, Some(here), transform, state);
    }
    false
}

/// Lays the content of a Scroll host in natural content coordinates.
fn lay_out_scroll(
    node: &Fiber,
    inner: Rect,
    viewport_clip: Rect,
    parent: usize,
    transform: Transform,
    state: &mut LayoutState,
) {
    let natural_width = node
        .children
        .iter()
        .map(|child| u32::from(measure(child, Axis::Across, inner, node.host_desc.axis)))
        .max()
        .unwrap_or(0);
    let natural_height = node
        .children
        .iter()
        .map(|child| u32::from(measure(child, Axis::Down, inner, node.host_desc.axis)))
        .fold(0u32, |height, child| height.saturating_add(child))
        .saturating_add(
            u32::from(node.host_desc.layout.gap)
                .saturating_mul(node.children.len().saturating_sub(1) as u32),
        );
    let metrics = crate::scroll::ScrollMetrics {
        content_width: if node.host_desc.scroll_axes.0 {
            node.host_desc
                .scroll_content_width
                .unwrap_or(natural_width)
                .max(u32::from(inner.width))
        } else {
            u32::from(inner.width)
        },
        content_height: if node.host_desc.scroll_axes.1 {
            node.host_desc
                .scroll_content_height
                .unwrap_or(natural_height)
                .max(u32::from(inner.height))
        } else {
            u32::from(inner.height)
        },
        viewport_width: u32::from(inner.width),
        viewport_height: u32::from(inner.height),
    };
    if let Some(view) = &node.host_desc.scroll {
        if node.host_desc.scroll_write_metrics && view.state.replace(metrics) != metrics {
            state.metrics_changed = true;
        }
        let content_transform = transform
            .subtract(view.requested.clamp(metrics))
            .add(node.host_desc.scroll_content_offset);
        let content_width = node
            .host_desc
            .scroll_content_area_width
            .unwrap_or(metrics.content_width)
            .min(u32::from(u16::MAX)) as u16;
        let content_height = metrics.content_height.min(u32::from(u16::MAX)) as u16;
        let content_area = Rect {
            x: inner.x,
            y: inner.y,
            width: content_width,
            height: content_height,
        };
        let items: Vec<Item> = node
            .children
            .iter()
            .map(|child| Item {
                layout: child.host_desc.layout,
                measured: measure(child, Axis::Down, content_area, node.host_desc.axis),
            })
            .collect();
        let assigned = assign(Axis::Down, content_area, node.host_desc.layout.gap, &items);
        for (child, child_area) in node.children.iter().zip(assigned.areas) {
            let visible_child = translate(child_area, content_transform);
            lay_out(
                child,
                child_area,
                viewport_clip.intersection(visible_child),
                Some(parent),
                content_transform,
                state,
            );
        }
    }
}

fn translate(area: Rect, transform: Transform) -> Rect {
    let left = i64::from(area.x).saturating_add(transform.x);
    let top = i64::from(area.y).saturating_add(transform.y);
    let right = left.saturating_add(i64::from(area.width));
    let bottom = top.saturating_add(i64::from(area.height));
    let visible_left = left.max(0).min(i64::from(u16::MAX));
    let visible_top = top.max(0).min(i64::from(u16::MAX));
    let visible_right = right.max(0).min(i64::from(u16::MAX));
    let visible_bottom = bottom.max(0).min(i64::from(u16::MAX));
    if visible_right <= visible_left || visible_bottom <= visible_top {
        return Rect::ZERO;
    }
    Rect {
        x: visible_left as u16,
        y: visible_top as u16,
        width: (visible_right - visible_left) as u16,
        height: (visible_bottom - visible_top) as u16,
    }
}

/// Measures auto-sized children and text.
fn measure(node: &Fiber, axis: Axis, room: Rect, parent_axis: Axis) -> u16 {
    let layout = node.host_desc.layout;
    if node.host_desc.scroll.is_some() {
        return if parent_axis == axis {
            match layout.basis {
                Basis::Length(n) => n,
                Basis::Percent(_) | Basis::Auto => 0,
            }
        } else {
            0
        };
    }
    if parent_axis == axis {
        match layout.basis {
            // Fixed-size children measure as their size.
            Basis::Length(n) => return n,
            // What a percentage is a share of is not known until the flex pass.
            Basis::Percent(_) => return 0,
            Basis::Auto => {}
        }
    }
    if let Some(measure) = node.host_desc.measure {
        let (across, down) = measure(&node.host_desc, room.width);
        return if axis == Axis::Down { down } else { across };
    }

    // Containers measure from their children, gaps, and padding.
    if node.children.is_empty() {
        return 0;
    }

    let pad = if axis == Axis::Down {
        layout.pad.down()
    } else {
        layout.pad.across()
    };
    let gaps = layout
        .gap
        .saturating_mul(node.children.len().saturating_sub(1) as u16);

    if node.host_desc.axis == axis {
        let sum: u32 = node
            .children
            .iter()
            .map(|c| u32::from(measure(c, axis, room, node.host_desc.axis)))
            .sum();
        (sum.min(u32::from(u16::MAX)) as u16)
            .saturating_add(gaps)
            .saturating_add(pad)
    } else {
        let largest = node
            .children
            .iter()
            .map(|c| measure(c, axis, room, node.host_desc.axis))
            .max()
            .unwrap_or(0);
        largest.saturating_add(pad)
    }
}

/// Writes refs before layout effects run.
fn write_refs(held: &RuntimeRef) {
    let writes: Vec<(crate::hook::Ref<Option<NodeHandle>>, NodeHandle)> = held
        .borrow()
        .placed
        .iter()
        .filter_map(|p| {
            p.host_desc.node_ref.map(|slot| {
                (
                    slot,
                    NodeHandle {
                        scope: p.scope,
                        nth: p.nth,
                    },
                )
            })
        })
        .collect();
    for (slot, node) in writes {
        *slot.current() = Some(node);
    }
}

/// Runs queued effects and installs their cleanups.
fn run_effects(held: &RuntimeRef, before_paint: bool) {
    let queued = {
        let mut rt = held.borrow_mut();
        if before_paint {
            std::mem::take(&mut rt.layout_effects)
        } else {
            std::mem::take(&mut rt.effects)
        }
    };

    for effect in queued {
        // Ignore replies from previous effect generations.
        let undo = {
            let mut rt = held.borrow_mut();
            let current = generation_of(&rt, effect.scope, effect.slot);
            if current != Some(effect.generation) {
                continue;
            }
            // Run cleanup before the next setup.
            rt.hooks
                .get_mut(&effect.scope)
                .and_then(|h| h.slots.get_mut(effect.slot as usize))
                .and_then(|s| match s {
                    crate::hook::Slot::Effect(e) | crate::hook::Slot::LayoutEffect(e) => {
                        e.cleanup.take()
                    }
                    _ => None,
                })
        };
        if let Some(undo) = undo {
            undo();
        }

        held.borrow_mut().running_effect = Some((effect.scope, effect.slot, effect.generation));
        let cleanup = (effect.run)();
        held.borrow_mut().running_effect = None;

        let mut rt = held.borrow_mut();
        if let Some(crate::hook::Slot::Effect(slot) | crate::hook::Slot::LayoutEffect(slot)) = rt
            .hooks
            .get_mut(&effect.scope)
            .and_then(|hooks| hooks.slots.get_mut(effect.slot as usize))
        {
            slot.cleanup = cleanup;
        }
    }
}

fn generation_of(rt: &Runtime, scope: ScopeId, slot: u16) -> Option<u64> {
    rt.hooks
        .get(&scope)
        .and_then(|h| h.slots.get(slot as usize))
        .and_then(|s| match s {
            crate::hook::Slot::Effect(e) | crate::hook::Slot::LayoutEffect(e) => Some(e.generation),
            _ => None,
        })
}

/// Paints fill first, then the node's own content.
fn paint_one(held: &RuntimeRef, at: usize, cells: &mut Cells) {
    let (desc, area, clip, content, origin, focused) = {
        let rt = held.borrow();
        let node = &rt.placed[at];
        let clip = node.area.intersection(node.clip);
        (
            std::rc::Rc::clone(&node.host_desc),
            node.area,
            clip,
            node.content,
            node.origin,
            rt.focused
                == Some(NodeHandle {
                    scope: node.scope,
                    nth: node.nth,
                }),
        )
    };

    if clip.width == 0 || clip.height == 0 {
        return;
    }

    if let Some(fill) = desc.layout.fill {
        for y in clip.top()..clip.bottom() {
            for x in clip.left()..clip.right() {
                if let Some(cell) = cells.cell_mut((x, y)) {
                    cell.set_style(fill);
                }
            }
        }
    }

    if let Some(text) = &desc.text {
        paint_text(cells, clip, origin, text, desc.style);
    }

    // Painters can read refs and stores while writing cells.
    if let Some(paint) = &desc.paint {
        let mut brush = Paint::new(cells, area, clip, content, origin, focused);
        paint(&mut brush);
    }
}

fn paint_text(
    cells: &mut Cells,
    clip: Rect,
    origin: (i64, i64),
    text: &str,
    style: ratatui::style::Style,
) {
    let y = origin.1;
    if y < i64::from(clip.y) || y >= i64::from(clip.bottom()) {
        return;
    }
    let y = y as u16;
    let mut x = origin.0;
    for (at, ch) in text.char_indices() {
        let grapheme = &text[at..at + ch.len_utf8()];
        let width = ratatui::text::Span::raw(grapheme).width() as i64;
        if width == 0 {
            continue;
        }
        for offset in 0..width {
            let cell_x = x.saturating_add(offset);
            if cell_x < i64::from(clip.x) || cell_x >= i64::from(clip.right()) {
                continue;
            }
            let Some(cell) = cells.cell_mut((cell_x as u16, y)) else {
                continue;
            };
            cell.set_symbol(if offset == 0 { grapheme } else { "" });
            cell.set_style(style);
        }
        x = x.saturating_add(width);
    }
}

fn inset(area: Rect, pad: crate::layout::Edges) -> Rect {
    Rect {
        x: area.x.saturating_add(pad.left),
        y: area.y.saturating_add(pad.top),
        width: area.width.saturating_sub(pad.across()),
        height: area.height.saturating_sub(pad.down()),
    }
}

/// Focuses the first eligible auto-focus node once.
fn auto_focus(held: &RuntimeRef) {
    if held.borrow().focused.is_some() {
        return;
    }
    let target = {
        let rt = held.borrow();
        rt.placed
            .iter()
            .enumerate()
            .find(|(_, p)| p.auto_focus && p.focusable)
            .map(|(i, p)| {
                (
                    i,
                    NodeHandle {
                        scope: p.scope,
                        nth: p.nth,
                    },
                )
            })
    };
    if let Some((_index, node)) = target {
        crate::event::move_focus(held, Some(node));
    }
}
