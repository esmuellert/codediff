//! `git diff` — which files differ, and by how many lines.
//!
//! Two output formats of the same command:
//!
//! ```text
//! name_status   --name-status -z    which files, and what happened
//! numstat       --numstat -z        how many lines each gained and lost
//! ```
//!
//! Rename detection is forced in both so they agree about what is a rename.

pub mod name_status;
pub mod numstat;

/// Forced in every `diff` this crate runs.
pub(crate) const RENAMES: &str = "--find-renames";

/// Builds the argument list for a `git diff` invocation.
pub(crate) fn command<'a>(
    format: &'a str,
    args: &[&'a str],
    pathspec: &'a [String],
) -> Vec<&'a str> {
    let mut out = vec!["diff", format, "-z", RENAMES];
    out.extend_from_slice(args);
    if !pathspec.is_empty() {
        out.push("--");
        out.extend(pathspec.iter().map(String::as_str));
    }
    out
}
