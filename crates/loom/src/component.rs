//! What a component is.

use crate::node::Node;
use crate::scope::Scope;

/// A function component: props in, one frame's description out.
pub trait Component: 'static {
    type Props: 'static;
    const NAME: &'static str;
    fn render(props: &Self::Props, scope: &mut Scope) -> Node;
}
