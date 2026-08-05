#![doc = include_str!("../README.md")]
//!
//! ---
//!
//! Admission criterion: does this identify what a piece of text *is* — a
//! keyword, a string, a comment? Never what colour it should be. `ui` chooses
//! the colours and hands them in as [`Rule`]s; this crate matches them against
//! what it finds, so the engine can be replaced without touching a theme.
//!
//! This crate performs no IO: the grammars are compiled in.

mod detect;
pub mod engine;
mod highlighted;
pub mod limits;
mod style;

pub use detect::Clues;
pub use engine::{Engine, Grammar, Palette};
pub use highlighted::Highlighted;
pub use style::{Capture, Pen, Rule, Span, Style, coalesce};
