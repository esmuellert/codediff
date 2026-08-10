//! The explorer, drawn against a real screen.
//!
//! One test target rather than five, because the fixtures in `common` are
//! shared: a helper only two of the files call is still reachable from here,
//! where in a target of its own it would look unused.

mod common;

mod colours;
mod mouse;
mod panes;
mod rows;
mod selection;
