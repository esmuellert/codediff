//! The command tree.

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "codediff",
    version,
    about = "A standalone, read-only terminal diff reviewer",
    disable_help_subcommand = true
)]
pub struct Cli {
    /// Narrow to one file (used by tests, not advertised).
    #[arg(hide = true)]
    pub path: Option<String>,

    /// Write debug logs to this file.
    #[arg(long, hide = true, value_name = "PATH")]
    pub log: Option<std::path::PathBuf>,

    /// Panic after taking over the terminal (for testing terminal restore).
    #[arg(long, hide = true)]
    pub self_panic: bool,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Subcommand)]
pub enum Command {
    /// Report how this binary was built, and what it found
    Doctor,

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

    /// Print the groups a request produces
    List {
        #[arg(long = "rev", short = 'r', value_name = "REV", num_args = 1..=2)]
        rev: Vec<String>,
        #[arg(long, visible_alias = "cached")]
        staged: bool,
        /// Paths to narrow the list to
        #[arg(last = true)]
        pathspec: Vec<String>,
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
