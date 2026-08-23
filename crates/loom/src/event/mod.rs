//! Listeners, hit-testing, focus, and where an event goes.

mod hit;
mod route;

pub(crate) use route::{focusable, key as route_key, mouse as route_mouse, move_focus};

use std::rc::Rc;

use crokey::KeyCombination;
use crossterm::event::MouseButton;
use ratatui::layout::Position;

use crate::node::NodeHandle;

/// What a listener says about an event it was given.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Bubble {
    /// Dealt with. Nothing above sees it.
    Stop,
    /// Not mine. Offer it to my parent.
    Continue,
}

pub struct Mouse {
    /// Which button is down, `None` when none is. On a move this is what
    /// separates a drag from a plain move.
    pub button: Option<MouseButton>,
    /// Where on the screen.
    pub at: Position,
    /// Where within this node's rectangle.
    pub local: Position,
}

pub struct Focus {
    /// The node on the other side of the move: the one losing focus in an
    /// `on_focus`, the one gaining it in an `on_blur`.
    pub related: Option<NodeHandle>,
}

/// Every listener one host can carry.
#[derive(Clone, Default)]
pub struct Listeners {
    pub(crate) key: Option<Rc<dyn Fn(KeyCombination) -> Bubble>>,
    pub(crate) mouse_down: Option<Rc<dyn Fn(Mouse) -> Bubble>>,
    pub(crate) mouse_move: Option<Rc<dyn Fn(Mouse) -> Bubble>>,
    pub(crate) mouse_up: Option<Rc<dyn Fn(Mouse) -> Bubble>>,
    pub(crate) wheel: Option<Rc<dyn Fn(i32) -> Bubble>>,
    pub(crate) focus: Option<Rc<dyn Fn(Focus) -> Bubble>>,
    pub(crate) blur: Option<Rc<dyn Fn(Focus) -> Bubble>>,
}

impl Listeners {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn on_key(mut self, listen: impl Fn(KeyCombination) -> Bubble + 'static) -> Self {
        self.key = Some(Rc::new(listen));
        self
    }
    pub fn on_mouse_down(mut self, listen: impl Fn(Mouse) -> Bubble + 'static) -> Self {
        self.mouse_down = Some(Rc::new(listen));
        self
    }
    /// Fires with a button held or without one; `Mouse::button` is the
    /// difference between a drag and a plain move.
    pub fn on_mouse_move(mut self, listen: impl Fn(Mouse) -> Bubble + 'static) -> Self {
        self.mouse_move = Some(Rc::new(listen));
        self
    }
    pub fn on_mouse_up(mut self, listen: impl Fn(Mouse) -> Bubble + 'static) -> Self {
        self.mouse_up = Some(Rc::new(listen));
        self
    }
    /// Positive is down.
    pub fn on_wheel(mut self, listen: impl Fn(i32) -> Bubble + 'static) -> Self {
        self.wheel = Some(Rc::new(listen));
        self
    }
    /// Focus arrived, at this scope or at one inside it.
    pub fn on_focus(mut self, listen: impl Fn(Focus) -> Bubble + 'static) -> Self {
        self.focus = Some(Rc::new(listen));
        self
    }
    /// Focus left, from this scope or from one inside it.
    pub fn on_blur(mut self, listen: impl Fn(Focus) -> Bubble + 'static) -> Self {
        self.blur = Some(Rc::new(listen));
        self
    }

}
/// Move focus to the next focusable node in paint order, wrapping. No-ops
/// when nothing is focusable.
pub fn focus_next() {
    if let Some(held) = crate::current::held() {
        route::step_focus(&held, 1);
    }
}

pub fn focus_previous() {
    if let Some(held) = crate::current::held() {
        route::step_focus(&held, -1);
    }
}

/// Route every mouse event to this node until the button comes up or
/// `release_pointer` is called. Called from `on_mouse_down`.
pub fn capture_pointer() {
    crate::current::with_mut(|rt| {
        rt.captured = rt.handling;
    });
}

pub fn release_pointer() {
    crate::current::with_mut(|rt| {
        rt.captured = None;
    });
}
