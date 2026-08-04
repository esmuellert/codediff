//! The seam.
//!
//! Everything above this module is written against [`Span`](crate::Span),
//! [`Style`](crate::Style) and [`Rule`](crate::Rule); everything below it
//! knows what a TextMate grammar is. `cargo xtask lint-arch` refuses the name
//! of a syntax engine anywhere outside this directory, which is what makes the
//! claim checkable rather than merely stated.
//!
//! One engine today. A second would be a sibling file and a choice here, and
//! nothing above would move — see D17.

mod syntect;

pub use syntect::{Engine, Grammar, Palette, Reading};
