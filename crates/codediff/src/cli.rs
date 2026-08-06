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
    after_help = "With no arguments this lists every changed file. Name one or two revisions to\ncompare those instead, or a file to review just that one.",
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

    /// Compare against this revision, or between two of them
    ///
    /// One name compares it against the file on disk; two compare them with
    /// each other. `a...b` compares `b` against where it parted from `a`,
    /// which is git's own spelling.
    #[arg(long = "rev", short = 'r', value_name = "REV", num_args = 1..=2)]
    pub rev: Vec<String>,

    /// List what is staged, against a revision
    #[arg(long, visible_alias = "cached")]
    pub staged: bool,

    #[command(subcommand)]
    pub command: Option<Command>,
}

impl Cli {
    /// What the reader asked the explorer to compare.
    ///
    /// The one place the command line becomes a diff type. Adding a way to
    /// compare is an arm here and an arm in the list pipeline's resolver, and
    /// nothing between them.
    pub fn diff_type(&self) -> explorer::ExplorerDiffType {
        diff_type(&self.rev, self.staged)
    }
}

/// The reader's words, as a diff type.
///
/// A free function because two callers share it: the review itself and
/// `debug list`, which exists so a test can see the groups without a
/// terminal. Adding a way to compare is an arm here and an arm in the list
/// pipeline's resolver, and nothing between them.
pub fn diff_type(rev: &[String], staged: bool) -> explorer::ExplorerDiffType {
    use explorer::ExplorerDiffType as Type;
    match (staged, rev.first().cloned(), rev.get(1).cloned()) {
        // `--staged` with no revision means against the last commit, which is
        // what `git diff --cached` means with no revision.
        (true, rev, _) => Type::Staged(rev.unwrap_or_else(|| "HEAD".to_owned())),
        (false, None, _) => Type::Worktree,
        (false, Some(a), Some(b)) => Type::Between(a, b),
        (false, Some(a), None) => match a.split_once("...") {
            Some((base, target)) => Type::MergeBase(base.to_owned(), target.to_owned()),
            None => Type::Against(a),
        },
    }
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
