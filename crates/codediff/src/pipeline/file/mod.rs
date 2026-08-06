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
//! | 4 | [`runner`] | hand over a [`Buffer`], ready to render |
//!
//! There were five. The first searched git for a file by path, which is the
//! list pipeline written again — and worse, because searching cannot know
//! which comparison the reader chose. It answered `HEAD → worktree` for every
//! file, so one path had three different diffs depending on how it was
//! reached. See D58.
//!
//! [`Buffer`]: ui::Buffer
//!
//! Every stage returns its result. That was briefly untrue — the pairing's
//! output borrowed, so the stage after it had to lend through a closure — and
//! D27 records what it cost.
//!
//! Stage one performs IO; everything after it is pure computation over the two
//! texts it produces.
//!
//! ```ignore
//! let runner = file::Runner::new(&file)?;
//! let mut session = ui::Session::new(runner.run()?, theme);
//! ```

pub mod contents;
pub mod diff;
pub mod runner;

pub use runner::Runner;

use anyhow::Result;
use file_types::ChangedFile;
use ui::Buffer;

/// Runs every stage, for a file the interface asked for.
///
/// What `ui` asks for and cannot do itself: `cargo xtask lint-arch` forbids a
/// renderer from reaching git, so the interface names the request and this
/// answers it. Handed to [`ui::run`] as a function, because it needs no state
/// — see [`ui::Open`].
///
/// **Nothing is kept between calls.** Three of the four revisions a row can
/// name — the working tree, the index, and a conflict stage — are mutable:
/// their bytes change while the review is open, and nothing in their name
/// changes with them. A cache keyed by those names cannot tell a re-read from
/// a stale one. Reading two versions and pairing them takes milliseconds,
/// which is the whole cost of getting this right. See D51.
pub fn open(wanted: &ChangedFile) -> Result<Buffer, String> {
    let path = wanted.path().as_str().to_owned();
    let runner = Runner::new(wanted).map_err(|why| format!("{path}: {why:#}"))?;
    if runner.is_binary() {
        return Err(format!("{path} is binary — there are no lines to review"));
    }
    runner.run().map_err(|why| format!("{path}: {why:#}"))
}
