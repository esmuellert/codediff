#![doc = include_str!("../README.md")]

// Both appear in this crate's public API, so a consumer needs the same
// versions we were built against.
pub use crossterm;
pub use ratatui;

pub mod components;
pub mod theme;

pub use theme::{Flavour, Rgb, Theme, blend, catppuccin};
