//! Git diff parsers for file status and line counts.
//!
//! Rename detection is forced in both formats.

pub mod name_status;
pub mod numstat;

/// Forced in every `diff` this crate runs.
pub(crate) const RENAMES: &str = "--find-renames";

/// Builds the argument list for `git diff`, or for `git diff-files` when
/// `subcommand` says so.
pub(crate) fn command<'a>(
    subcommand: &'a str,
    format: &'a str,
    args: &[&'a str],
    pathspec: &'a [String],
) -> Vec<&'a str> {
    let mut out = vec![subcommand, format, "-z", RENAMES];
    out.extend_from_slice(args);
    if !pathspec.is_empty() {
        out.push("--");
        out.extend(pathspec.iter().map(String::as_str));
    }
    out
}
