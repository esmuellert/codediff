#![doc = include_str!("../README.md")]

// Both appear in this crate's public API — `Session::draw` takes a ratatui
// terminal and `Session::handle` a crossterm event — so a consumer needs the
// same versions we were built against.
pub use crossterm;
pub use ratatui;

mod app;
mod cells;
pub mod screen_map;
pub mod components;

pub use screen_map::ScreenMap;
pub mod input;
pub mod state;
mod start;
mod terminal;
pub mod theme;

pub use app::event::Event;
pub use app::{Exit, Flow, Session, Workers, run};
pub use start::start;
pub use terminal::{Screen, restore};
pub use theme::{Flavour, Rgb, Theme, blend, catppuccin};
pub use state::buffer::{Buffer, BufferType, Inline, SideBySide, SingleFile};
pub use state::{View, Viewport};
pub mod testing;
