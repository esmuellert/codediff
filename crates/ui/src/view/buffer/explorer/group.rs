//! Which comparison a file is in, and which one a line falls in.
//!
//! **A group is a revision pair.** "Staged Changes" is not a category a file
//! belongs to — it is the name for comparing the index against a commit, and
//! every file already carries that pair. So grouping reads a field rather than
//! consulting a category a file was put in, and the heading is derived rather
//! than stored. See D57.
//!
//! The Neovim plugin kept it twice, as a struct of `unstaged` and `staged`
//! lists, and its own source records what that cost: comparing two revisions
//! produced files that were neither, and it wrote
//!
//! ```text
//! -- For revision comparison, we treat everything as "unstaged" for explorer
//! -- compatibility
//! ```
//!
//! **A heading occupies a line.** That is what [`locate`] is for: the groups
//! are drawn one after another, so a line number has to be resolved to a group
//! and a line within it before anything can be asked about it.

use file_types::{File, Revs};

use super::Style;

/// One comparison: what it is called, what is in it, and how that is arranged.
#[derive(Debug)]
pub struct Group {
    pub heading: &'static str,
    /// Places in the list the explorer holds. Kept beside the arrangement
    /// because how many files a heading holds is the *group's* fact, and an
    /// arrangement that had to be asked would have to be asked differently for
    /// each.
    pub files: Vec<usize>,
    /// Whether what is under the heading is showing.
    pub open: bool,
    pub style: Style,
}

/// The files of each comparison, in the order the comparisons first appear.
///
/// Positions rather than files, because whatever calls this owns the list. The
/// order is the backend's: it knows that what is unstaged is reviewed before
/// what is staged, and a comparison of two revisions has only one group to
/// order.
pub fn of(files: &[File]) -> Vec<(&'static str, Vec<usize>)> {
    let mut groups: Vec<(Revs, Vec<usize>)> = Vec::new();
    for (index, file) in files.iter().enumerate() {
        let revs = file.revs();
        match groups.iter_mut().find(|(seen, _)| *seen == revs) {
            Some((_, members)) => members.push(index),
            None => groups.push((revs, vec![index])),
        }
    }
    groups
        .into_iter()
        .map(|(revs, members)| (revs.heading(), members))
        .collect()
}

/// How many lines the groups take, headings included.
pub fn view_lines(groups: &[Group]) -> u32 {
    groups
        .iter()
        .map(|group| {
            1 + if group.open {
                group.style.view_lines()
            } else {
                0
            }
        })
        .sum()
}

/// Which group's heading is on a line, if a heading is.
///
/// The groups are drawn one after another, so this and [`get_line_style`] are
/// the two halves of one question and cover a line between them: a line is a
/// heading, or it is inside a style, or it is past the end.
pub fn get_heading_line(groups: &[Group], line: u32) -> Option<usize> {
    let mut at = 0;
    for (index, group) in groups.iter().enumerate() {
        if line == at {
            return Some(index);
        }
        at += 1;
        if group.open {
            at += group.style.view_lines();
        }
    }
    None
}

/// Which group's style owns a line, and which of *its* lines it is.
///
/// A translation: the screen counts every line of every group from zero, and a
/// style counts only its own. So the first line of the second group might be
/// line 5 on screen and line 0 to the style that draws it.
///
/// `None` for a heading, which is what the styles sit under rather than a line
/// in one.
///
/// Walked rather than looked up: there are one or two groups, and a table of
/// line numbers would be a third thing to keep in step with a fold.
pub fn get_line_style(groups: &[Group], line: u32) -> Option<(usize, usize)> {
    let mut at = 0;
    for (index, group) in groups.iter().enumerate() {
        // The heading's own line, which is not inside anything.
        at += 1;
        if !group.open {
            continue;
        }
        let lines = group.style.view_lines();
        if line >= at && line < at + lines {
            return Some((index, (line - at) as usize));
        }
        at += lines;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use file_types::{File, Oid, RepoPath, Rev};
    use std::path::Path;

    fn at(path: &str, revs: Revs) -> File {
        File::unchanged_path(RepoPath::new(path, Path::new("/repo")), revs)
    }

    fn unstaged() -> Revs {
        Revs::new(Rev::Index, Rev::Worktree)
    }

    fn staged() -> Revs {
        Revs::new(Rev::Commit(Oid::new("b87b24c")), Rev::Index)
    }

    #[test]
    fn a_heading_is_read_off_the_revisions_rather_than_stored() {
        // The property `vcs::Changes.name` could not have: there is one answer
        // to what a group is, so nothing can disagree with it.
        assert_eq!(unstaged().heading(), "Changes");
        assert_eq!(staged().heading(), "Staged Changes");
    }

    #[test]
    fn a_file_staged_and_edited_again_is_in_two_groups() {
        // Two comparisons of one path, and two lines to review, so grouping by
        // the revision pair must not merge them. See D47.
        assert_eq!(
            of(&[at("a.rs", unstaged()), at("a.rs", staged())]),
            vec![("Changes", vec![0]), ("Staged Changes", vec![1])]
        );
    }

    #[test]
    fn the_groups_keep_the_order_the_files_arrived_in() {
        // What is unstaged is reviewed before what is staged, and only the
        // backend knows that — so the order is read here, never decided.
        assert_eq!(
            of(&[
                at("a.rs", staged()),
                at("b.rs", unstaged()),
                at("c.rs", staged()),
            ]),
            vec![("Staged Changes", vec![0, 2]), ("Changes", vec![1])],
            "the comparison that arrived first is listed first"
        );
    }

    #[test]
    fn nothing_changed_is_no_groups_at_all() {
        assert!(of(&[]).is_empty());
    }
}
