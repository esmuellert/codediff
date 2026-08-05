//! The command tree.
//!
//! Declared with `clap`'s derive so that flag parsing, `--help`, "did you
//! mean" and shell completions all come from one description rather than from
//! six hand-written `match` arms that had to agree with each other.

use clap::builder::PossibleValuesParser;
use clap::{Parser, Subcommand};

/// The themes `--theme` accepts, taken from `ui` rather than repeated
/// here, so a new theme appears in `--help` and in tab completion without
/// anything else being edited.
fn themes() -> PossibleValuesParser {
    PossibleValuesParser::new(ui::Theme::NAMES)
}

#[derive(Parser)]
#[command(
    name = "codediff",
    version,
    about = "A standalone, read-only terminal diff reviewer",
    after_help = "With no arguments this will open the explorer, listing every changed file. That\nis not built yet; until then, name a file. See docs/plan/04-milestones.md.",
    disable_help_subcommand = true
)]
pub struct Cli {
    /// File to review, as given or relative to the repository root
    ///
    /// An argument rather than a subcommand, following the Neovim plugin,
    /// where `:CodeDiff` *is* the diff and arguments say what to compare.
    pub path: Option<String>,

    /// Colours to draw with
    ///
    /// Defaults to Catppuccin Mocha, or `basic-dark` on a terminal that does
    /// not advertise 24-bit colour, where Catppuccin's diff backgrounds would
    /// round into the background and vanish.
    #[arg(long, value_name = "NAME", value_parser = themes())]
    pub theme: Option<String>,

    /// Take over the terminal and then panic, to check it is given back
    ///
    /// The one failure mode a diff reviewer must never have is leaving the
    /// shell with no echo and an invisible cursor. Hidden because it is for
    /// the test suite, not for people.
    #[arg(long, hide = true)]
    pub self_panic: bool,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand)]
pub enum Command {
    /// Report how this binary was built, and what it found
    Doctor,

    // Hidden from the main help for the reason git's plumbing is hidden from
    // `git --help`: these exist for bug reports and for driving each crate
    // from outside, not for daily use. `codediff debug` lists them.
    /// Inspect one layer at a time
    #[command(subcommand, hide = true)]
    Debug(Debug),
}

#[derive(Subcommand)]
pub enum Debug {
    /// Print the raw diff of two files, as the engine reports it
    Diff { original: String, modified: String },

    /// Diff one file of this repository, end to end
    DiffFile {
        path: String,
        /// Also print hunks, character spans and unchanged regions
        #[arg(short, long)]
        verbose: bool,
    },

    /// Print two files paired up, with fillers where lines were added or removed
    Align {
        original: String,
        modified: String,
        #[arg(short, long)]
        verbose: bool,
    },

    /// Print everything drawn, in a form a machine can diff
    ///
    /// For the harness that checks this against `codediff.nvim`; the other
    /// commands here are for a reader.
    Parity { original: String, modified: String },

    /// Print where each character of a line sits
    Line {
        file: String,
        /// List every character, not only those whose positions disagree
        #[arg(short, long)]
        verbose: bool,
    },

    /// Print a file as of a revision
    Show {
        /// `<rev>:<path>`, as `git show` spells it
        spec: String,
        /// Write the exact bytes to stdout, for comparing against `git show`
        #[arg(long)]
        raw: bool,
    },

    /// Print what git says about a worktree
    Status {
        /// Defaults to the current directory
        #[arg(default_value = ".")]
        dir: String,
        /// Also print the same entries as the reviewer sees them
        #[arg(short, long)]
        verbose: bool,
    },
}
