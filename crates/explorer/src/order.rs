//! What comes before what.
//!
//! Two orders, because the two views are answering different questions. A flat
//! list is a list of *paths*, so it sorts by the whole path, as VS Code's
//! source control panel does. A tree is a walk of *directories*, so each
//! directory sorts its own children, and directories come before files —
//! otherwise a folder could sit between two files that are not in it, and the
//! indent guides would cross unrelated rows.
//!
//! Both compare case-insensitively first and fall back to the bytes, so
//! `README.md` and `readme.md` are neighbours rather than pages apart, and two
//! names that differ only in case still have a stable order.

use std::cmp::Ordering;

/// Compares two names the way both orders compare names.
///
/// Lowercase first, so that case is a tie-break rather than the sort. The
/// byte comparison behind it is what makes the order total: without it, two
/// spellings of one name would compare equal and their order would depend on
/// how the sort was implemented.
pub fn by_name(left: &str, right: &str) -> Ordering {
    let folded = left
        .chars()
        .flat_map(char::to_lowercase)
        .cmp(right.chars().flat_map(char::to_lowercase));
    folded.then_with(|| left.cmp(right))
}

/// Compares two rows of a tree: directories first, then by name.
pub fn in_tree(left: (bool, &str), right: (bool, &str)) -> Ordering {
    let (left_is_directory, left_name) = left;
    let (right_is_directory, right_name) = right;
    right_is_directory
        .cmp(&left_is_directory)
        .then_with(|| by_name(left_name, right_name))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sorted_flat(mut names: Vec<&str>) -> Vec<&str> {
        names.sort_by(|a, b| by_name(a, b));
        names
    }

    #[test]
    fn a_flat_list_sorts_by_the_whole_path() {
        assert_eq!(
            sorted_flat(vec!["src/z.rs", "README.md", "src/a.rs", "docs/plan.md"]),
            vec!["docs/plan.md", "README.md", "src/a.rs", "src/z.rs"]
        );
    }

    #[test]
    fn case_is_a_tie_break_and_not_the_sort() {
        // Byte order would put every capital before every lowercase, so
        // `README.md` would sort before `docs/`, which reads as random.
        assert_eq!(
            sorted_flat(vec!["b.rs", "A.rs", "a.rs"]),
            vec!["A.rs", "a.rs", "b.rs"]
        );
    }

    #[test]
    fn a_tree_puts_directories_above_files() {
        let mut rows = vec![(false, "a.rs"), (true, "zebra"), (false, "b.rs")];
        rows.sort_by(|&a, &b| in_tree(a, b));
        assert_eq!(
            rows,
            vec![(true, "zebra"), (false, "a.rs"), (false, "b.rs")]
        );
    }

    #[test]
    fn the_order_is_total_so_a_sort_cannot_wobble() {
        // Two spellings of one name must not compare equal: an unstable order
        // between them would move rows under the reader between refreshes.
        assert_ne!(by_name("readme", "README"), Ordering::Equal);
    }
}
