#![doc = include_str!("../README.md")]
//!
//! ---
//!
//!
//! This crate performs no IO: the grammars are compiled in.

mod detect;
pub mod engine;
mod group;
mod highlighted;
pub mod limits;
mod style;
pub mod worker;

pub use detect::Clues;
pub use engine::{Engine, Grammar, Palette, group, rules};
pub use group::Group;
pub use highlighted::Highlighted;
pub use style::{Capture, Pen, Rule, Span, Style, coalesce};
pub use worker::{Colours, Spans, Store, Syntax, SyntaxRequest, SyntaxResponse, Version, path_of};
