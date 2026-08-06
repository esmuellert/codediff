//! `codediff debug <command>` — one command per layer, printed as text.
//!
//! These ship. They are not scaffolding:
//!
//! - a bug report becomes "send me `codediff debug align` output";
//! - the golden tests run these commands against the built binary, so what is
//!   tested is what ships;
//! - each one drives a single crate from outside, which is a standing check
//!   that the layering holds. If `debug align` ever cannot be written without
//!   reaching for git, the architecture has already broken.
//!
//! They are absent from `codediff --help` for the same reason git's plumbing
//! is absent from `git --help`: `codediff debug` lists them.

mod align;
mod diff;
mod diff_file;
mod line;
mod list;
mod parity;
mod show;
mod status;

pub use align::print as print_alignment;

use anyhow::Result;

use crate::cli::Debug;

/// Runs a debug command.
pub fn run(command: Debug) -> Result<()> {
    match command {
        Debug::Diff { original, modified } => diff::run(&original, &modified),
        Debug::DiffFile { path, verbose } => diff_file::run(&path, verbose),
        Debug::Align {
            original,
            modified,
            verbose,
        } => align::run(&original, &modified, verbose),
        Debug::Line { file, verbose } => line::run(&file, verbose),
        Debug::Parity { original, modified } => parity::run(&original, &modified),
        Debug::Show { spec, raw } => show::run(&spec, raw),
        Debug::List {
            rev,
            staged,
            pathspec,
        } => list::run(list::diff_type(&rev, staged), pathspec),
        Debug::Status { dir, verbose } => status::run(&dir, verbose),
    }
}
