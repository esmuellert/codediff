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
mod show;
mod status;

pub use align::print as print_alignment;

use anyhow::{Result, bail};

/// Runs a debug command. `args` begins at the command name.
pub fn run(args: &[String]) -> Result<()> {
    let verbose = args.iter().any(|a| a == "--verbose" || a == "-v");
    let arg = |n: usize| args.get(n).map(String::as_str);

    match arg(0) {
        Some("diff") => match (arg(1), arg(2)) {
            (Some(original), Some(modified)) => diff::run(original, modified),
            _ => bail!("usage: codediff debug diff <original> <modified>"),
        },
        Some("diff-file") => match arg(1) {
            Some(path) => diff_file::run(path, verbose),
            None => bail!("usage: codediff debug diff-file <path> [--verbose]"),
        },
        Some("align") => match (arg(1), arg(2)) {
            (Some(original), Some(modified)) => align::run(original, modified, verbose),
            _ => bail!("usage: codediff debug align <original> <modified> [--verbose]"),
        },
        Some("line") => match arg(1) {
            Some(path) => line::run(path, verbose),
            None => bail!("usage: codediff debug line <file> [--verbose]"),
        },
        Some("show") => match arg(1) {
            Some(spec) => show::run(spec, args.iter().any(|a| a == "--raw")),
            None => bail!("usage: codediff debug show <rev>:<path> [--raw]"),
        },
        Some("status") => status::run(arg(1).unwrap_or("."), verbose),
        Some(other) => {
            help();
            bail!("unknown debug command: {other}");
        }
        None => {
            help();
            Ok(())
        }
    }
}

fn help() {
    eprintln!(
        "\
codediff debug <command> — inspect one layer at a time

    diff <old> <new>            the raw diff of two files
    diff-file <path>            one file of this repository, end to end
    align <old> <new> [-v]      the two files paired up, with fillers
    line <file> [-v]            where each character sits
    show <rev>:<path> [--raw]   a file as of a revision
    status [dir] [-v]           what git says about a worktree

-v adds detail; --raw writes exact bytes, for comparing against git."
    );
}
