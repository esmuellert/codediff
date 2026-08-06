#![doc = include_str!("../README.md")]
//!
//! ---
//!
//! Admission criterion: does this decide what appears on the screen, or what a
//! keypress means? Presentation only — this crate cannot see a repository, and
//! `cargo xtask lint-arch` fails if it tries.
//!
//! Three structural commitments worth stating up front.
//!
//! [`View`] is four nested levels — tabs, panes, buffers, viewports — and an
//! action is executed by the lowest one that contains everything it affects.
//! That is what decides where each piece of behaviour lives.
//!
//! [`Viewport`] holds a single scroll position for every column of a pane, so
//! two sides of a diff cannot drift apart and there is no synchronisation code
//! to get wrong.
//!
//! And a key resolves to a [`Command`] that is one of exactly three kinds —
//! executed by a buffer, by the terminal's owner, or off-thread — so the loop
//! always knows whether it may block.
//!
//! [`Command`]: input::Command
//! [`View`]: view::View

// Both appear in this crate's public API — `Session::draw` takes a ratatui
// terminal and `Session::handle` a crossterm event — so a consumer needs the
// same versions we were built against, and getting them from here is the only
// way to be sure of that.
pub use crossterm;
pub use ratatui;

mod app;
mod diff;
mod draw;
pub mod input;
mod render;
mod syntax;
mod terminal;
pub mod theme;
pub mod view;

pub use align::DiffLayout;
pub use app::{Flow, Open, Session, run};
pub use diff::Diff;
pub use terminal::{Screen, restore};
pub use theme::{Flavour, Rgb, Theme, blend, catppuccin};
pub use view::buffer::{Buffer, BufferType, Inline, SideBySide, SingleFile};
pub use view::{View, Viewport};
