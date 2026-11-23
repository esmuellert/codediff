//! The command tree.

use clap::{Parser, Subcommand, ValueEnum};
use vcs::DiffType;

#[derive(Parser)]
#[command(
    name = "codediff",
    version,
    about = "A standalone terminal diff reviewer",
    disable_help_subcommand = true,
    args_conflicts_with_subcommands = true,
    subcommand_precedence_over_arg = true
)]
pub struct Cli {
    /// Load user preferences from this file.
    #[arg(long, value_name = "PATH")]
    pub config: Option<std::path::PathBuf>,

    /// Write debug logs to this file.
    #[arg(long, hide = true, value_name = "PATH")]
    pub log: Option<std::path::PathBuf>,

    /// Panic after taking over the terminal (for testing terminal restore).
    #[arg(long, hide = true)]
    pub self_panic: bool,

    /// Revision or range to review (e.g. `main`, `a..b`, `main...HEAD`)
    #[arg(value_name = "REV")]
    pub revision: Option<String>,

    #[command(subcommand)]
    pub command: Option<Command>,
}

impl Cli {
    pub fn diff_type(&self) -> DiffType {
        match self.revision.as_deref() {
            None => DiffType::Worktree,
            Some(spec) => parse_revision(spec),
        }
    }
}

/// Parses a single revision or range specifier into [`DiffType`].
pub fn parse_revision(spec: &str) -> DiffType {
    let default_head = |side: &str| {
        if side.is_empty() {
            "HEAD".to_owned()
        } else {
            side.to_owned()
        }
    };

    if let Some((left, right)) = spec.split_once("...") {
        DiffType::MergeBase(default_head(left), default_head(right))
    } else if let Some((left, right)) = spec.split_once("..") {
        DiffType::Between(default_head(left), default_head(right))
    } else {
        DiffType::Against(spec.to_owned())
    }
}

#[derive(Debug, PartialEq, Eq, Subcommand)]
pub enum Command {
    /// Report how this binary was built, and what it found
    Doctor,

    /// Inspect one layer at a time
    #[command(subcommand, hide = true)]
    Debug(Debug),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum ViewerMode {
    SideBySide,
    Inline,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum LogOrder {
    Topo,
    Date,
    Author,
}

#[derive(Debug, PartialEq, Eq, Subcommand)]
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

    /// Print rendered lines and highlight roles for VS Code comparison
    Parity {
        original: String,
        modified: String,
        #[arg(long, value_enum, default_value = "side-by-side")]
        layout: ViewerMode,
        #[arg(long, default_value_t = false, action = clap::ArgAction::Set)]
        ignore_trim_whitespace: bool,
        /// Render with source wrapping enabled or disabled.
        #[arg(long, default_value_t = true, action = clap::ArgAction::Set)]
        wrap: bool,
    },

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

    /// Open a deterministic production-component story
    Ui {
        /// Stable story ID; omit it to browse the catalog
        story: Option<String>,
        /// Print story IDs without taking over the terminal
        #[arg(long, conflicts_with_all = ["story", "snapshot"])]
        list: bool,
        /// Print a deterministic text frame instead of opening the terminal
        #[arg(long)]
        snapshot: bool,
        /// Width of a snapshot in terminal cells
        #[arg(long, requires = "snapshot")]
        width: Option<u16>,
        /// Height of a snapshot in terminal cells
        #[arg(long, requires = "snapshot")]
        height: Option<u16>,
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

    /// Print commit history
    Log {
        /// Commit walk order
        #[arg(long, value_enum, default_value = "topo")]
        order: LogOrder,
        /// Maximum number of commits to list
        #[arg(long, short = 'n', default_value_t = 20)]
        limit: usize,
        /// Repository path, defaulting to the current directory
        #[arg(default_value = ".")]
        dir: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Cli {
        Cli::try_parse_from(std::iter::once("codediff").chain(args.iter().copied()))
            .expect("CLI arguments parse")
    }

    #[test]
    fn no_arguments_reviews_the_worktree() {
        let cli = parse(&[]);
        assert_eq!(cli.command, None);
        assert_eq!(cli.revision, None);
        assert_eq!(cli.diff_type(), DiffType::Worktree);
    }

    #[test]
    fn single_revision_compares_against_the_worktree() {
        let cli = parse(&["main"]);
        assert_eq!(cli.command, None);
        assert_eq!(cli.diff_type(), DiffType::Against("main".to_owned()));
    }

    #[test]
    fn two_dots_parse_as_between_and_default_empty_sides_to_head() {
        assert_eq!(
            parse(&["HEAD~2..HEAD"]).diff_type(),
            DiffType::Between("HEAD~2".to_owned(), "HEAD".to_owned())
        );
        assert_eq!(
            parse(&["main.."]).diff_type(),
            DiffType::Between("main".to_owned(), "HEAD".to_owned())
        );
        assert_eq!(
            parse(&["..feat"]).diff_type(),
            DiffType::Between("HEAD".to_owned(), "feat".to_owned())
        );
        assert_eq!(
            parse(&[".."]).diff_type(),
            DiffType::Between("HEAD".to_owned(), "HEAD".to_owned())
        );
    }

    #[test]
    fn three_dots_parse_as_merge_base_before_two_dots() {
        assert_eq!(
            parse(&["main...feat"]).diff_type(),
            DiffType::MergeBase("main".to_owned(), "feat".to_owned())
        );
        assert_eq!(
            parse(&["main..."]).diff_type(),
            DiffType::MergeBase("main".to_owned(), "HEAD".to_owned())
        );
        assert_eq!(
            parse(&["...feat"]).diff_type(),
            DiffType::MergeBase("HEAD".to_owned(), "feat".to_owned())
        );
        assert_eq!(
            parse(&["..."]).diff_type(),
            DiffType::MergeBase("HEAD".to_owned(), "HEAD".to_owned())
        );
    }

    #[test]
    fn subcommands_take_precedence_over_positional_revision() {
        let doctor = parse(&["doctor"]);
        assert_eq!(doctor.command, Some(Command::Doctor));
        assert_eq!(doctor.revision, None);

        let status = parse(&["debug", "status"]);
        assert_eq!(
            status.command,
            Some(Command::Debug(Debug::Status {
                dir: ".".to_owned(),
                verbose: false,
            }))
        );
        assert_eq!(status.revision, None);
    }

    #[test]
    fn double_dash_treats_subcommand_names_as_revisions() {
        let doctor = parse(&["--", "doctor"]);
        assert_eq!(doctor.command, None);
        assert_eq!(doctor.revision.as_deref(), Some("doctor"));
        assert_eq!(doctor.diff_type(), DiffType::Against("doctor".to_owned()));

        let debug = parse(&["--", "debug"]);
        assert_eq!(debug.command, None);
        assert_eq!(debug.revision.as_deref(), Some("debug"));
        assert_eq!(debug.diff_type(), DiffType::Against("debug".to_owned()));
    }
}
