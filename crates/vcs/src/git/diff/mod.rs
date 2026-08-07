//! `git diff` — what differs between two things git can name.
//!
//! Two files because there are two questions and git answers them with two
//! output formats, not because they are two commands. Both take the same
//! revisions and the same pathspec; only the flag that shapes the output
//! differs.
//!
//! ```text
//! name_status   --name-status -z    which files, and what happened
//! numstat       --numstat -z        how many lines each gained and lost
//! ```
//!
//! **Rename detection is forced in both**, because they are read together: one
//! saying a file is a rename while the other counts it as a whole new file
//! would put a `+400` beside a move. Neither may be left to the reader's own
//! configuration. See D56.

pub mod name_status;
pub mod numstat;

/// Forced in every `diff` this crate runs. See the module doc.
pub(crate) const RENAMES: &str = "--find-renames";

/// `diff`, the flags, whatever the caller added, then the pathspec.
///
/// Built here because both formats need the same shape and got it slightly
/// differently: one appended `--` unconditionally, which git reads as an empty
/// pathspec matching nothing.
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
