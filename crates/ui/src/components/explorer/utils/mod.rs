//! Pure functions over paths.
//!
//! ```text
//! filter.rs  glob filtering
//! order.rs   sort order
//! ```
//!
//! Nothing here holds state: each takes a path and returns an answer, so the
//! model can call them from wherever it happens to need them.

pub mod filter;
pub mod order;
