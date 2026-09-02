//! Where each node landed, and what is under a point.

use ratatui::layout::Position;

use crate::runtime::Runtime;

/// The deepest node whose rectangle holds `at`, and its ancestors after it.
///
/// `placed` is deepest last, so the last match is the deepest node.
pub(crate) fn at(rt: &Runtime, at: Position) -> Option<usize> {
    rt.placed
        .iter()
        .enumerate()
        .rev()
        .find(|(_, p)| p.area.contains(at) && p.clip.contains(at))
        .map(|(i, _)| i)
}

/// The chain from a node up to the root, nearest first. What bubbling walks.
pub(crate) fn upward(rt: &Runtime, from: usize) -> Vec<usize> {
    let mut chain = vec![from];
    let mut up = rt.placed[from].parent;
    while let Some(i) = up {
        chain.push(i);
        up = rt.placed[i].parent;
    }
    chain
}
