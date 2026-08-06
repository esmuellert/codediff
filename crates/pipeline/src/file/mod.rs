//! One request for one file, to something the interface can draw.
//!
//! ---
//!
//! Admission criterion: is this a step between one file and its buffer? Four
//! of them, in order:
//!
//! | | file | |
//! |---|---|---|
//! | 1 | [`contents`] | read both sides |
//! | 2 | [`diff`] | call the C engine |
//! | 3 | [`diff`] | pair the lines up |
//! | 4 | [`runner`] | hand over a [`DiffContent`], ready to draw |
//!
//! There were five. The first searched git for a file by path, which is the
//! list pipeline written again — and worse, because searching cannot know
//! which comparison the reader chose. It answered `HEAD → worktree` for every
//! file, so one path had three different diffs depending on how it was
//! reached. See D58.
//!
//! [`DiffContent`]: runner::DiffContent
//!
//! Every stage returns its result. That was briefly untrue — the pairing's
//! output borrowed, so the stage after it had to lend through a closure — and
//! D27 records what it cost.
//!
//! Stage one performs IO; everything after it is pure computation over the two
//! texts it produces. All four run on a thread of their own — see [`service`],
//! which is not a stage but the thing that runs them.
//!
//! ```ignore
//! let mut files = file::Files::start();
//! files.want(&changed);          // returns at once
//! let answer = files.take();     // next frame, or the one after
//! ```
//!
pub mod contents;
pub mod diff;
pub mod runner;
pub mod service;

pub use runner::{DiffContent, Runner};
pub use service::{Answer, Files};
