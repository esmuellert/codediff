//! The runtime the application owns.

use std::cell::RefCell;
use std::rc::Rc;

use ratatui::buffer::Buffer as Cells;
use ratatui::layout::Rect;

use crate::component::Component;
use crate::runtime::Runtime;
use crate::scope::ScopeId;

/// The runtime the application owns. Single-threaded by construction: every
/// handle into it holds an `Rc` or reaches it through a thread-local.
pub struct Tree {
    runtime: Rc<RefCell<Runtime>>,
    area: Rect,
}

impl Tree {
    pub fn new<C: Component>(props: C::Props) -> Self {
        let runtime = Rc::new(RefCell::new(Runtime::new()));
        let root = crate::current::enter(&runtime, || {
            let mut rt = runtime.borrow_mut();
            let id = rt.mount(
                C::NAME,
                std::any::TypeId::of::<C>(),
                None,
                None,
                Rc::new(props),
                |props, scope| {
                    let props = props.downcast_ref::<C::Props>().expect("props of the declared type");
                    C::render(props, scope)
                },
                None,
            );
            rt.root = Some(id);
            rt.mark(id);
            id
        });
        let _: ScopeId = root;
        Self { runtime, area: Rect::ZERO }
    }

    /// Replaces the root's props and redraws it.
    pub fn set_props<C: Component>(&mut self, props: C::Props) {
        crate::current::enter(&self.runtime, || {
            let mut rt = self.runtime.borrow_mut();
            let Some(root) = rt.root else { return };
            if let Some(mounted) = rt.scopes.get_mut(root) {
                mounted.props = Rc::new(props);
            }
            rt.mark(root);
        });
    }

    /// Reconcile, lay out, paint, run effects. The only entry point that
    /// writes cells.
    pub fn draw(&mut self, cells: &mut Cells, area: Rect) {
        if area != self.area {
            self.area = area;
            self.redraw_all();
        }
        let runtime = Rc::clone(&self.runtime);
        crate::current::enter(&runtime, || crate::frame::draw(&runtime, cells, area));
    }

    /// Routes a key to the focused scope, then upward. Returns whether one
    /// listener stopped it.
    pub fn press(&mut self, key: crokey::KeyCombination) -> bool {
        let runtime = Rc::clone(&self.runtime);
        crate::current::enter(&runtime, || crate::event::route_key(&runtime, key))
    }

    pub fn mouse(&mut self, event: crossterm::event::MouseEvent) -> bool {
        let runtime = Rc::clone(&self.runtime);
        crate::current::enter(&runtime, || crate::event::route_mouse(&runtime, event))
    }

    /// Whether anything has been marked for redraw since the last `draw`.
    pub fn needs_draw(&self) -> bool {
        self.runtime.borrow().needs_draw()
    }

    /// How many render-and-layout rounds the last `draw` took. One, unless a
    /// layout effect wrote state.
    pub fn layout_rounds(&self) -> usize {
        self.runtime.borrow().rounds
    }

    /// Mark everything. What a terminal resize does.
    pub fn redraw_all(&mut self) {
        let runtime = Rc::clone(&self.runtime);
        crate::current::enter(&runtime, || runtime.borrow_mut().mark_all());
    }

    pub fn focused_scope(&self) -> Option<ScopeId> {
        self.runtime.borrow().focused.map(|node| node.scope)
    }

    pub(crate) fn runtime(&self) -> &Rc<RefCell<Runtime>> {
        &self.runtime
    }
}
