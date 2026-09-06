//! Measuring a node's laid-out size.

use ratatui::layout::Rect;

use super::reference::Ref;
use crate::node::NodeHandle;
use crate::scope::Scope;

/// The size of a laid-out node. Updated after each layout pass.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Size {
    pub width: u16,
    pub height: u16,
}

/// Returns a host ref and its last measured size.
///
/// The size is zero before the first layout pass.
pub fn use_measure(scope: &mut Scope) -> (Ref<Option<NodeHandle>>, Size) {
    let node_ref = super::reference::use_ref(scope, || None::<NodeHandle>);
    let (size, set_size) = super::state::use_state(scope, Size::default);
    super::effect::use_layout_effect(scope, super::effect::Always, move || {
        let area = node_ref.current().as_ref().map_or(Rect::ZERO, |n| n.area());
        let measured = Size {
            width: area.width,
            height: area.height,
        };
        set_size(&move |_| measured);
    });
    (node_ref, size)
}
