//! What goes in front of a line of the nested arrangement.
//!
//! A guide at a given depth means *an ancestor at that depth has more children
//! after it* — read off the node's own parents, and blank space rather than a
//! guide where that ancestor was the last, so nothing runs down beside
//! nothing.
//!
//! The chain is walked upwards and the indent written out reversed, because a
//! node knows what it hangs from and not the other way round. It is a handful
//! of steps for a handful of lines: the depth of a repository's directories,
//! once per line that fits on the screen.
//!
//! **Only the nested arrangement reaches here.** A flat list has no ancestors,
//! and a guide beside a whole path would draw a tree where there is none —
//! which is what VS Code's own list mode refuses too. See D69.

use ratatui::style::Style;

use crate::theme::Theme;
use crate::view::buffer::explorer::{NodeId, Tree};

use super::view_line::{Piece, priority};

/// How a directory says whether it is open.
///
/// Triangles rather than nerd-font folders, so the list is readable in a
/// terminal with any font. One place to change if that is ever configurable.
const OPEN: &str = "▾ ";
const SHUT: &str = "▸ ";

/// The indent guides and fold arrow for one node.
pub fn prefix(tree: &Tree, id: NodeId, theme: &Theme, background: Style) -> Vec<Piece> {
    let marker = background.fg(theme.tree.marker);
    let node = tree.node(id);
    let mut pieces = vec![Piece::droppable(indent(tree, id), marker, priority::GUIDES)];
    if node.is_directory() {
        pieces.push(Piece::fixed(
            if node.is_open() { OPEN } else { SHUT },
            marker,
        ));
    }
    pieces
}

/// The columns before a line's name.
///
/// Its own branch, then one column pair per ancestor above it.
fn indent(tree: &Tree, id: NodeId) -> String {
    let node = tree.node(id);
    let mut levels = vec![if node.is_last { "└ " } else { "├ " }];
    let mut above = node.parent;
    while let Some(parent) = above {
        let parent = tree.node(parent);
        levels.push(if parent.is_last { "  " } else { "│ " });
        above = parent.parent;
    }
    levels.into_iter().rev().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::view::buffer::explorer::{Explorer, ViewMode};
    use file_types::{File, Oid, RepoPath, Revs};
    use std::path::Path;

    fn file(path: &str) -> File {
        File::unchanged_path(
            RepoPath::new(path, Path::new("/repo")),
            Revs::worktree_against(Oid::new("b87b24c")),
        )
    }

    /// The prefix of every line, as one string each.
    fn indents(explorer: &Explorer) -> Vec<String> {
        let theme = Theme::named("basic-dark").expect("a theme");
        (0..explorer.view_lines())
            .map(|line| match explorer.nested_at(line) {
                Some((tree, id)) => prefix(tree, id, &theme, Style::default())
                    .iter()
                    .map(|piece| piece.text.as_str())
                    .collect::<String>(),
                // A heading, which is what the arrangement hangs from rather
                // than a line in it.
                None => String::new(),
            })
            .collect()
    }

    #[test]
    fn a_guide_is_drawn_only_where_an_ancestor_has_more_to_come() {
        let explorer = Explorer::new(vec![
            file("nest/a/one.txt"),
            file("nest/b/two.txt"),
            file("nest/b/three.txt"),
        ]);
        assert_eq!(
            indents(&explorer),
            vec![
                "",       // the heading has no indent to describe
                "└ ▾ ",   // nest, the only thing at the top level
                "  ├ ▾ ", // nest/a — its ancestor was last, so blank space
                "  │ └ ", // one.txt
                "  └ ▾ ", // nest/b
                "    ├ ", // three.txt
                "    └ ", // two.txt
            ]
        );
    }

    #[test]
    fn a_shut_directory_says_so_and_an_open_one_says_so() {
        let mut explorer = Explorer::new(vec![file("src/a.rs"), file("src/b.rs")]);
        assert!(indents(&explorer)[1].contains(OPEN));
        explorer.toggle(1);
        assert!(indents(&explorer)[1].contains(SHUT));
    }

    #[test]
    fn the_flat_arrangement_has_no_prefix_to_ask_for() {
        // Not a special case here but an absence: `nested_at` answers only for
        // the arrangement that has ancestors, so there is no tree to ask.
        let mut explorer = Explorer::new(vec![file("nest/a/one.txt")]);
        explorer.set_mode(ViewMode::List);
        assert_eq!(indents(&explorer), vec!["", ""]);
    }
}
