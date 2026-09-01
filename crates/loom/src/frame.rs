//! The seven steps of a frame: render, lay out, run layout effects, paint,
//! run effects.
//!
//! The runtime is borrowed in short bursts. Nothing here holds it across a
//! component's function, a listener, an effect body or a painter, because all
//! four reach the runtime themselves.

use ratatui::buffer::Buffer as Cells;
use ratatui::layout::Rect;

use crate::event::Listeners;
use crate::layout::{Axis, Basis, Item, assign};
use crate::node::NodeHandle;
use crate::paint::Paint;
use crate::reconcile::{Fiber, HostDesc, RuntimeRef};
use crate::runtime::Runtime;
use crate::scope::ScopeId;

/// One host that got a rectangle. Flat, deepest last, so painting is a walk
/// forward and hit-testing is a walk back.
#[derive(Clone)]
pub(crate) struct FrameNode {
    pub scope: ScopeId,
    /// Which host within that scope, in paint order.
    pub nth: u32,
    pub parent: Option<usize>,
    pub area: Rect,
    pub clip: Rect,
    pub host_desc: std::rc::Rc<HostDesc>,
    pub listeners: Listeners,
    pub focusable: bool,
    pub auto_focus: bool,
}

/// R5.8.2 — a layout effect may write state, which re-renders and re-lays
/// out. Four rounds is far above anything that settles.
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

        let mut placed = Vec::new();
        for node in &tree {
            lay_out(node, area, area, None, &mut placed);
        }
        held.borrow_mut().placed = placed;

        // R5.8 — every `ref` holds its node before a layout effect runs.
        write_refs(held);
        auto_focus(held);
        run_effects(held, true);

        // A layout effect that wrote state gets another round before anything
        // reaches the screen.
        if !held.borrow().needs_draw() || rounds >= ROUNDS {
            break;
        }
    }

    held.borrow_mut().rounds = rounds;

    let count = held.borrow().placed.len();
    for at in 0..count {
        paint_one(held, at, cells);
    }

    // Effects run after the frame is on screen. What they mark belongs to
    // the next one, so nothing is cleared after this.
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

/// Lays one host out, then its children, appending to `placed`.
///
/// Answers whether this node could not fit its children. A container that
/// cannot paints its own `too_small` node instead; one with nothing to say
/// passes the condition to its parent (R5.6.2).
fn lay_out(
    node: &Fiber,
    area: Rect,
    clip: Rect,
    parent: Option<usize>,
    placed: &mut Vec<FrameNode>,
) -> bool {
    let layout = node.host_desc.layout;
    if layout.hidden {
        return false;
    }

    let nth = placed.iter().filter(|p| p.scope == node.scope).count() as u32;
    let here = placed.len();
    placed.push(FrameNode {
        scope: node.scope,
        nth,
        parent,
        area,
        clip,
        host_desc: std::rc::Rc::clone(&node.host_desc),
        listeners: node.host_desc.listeners.clone(),
        focusable: node.host_desc.focusable,
        auto_focus: node.host_desc.auto_focus,
    });

    // R5.5.2 — padding comes off before the children.
    let inner = inset(area, layout.pad);
    // R5.5.3 — a clipping parent shrinks what its children may reach.
    let inner_clip = if layout.clip {
        clip.intersection(inner)
    } else {
        clip
    };

    let items: Vec<Item> = node
        .children
        .iter()
        .map(|child| Item {
            layout: child.host_desc.layout,
            measured: measure(child, node.host_desc.axis, inner),
        })
        .collect();

    let out = assign(node.host_desc.axis, inner, layout.gap, &items);

    // Everything below this node, so it can be taken back if the subtree
    // turns out not to fit.
    let below = placed.len();

    let mut short = out.too_small;
    if !short {
        for (child, child_area) in node.children.iter().zip(out.areas) {
            // I5 — every rectangle handed to a child lies inside its parent's.
            let child_area = child_area.intersection(inner);
            short |= lay_out(
                child,
                child_area,
                inner_clip.intersection(child_area),
                Some(here),
                placed,
            );
        }
    }

    if !short {
        return false;
    }

    // R5.6.2 — a container that cannot fit its children assigns nothing below
    // it and paints its `too_small` node instead. One with nothing to say
    // passes the condition up.
    placed.truncate(below);
    let Some(message) = &node.too_small else {
        return true;
    };
    for child in message.iter() {
        lay_out(child, inner, inner_clip, Some(here), placed);
    }
    false
}

/// R5.3 — only `Basis::Auto` needs measuring, and only `Text` answers.
fn measure(node: &Fiber, axis: Axis, room: Rect) -> u16 {
    let layout = node.host_desc.layout;
    match layout.basis {
        // R5.3.3 — a fixed child measures as its size.
        Basis::Length(n) => return n,
        // What a percentage is a share of is not known until the flex pass.
        Basis::Percent(_) => return 0,
        Basis::Auto => {}
    }

    if let Some(measure) = node.host_desc.measure {
        let (across, down) = measure(&node.host_desc, room.width);
        return if axis == Axis::Down { down } else { across };
    }

    // R5.3.2 — a container measures as the sum of its children along its main
    // axis, plus the gaps, plus padding.
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
            .map(|c| u32::from(measure(c, axis, room)))
            .sum();
        (sum.min(u32::from(u16::MAX)) as u16)
            .saturating_add(gaps)
            .saturating_add(pad)
    } else {
        let largest = node
            .children
            .iter()
            .map(|c| measure(c, axis, room))
            .max()
            .unwrap_or(0);
        largest.saturating_add(pad)
    }
}

/// R5.8 — every `ref` holds its node before a layout effect runs.
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

/// Runs one queue of effects: the old cleanup first, then the body, then the
/// new cleanup into the slot.
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
        // R9.3.3 — a reply from a previous run is refused, so the generation
        // has to still match.
        let undo = {
            let mut rt = held.borrow_mut();
            let current = generation_of(&rt, effect.scope, effect.slot);
            if current != Some(effect.generation) {
                continue;
            }
            // I11 — a cleanup runs before its next setup.
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

/// R7.1 — the walk. Fill first, then the node's own ink.
fn paint_one(held: &RuntimeRef, at: usize, cells: &mut Cells) {
    let (desc, area, clip, focused) = {
        let rt = held.borrow();
        let node = &rt.placed[at];
        let clip = node.area.intersection(node.clip);
        (
            std::rc::Rc::clone(&node.host_desc),
            node.area,
            clip,
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
        let line =
            ratatui::text::Line::from(ratatui::text::Span::styled(text.as_ref(), desc.style));
        ratatui::widgets::Widget::render(line, clip, cells);
    }

    // The runtime is not borrowed here, so a painter may read a ref or a
    // store while it writes cells.
    if let Some(paint) = &desc.paint {
        let mut brush = Paint::new(cells, area, clip, focused);
        paint(&mut brush);
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

/// Focuses the first node with `auto_focus: true`, once. After that first
/// focus, `auto_focus` on the same node is inert — loom does not steal focus
/// back on every render.
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
