//! Routing a key, a click or a wheel turn to the node that answers it.
//!
//! The runtime is never borrowed across a listener call, because a listener
//! writes state and reads refs, which reach the runtime themselves.

use crokey::KeyCombination;
use crossterm::event::{MouseEvent, MouseEventKind};
use ratatui::layout::Position;

use super::{Bubble, Focus, Listeners, Mouse, Wheel, hit};
use crate::node::NodeHandle;
use crate::reconcile::RuntimeRef;
use crate::runtime::Runtime;

/// One step of a bubble: which node, what it listens for, and where the
/// pointer was inside it.
struct Step {
    node: NodeHandle,
    listeners: Listeners,
    local: Position,
}

/// The chain from the node an event started at up to the root.
fn chain(held: &RuntimeRef, start: usize, at: Position) -> Vec<Step> {
    let rt = held.borrow();
    hit::upward(&rt, start)
        .into_iter()
        .map(|index| {
            let frame_node = &rt.placed[index];
            Step {
                node: NodeHandle {
                    scope: frame_node.scope,
                    nth: frame_node.nth,
                },
                listeners: frame_node.listeners.clone(),
                local: Position {
                    x: at.x.saturating_sub(frame_node.area.x),
                    y: at.y.saturating_sub(frame_node.area.y),
                },
            }
        })
        .collect()
}

/// Runs one listener with the runtime free, naming the node it belongs to so
/// `capture_pointer` knows what to capture.
fn fire<T>(held: &RuntimeRef, node: NodeHandle, listen: &dyn Fn(T) -> Bubble, event: T) -> Bubble {
    held.borrow_mut().handling = Some(node);
    let bubble = listen(event);
    held.borrow_mut().handling = None;
    bubble
}

/// R8.3 — a key goes to the focused node, then upward.
pub(crate) fn key(held: &RuntimeRef, press: KeyCombination) -> bool {
    let start = {
        let rt = held.borrow();
        match rt.focused {
            Some(node) => rt
                .placed
                .iter()
                .position(|p| p.scope == node.scope && p.nth == node.nth),
            // With nothing focused, a key is offered to every node from
            // the deepest up, the way a browser sends Tab to the first
            // focusable element.
            None => rt
                .placed
                .iter()
                .enumerate()
                .rev()
                .find(|(_, p)| p.listeners.key.is_some())
                .map(|(i, _)| i)
                .or((!rt.placed.is_empty()).then_some(0)),
        }
    };
    let Some(start) = start else { return false };

    for step in chain(held, start, Position { x: 0, y: 0 }) {
        let Some(listen) = step.listeners.key.clone() else {
            continue;
        };
        if fire(held, step.node, &*listen, press) == Bubble::Stop {
            return true;
        }
    }
    false
}

/// R8.1 and R8.4 — hit-test unless the pointer is captured, then bubble.
pub(crate) fn mouse(held: &RuntimeRef, event: MouseEvent) -> bool {
    let at = Position {
        x: event.column,
        y: event.row,
    };

    let start = {
        let rt = held.borrow();
        match rt.captured {
            Some(node) => rt
                .placed
                .iter()
                .position(|p| p.scope == node.scope && p.nth == node.nth),
            None => hit::at(&rt, at),
        }
    };
    let Some(start) = start else { return false };

    let captured = held.borrow().captured.is_some();

    // A click moves focus to the deepest focusable node under the point.
    if matches!(event.kind, MouseEventKind::Down(_)) && !captured {
        let focusable = {
            let rt = held.borrow();
            hit::upward(&rt, start)
                .into_iter()
                .find(|&i| rt.placed[i].focusable)
                .map(|i| NodeHandle {
                    scope: rt.placed[i].scope,
                    nth: rt.placed[i].nth,
                })
        };
        if focusable != held.borrow().focused {
            move_focus(held, focusable);
        }
    }

    let mut stopped = false;
    for step in chain(held, start, at) {
        let bubble = match event.kind {
            MouseEventKind::Down(button) => step.listeners.mouse_down.clone().map(|listen| {
                fire(
                    held,
                    step.node,
                    &*listen,
                    Mouse {
                        button: Some(button),
                        at,
                        local: step.local,
                    },
                )
            }),
            MouseEventKind::Up(_) => step.listeners.mouse_up.clone().map(|listen| {
                fire(
                    held,
                    step.node,
                    &*listen,
                    Mouse {
                        button: None,
                        at,
                        local: step.local,
                    },
                )
            }),
            MouseEventKind::Drag(button) => step.listeners.mouse_move.clone().map(|listen| {
                fire(
                    held,
                    step.node,
                    &*listen,
                    Mouse {
                        button: Some(button),
                        at,
                        local: step.local,
                    },
                )
            }),
            MouseEventKind::Moved => step.listeners.mouse_move.clone().map(|listen| {
                fire(
                    held,
                    step.node,
                    &*listen,
                    Mouse {
                        button: None,
                        at,
                        local: step.local,
                    },
                )
            }),
            MouseEventKind::ScrollDown => step.listeners.wheel.clone().map(|listen| {
                fire(
                    held,
                    step.node,
                    &*listen,
                    Wheel {
                        horizontal: 0,
                        vertical: 1,
                    },
                )
            }),
            MouseEventKind::ScrollUp => step.listeners.wheel.clone().map(|listen| {
                fire(
                    held,
                    step.node,
                    &*listen,
                    Wheel {
                        horizontal: 0,
                        vertical: -1,
                    },
                )
            }),
            MouseEventKind::ScrollLeft => step.listeners.wheel.clone().map(|listen| {
                fire(
                    held,
                    step.node,
                    &*listen,
                    Wheel {
                        horizontal: -1,
                        vertical: 0,
                    },
                )
            }),
            MouseEventKind::ScrollRight => step.listeners.wheel.clone().map(|listen| {
                fire(
                    held,
                    step.node,
                    &*listen,
                    Wheel {
                        horizontal: 1,
                        vertical: 0,
                    },
                )
            }),
        };

        if bubble == Some(Bubble::Stop) {
            stopped = true;
            break;
        }
    }

    // The button coming up ends a capture, whoever answered it.
    if matches!(event.kind, MouseEventKind::Up(_)) {
        held.borrow_mut().captured = None;
    }
    stopped
}

/// R8.2 — the blur fires before the focus, each with the other node.
///
/// Both events bubble to ancestors, like React's `onFocus` / `onBlur`.
pub(crate) fn move_focus(held: &RuntimeRef, to: Option<NodeHandle>) {
    let from = held.borrow().focused;
    if from == to {
        return;
    }

    for node in ancestry(held, from) {
        let listen = held
            .borrow()
            .listeners_of(node)
            .and_then(|l| l.blur.clone());
        if let Some(listen) = listen
            && fire(held, node, &*listen, Focus { related: to }) == Bubble::Stop
        {
            break;
        }
    }

    held.borrow_mut().focused = to;

    for node in ancestry(held, to) {
        let listen = held
            .borrow()
            .listeners_of(node)
            .and_then(|l| l.focus.clone());
        if let Some(listen) = listen
            && fire(held, node, &*listen, Focus { related: from }) == Bubble::Stop
        {
            break;
        }
    }

    let root = held.borrow().root;
    if let Some(root) = root {
        held.borrow_mut().mark(root);
    }
}

/// A node and every ancestor above it, nearest first.
fn ancestry(held: &RuntimeRef, node: Option<NodeHandle>) -> Vec<NodeHandle> {
    let Some(node) = node else { return Vec::new() };
    let rt = held.borrow();
    let Some(start) = rt
        .placed
        .iter()
        .position(|p| p.scope == node.scope && p.nth == node.nth)
    else {
        return Vec::new();
    };
    hit::upward(&rt, start)
        .into_iter()
        .map(|i| NodeHandle {
            scope: rt.placed[i].scope,
            nth: rt.placed[i].nth,
        })
        .collect()
}

/// Focus order is paint order, and it wraps.
pub(crate) fn step_focus(held: &RuntimeRef, by: i32) {
    let order: Vec<NodeHandle> = {
        let rt = held.borrow();
        rt.placed
            .iter()
            .filter(|p| p.focusable)
            .map(|p| NodeHandle {
                scope: p.scope,
                nth: p.nth,
            })
            .collect()
    };
    if order.is_empty() {
        return;
    }

    let at = held
        .borrow()
        .focused
        .and_then(|f| order.iter().position(|&n| n == f));
    let next = match at {
        Some(at) => {
            let len = order.len() as i32;
            order[(at as i32 + by).rem_euclid(len) as usize]
        }
        None => order[if by >= 0 { 0 } else { order.len() - 1 }],
    };
    move_focus(held, Some(next));
}

/// Whether a node is still mounted and focusable. Used by `NodeHandle::focus`.
pub(crate) fn focusable(rt: &Runtime, node: NodeHandle) -> bool {
    rt.placed
        .iter()
        .any(|p| p.scope == node.scope && p.nth == node.nth && p.focusable)
}
