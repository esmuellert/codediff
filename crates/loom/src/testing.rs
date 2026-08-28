//! One component, one screen, no terminal.

use std::rc::Rc;

use ratatui::buffer::Buffer as Cells;
use ratatui::layout::Rect;
use ratatui::style::Style;

use crate::component::Component;
use crate::hook::Context;
use crate::tree::Tree;

/// One component, one screen, no terminal.
pub struct Harness {
    tree: Tree,
    cells: Cells,
    area: Rect,
}

impl Harness {
    /// Mounts `C` at `width` × `height`. Does not draw.
    pub fn new<C: Component>(props: C::Props, width: u16, height: u16) -> Self {
        let area = Rect { x: 0, y: 0, width, height };
        Self { tree: Tree::new::<C>(props), cells: Cells::empty(area), area }
    }

    /// Provides a context value above the root, for a component that reads one.
    pub fn provide<C: Context>(self, value: C::Value) -> Self {
        let runtime = Rc::clone(self.tree.runtime());
        crate::current::enter(&runtime, || {
            let mut rt = runtime.borrow_mut();
            let Some(root) = rt.root else { return };
            let version = rt.context_version + 1;
            rt.context_version = version;
            rt.offers.entry(root).or_default().push(crate::runtime::Offer {
                context: std::any::TypeId::of::<C>(),
                value: Rc::new(value),
                version,
            });
        });
        self
    }

    /// Replaces the root's props.
    pub fn set_props<C: Component>(&mut self, props: C::Props) -> &mut Self {
        self.tree.set_props::<C>(props);
        self
    }

    /// Draws if anything is marked. Idempotent.
    pub fn draw(&mut self) -> &mut Self {
        if self.tree.needs_draw() {
            self.force_draw();
        }
        self
    }

    /// Draws whether or not anything is marked.
    pub fn force_draw(&mut self) -> &mut Self {
        self.cells = Cells::empty(self.area);
        self.tree.draw(&mut self.cells, self.area);
        self
    }

    /// The screen as text, one string per row, trailing blanks trimmed.
    pub fn screen(&mut self) -> Vec<String> {
        self.draw();
        (0..self.area.height).map(|y| self.row(y)).collect()
    }

    pub fn screen_row(&mut self, y: u16) -> String {
        self.draw();
        self.row(y)
    }

    fn row(&self, y: u16) -> String {
        let text: String = (0..self.area.width)
            .filter_map(|x| self.cells.cell((x, y)))
            .map(|cell| cell.symbol())
            .collect();
        text.trim_end().to_string()
    }

    pub fn style_at(&mut self, x: u16, y: u16) -> Style {
        self.draw();
        self.cells.cell((x, y)).map(|cell| cell.style()).unwrap_or_default()
    }

    pub fn cells(&mut self) -> &Cells {
        self.draw();
        &self.cells
    }

    pub fn press(&mut self, key: crokey::KeyCombination) -> &mut Self {
        self.tree.press(key);
        self
    }

    /// Whether a component has asked the loop to stop.
    pub fn exiting(&self) -> bool {
        self.tree.exiting()
    }

    pub fn click(&mut self, x: u16, y: u16) -> &mut Self {
        self.mouse(x, y, crossterm::event::MouseEventKind::Down(crossterm::event::MouseButton::Left))
    }

    pub fn drag(&mut self, x: u16, y: u16) -> &mut Self {
        self.mouse(x, y, crossterm::event::MouseEventKind::Drag(crossterm::event::MouseButton::Left))
    }

    pub fn release(&mut self, x: u16, y: u16) -> &mut Self {
        self.mouse(x, y, crossterm::event::MouseEventKind::Up(crossterm::event::MouseButton::Left))
    }

    pub fn wheel(&mut self, x: u16, y: u16, lines: i32) -> &mut Self {
        let kind = if lines >= 0 {
            crossterm::event::MouseEventKind::ScrollDown
        } else {
            crossterm::event::MouseEventKind::ScrollUp
        };
        self.mouse(x, y, kind)
    }

    fn mouse(&mut self, x: u16, y: u16, kind: crossterm::event::MouseEventKind) -> &mut Self {
        self.tree.mouse(crossterm::event::MouseEvent {
            kind,
            column: x,
            row: y,
            modifiers: crossterm::event::KeyModifiers::NONE,
        });
        self
    }

    pub fn resize(&mut self, width: u16, height: u16) -> &mut Self {
        self.area = Rect { x: 0, y: 0, width, height };
        self.cells = Cells::empty(self.area);
        self.force_draw()
    }

    /// The scope tree as indented text: name, key, rectangle.
    pub fn tree_text(&mut self) -> String {
        self.draw();
        let runtime = self.tree.runtime().borrow();
        let mut out = String::new();
        for placed in &runtime.placed {
            let depth = {
                let mut depth = 0;
                let mut up = placed.parent;
                while let Some(i) = up {
                    depth += 1;
                    up = runtime.placed[i].parent;
                }
                depth
            };
            out.push_str(&"  ".repeat(depth));
            out.push_str(placed.host_desc.name);
            out.push_str(&format!(
                " {}x{}+{}+{}\n",
                placed.area.width, placed.area.height, placed.area.x, placed.area.y
            ));
        }
        out
    }

    /// The rectangle of the first scope with this component name.
    pub fn area_of(&self, name: &str) -> Option<Rect> {
        let runtime = self.tree.runtime().borrow();
        runtime
            .placed
            .iter()
            .find(|p| runtime.name_of(p.scope) == name || p.host_desc.name == name)
            .map(|p| p.area)
    }

    /// How many times a component of this name has run since the harness was
    /// built.
    pub fn render_count_of(&self, name: &str) -> usize {
        self.tree.runtime().borrow().renders_by_name.get(name).copied().unwrap_or(0)
    }

    /// How many component functions ran during the last `draw`.
    pub fn render_count(&self) -> usize {
        self.tree.runtime().borrow().renders
    }

    pub fn layout_rounds(&self) -> usize {
        self.tree.layout_rounds()
    }

    pub fn focused_name(&self) -> Option<&'static str> {
        let runtime = self.tree.runtime().borrow();
        runtime.focused.map(|node| runtime.name_of(node.scope))
    }

    pub fn needs_draw(&self) -> bool {
        self.tree.needs_draw()
    }
}

/// A component that renders nothing and counts its renders. For tests about
/// identity that do not want a real component.
pub struct Probe;

#[derive(PartialEq, Eq)]
pub struct ProbeProps {
    pub tag: u32,
}

impl Component for Probe {
    type Props = ProbeProps;
    const NAME: &'static str = "Probe";
    fn render(_: &Self::Props, _: &mut crate::scope::Scope) -> crate::node::Node {
        crate::node::Node::Empty
    }
}
