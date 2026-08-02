//! From "what should I look at" to "here is what to draw".
//!
//! ---
//!
//! Admission criterion: is this a step between a request and a drawable buffer?
//! Five of them, in order, one file each:
//!
//! | | file | |
//! |---|---|---|
//! | 1 | [`resolver`] | which file, in which repository |
//! | 2 | [`contents`] | read both sides |
//! | 3 | [`diff`] | call the C engine |
//! | 4 | [`diff`] | pair the lines up |
//! | 5 | [`runner`] | hand over a [`Buffer`], ready to render |
//!
//! [`Buffer`]: ui::Buffer
//!
//! Every stage returns its result. That was briefly untrue — stage four's
//! output borrowed, so stage five had to lend through a closure — and D27
//! records what it cost.
//!
//! This lives in the binary because it is the only crate allowed to name
//! `vcs`, `vscode-diff`, `align` and `ui` together — `cargo xtask
//! lint-arch` forbids those edges everywhere else. A renderer that could
//! assemble its own input would be a renderer that can shell out to git, which
//! is the failure that produced a 674-line `explorer/render.lua` in the plugin.
//!
//! Stages one and two perform IO; everything after them is pure computation
//! over the two texts they produce.
//!
//! ```ignore
//! let runner = pipeline::Runner::new(&request)?;
//! let mut session = ui::Session::new(runner.run()?, theme);
//! ```

pub mod contents;
pub mod diff;
pub mod resolver;
pub mod runner;

pub use resolver::Request;
pub use runner::Runner;
