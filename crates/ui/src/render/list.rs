//! What one row of the file list says, and how it is coloured.
//!
//! The `explorer` crate reports facts — this row is a directory, it is the
//! last of its siblings, its second ancestor was not. This file turns those
//! into text and colour: `▾`, `│ `, `+4`, `M`.
//!
//! The list's half of [`line`](super::line), which does the same for a diff:
//! takes what its own crate reports, adds a theme, answers in text and colour.
//! That `align` reports a gap and `column` knows a gap is drawn `╱` is the
//! same division, and the reason both live here rather than in the crate that
//! reports. See D65.
//!
//! Nothing here decides what survives a narrow pane. That is [`fit`], which
//! knows about neither list nor diff and is shared with the status line.
//!
//! [`fit`]: super::fit

use ratatui::style::{Modifier, Style};

use explorer::{Content, Guides, Row};
use file_types::{ChangeType, Stats};

use super::fit::Piece;
use crate::theme::Theme;

/// How a directory says whether it is open.
///
/// Triangles rather than nerd-font folders, so the list is readable in a
/// terminal with any font. One place to change if that is ever configurable.
const OPEN: &str = "▾ ";
const SHUT: &str = "▸ ";

/// The order pieces are dropped in when a row will not fit.
///
/// Named rather than written as numbers at each use, so that "stats go before
/// the guides" is a statement in one place instead of a comparison a reader
/// has to make between two distant literals.
mod priority {
    /// Where a moved file came from: useful, never essential.
    pub const MOVED: u8 = 0;
    /// The line counts.
    pub const STATS: u8 = 1;
    /// How many files a section holds.
    pub const COUNT: u8 = 2;
    /// The indent guides. Last to go, because losing them makes the tree
    /// unreadable rather than merely plainer.
    pub const GUIDES: u8 = 3;
}

/// One row as the pieces it is drawn from.
///
/// The two sides are separate because a row is drawn from both ends: the name
/// grows rightwards from the indent, and the status letter is pinned to the
/// edge so the eye can run down the column.
///
/// `background` is the row's own — the selected row keeps its highlight under
/// every colour it holds, rather than having it replaced by them.
pub fn pieces(row: &Row, theme: &Theme, background: Style) -> (Vec<Piece>, Vec<Piece>) {
    let list = &theme.list;
    let mut left = Vec::new();
    let mut right = Vec::new();

    if let Some(guides) = &row.guides {
        left.push(Piece::droppable(
            indent(guides),
            background.fg(list.marker),
            priority::GUIDES,
        ));
    }

    match &row.content {
        Content::Heading {
            title,
            files,
            stats,
        } => {
            // Bold is applied here rather than stored in the theme: a heading
            // is bold in every theme, so it is structural. That is the same
            // division `Code` makes, where weight comes from the scope table
            // and colour from the theme.
            left.push(Piece::fixed(
                title,
                background.fg(list.heading).add_modifier(Modifier::BOLD),
            ));
            let count = background.fg(list.count);
            match stats {
                Some(stats) => {
                    left.push(Piece::droppable(
                        format!(" ({files} · "),
                        count,
                        priority::COUNT,
                    ));
                    push_stats(&mut left, *stats, priority::COUNT, theme, background);
                    left.push(Piece::droppable(")", count, priority::COUNT));
                }
                None => left.push(Piece::droppable(
                    format!(" ({files})"),
                    count,
                    priority::COUNT,
                )),
            }
        }
        Content::Directory { name, open } => {
            left.push(Piece::fixed(
                if *open { OPEN } else { SHUT },
                background.fg(list.marker),
            ));
            left.push(Piece::fixed(name, background.fg(list.directory)));
        }
        Content::File {
            name,
            moved_from,
            stats,
            change,
        } => {
            left.push(Piece::fixed(name, background.fg(list.name)));
            if let Some(previous) = moved_from {
                left.push(Piece::droppable(
                    format!(" ← {previous}"),
                    background.fg(list.moved),
                    priority::MOVED,
                ));
            }
            if let Some(stats) = stats {
                push_stats(&mut right, *stats, priority::STATS, theme, background);
                // The space between the counts and the letter, which goes with
                // them rather than staying behind as a lone column.
                right.push(Piece::droppable(
                    " ",
                    background.fg(list.name),
                    priority::STATS,
                ));
            }
            right.push(Piece::fixed(
                letter(*change),
                background
                    .fg(status_colour(*change, theme))
                    .add_modifier(Modifier::BOLD),
            ));
        }
    }

    (left, right)
}

/// The indent guides for a row.
///
/// A guide at a given depth means an ancestor at that depth has more children
/// after it; an ancestor that was last leaves blank space, not a guide running
/// down beside nothing.
fn indent(guides: &Guides) -> String {
    let mut out = String::new();
    for &ancestor_was_last in &guides.ancestors {
        out.push_str(if ancestor_was_last { "  " } else { "│ " });
    }
    out.push_str(if guides.is_last { "└ " } else { "├ " });
    out
}

/// The `+4 -3` pair, with a side left out when it is zero.
///
/// A file that only gained lines says `+4`, not `+4 -0`: the zero is noise in
/// a column the eye is scanning.
fn push_stats(
    pieces: &mut Vec<Piece>,
    stats: Stats,
    priority: u8,
    theme: &Theme,
    background: Style,
) {
    if stats.added > 0 {
        pieces.push(Piece::droppable(
            format!("+{}", stats.added),
            background.fg(theme.list.added),
            priority,
        ));
    }
    if stats.removed > 0 {
        let separator = if stats.added > 0 { " " } else { "" };
        pieces.push(Piece::droppable(
            format!("{separator}-{}", stats.removed),
            background.fg(theme.list.removed),
            priority,
        ));
    }
}

/// Git's letter for what happened.
pub fn letter(change: ChangeType) -> &'static str {
    match change {
        ChangeType::Added => "A",
        ChangeType::Modified => "M",
        ChangeType::Deleted => "D",
        ChangeType::Moved => "R",
        ChangeType::Untracked => "??",
        ChangeType::Conflicted => "!",
    }
}

fn status_colour(change: ChangeType, theme: &Theme) -> ratatui::style::Color {
    let list = &theme.list;
    match change {
        ChangeType::Added => list.new_file,
        ChangeType::Modified => list.modified,
        ChangeType::Deleted => list.deleted,
        ChangeType::Moved => list.renamed,
        ChangeType::Untracked => list.untracked,
        ChangeType::Conflicted => list.conflicted,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn guides(ancestors: &[bool], is_last: bool) -> Guides {
        Guides {
            ancestors: ancestors.to_vec(),
            is_last,
        }
    }

    #[test]
    fn a_guide_is_drawn_only_where_an_ancestor_has_more_to_come() {
        // The three cases, in the order they appear down a tree.
        assert_eq!(indent(&guides(&[], false)), "├ ");
        assert_eq!(indent(&guides(&[], true)), "└ ");
        assert_eq!(indent(&guides(&[false], true)), "│ └ ");
        // An ancestor that was last leaves blank space, not a trailing guide
        // running down beside nothing.
        assert_eq!(indent(&guides(&[true], false)), "  ├ ");
        assert_eq!(indent(&guides(&[false, true], false)), "│   ├ ");
    }

    #[test]
    fn every_change_has_a_letter_of_its_own() {
        // A letter shared by two changes would make one of them unreadable in
        // a column the eye is scanning.
        let changes = [
            ChangeType::Added,
            ChangeType::Modified,
            ChangeType::Deleted,
            ChangeType::Moved,
            ChangeType::Untracked,
            ChangeType::Conflicted,
        ];
        let mut seen: Vec<&str> = changes.iter().map(|&c| letter(c)).collect();
        seen.sort_unstable();
        let count = seen.len();
        seen.dedup();
        assert_eq!(seen.len(), count, "two changes share a letter");
    }
}
