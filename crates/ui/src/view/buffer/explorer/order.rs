//! Sort order for the file list.
//!
//! Sort order matches VS Code's `SCMTreeSorter`: case-insensitive, numeric
//! (`file9` < `file10`), shallower paths first in flat mode.
//!
//! A sort key is built once per path (not per comparison) — 1.2 ms for 20k
//! paths vs 81 ms with inline case-folding.

/// Marks the last segment of a path, and sorts below [`SEGMENT`].
///
/// This is what puts a shallower path first: after a shared prefix one key has
/// reached its file name while the other has another directory to go, and `0`
/// beats `1` without anything having to count segments.
const NAME: u8 = 0;
/// Marks a directory segment.
const SEGMENT: u8 = 1;
/// Marks a run of digits inside a name, and sorts below every text byte, which
/// is where a collator puts digits.
const NUMBER: u8 = 0;

/// A key that sorts as VS Code's `comparePaths` compares.
pub fn path_key(path: &str) -> Vec<u8> {
    let mut key = Vec::with_capacity(path.len() + 8);
    let mut segments = path.split('/').filter(|s| !s.is_empty()).peekable();
    while let Some(segment) = segments.next() {
        if segments.peek().is_none() {
            key.push(NAME);
            push_name(&mut key, segment);
            return key;
        }
        // A directory is folded but not read numerically, because
        // `comparePathComponents` does not read one.
        key.push(SEGMENT);
        push_folded(&mut key, segment);
    }
    // A path that was nothing but separators. It still needs a key.
    key.push(NAME);
    key
}

/// A tree row's key: directories above files, then the name.
///
/// The name sorts as VS Code's `compareFileNames` compares — folded, and with
/// each run of digits written as *how many digits, then the digits*, so `9`
/// (one digit) precedes `10` (two) whatever surrounds them. That is what
/// "numeric" means in a collator.
pub fn tree_key(is_directory: bool, name: &str) -> Vec<u8> {
    let mut key = Vec::with_capacity(name.len() + 5);
    key.push(u8::from(!is_directory));
    push_name(&mut key, name);
    key
}

/// Writes a name folded, with its digit runs made comparable by value.
fn push_name(key: &mut Vec<u8>, name: &str) {
    let bytes = name.as_bytes();
    let mut at = 0;
    while at < bytes.len() {
        let start = at;
        if bytes[at].is_ascii_digit() {
            while at < bytes.len() && bytes[at].is_ascii_digit() {
                at += 1;
            }
            // Leading zeros are not part of the value: `01` and `1` are one
            // number, and a collator calls them equal before falling back to
            // the characters. Whatever carries the key is that fallback.
            let digits = name[start..at].trim_start_matches('0');
            key.push(NUMBER);
            // One byte of length, saturating: a run of more than 255 digits is
            // not a number anyone named a file after, and all of them sorting
            // together is a better answer than a length that wraps around.
            key.push(digits.len().min(u8::MAX as usize) as u8);
            key.extend_from_slice(digits.as_bytes());
            continue;
        }
        while at < bytes.len() && !bytes[at].is_ascii_digit() {
            at += 1;
        }
        push_folded(key, &name[start..at]);
    }
}

/// Writes text folded to lower case.
///
/// ASCII without allocating, which is what almost every path is. Anything else
/// goes through `char::to_lowercase`, which is the only thing that knows what
/// lower case means for it.
fn push_folded(key: &mut Vec<u8>, text: &str) {
    if text.is_ascii() {
        key.extend(text.bytes().map(|byte| byte.to_ascii_lowercase()));
        return;
    }
    let mut buffer = [0u8; 4];
    for lowered in text.chars().flat_map(char::to_lowercase) {
        key.extend_from_slice(lowered.encode_utf8(&mut buffer).as_bytes());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Sorted as the flat list sorts: by the key, with the path as the
    /// tie-break the key deliberately leaves to its carrier.
    fn flat(paths: &[&str]) -> Vec<String> {
        let mut keyed: Vec<(Vec<u8>, &str)> = paths.iter().map(|&p| (path_key(p), p)).collect();
        keyed.sort();
        keyed.into_iter().map(|(_, p)| p.to_owned()).collect()
    }

    fn in_tree(rows: &[(bool, &str)]) -> Vec<String> {
        let mut keyed: Vec<(Vec<u8>, &str)> = rows
            .iter()
            .map(|&(directory, name)| (tree_key(directory, name), name))
            .collect();
        keyed.sort();
        keyed.into_iter().map(|(_, n)| n.to_owned()).collect()
    }

    #[test]
    fn a_shallower_file_comes_before_a_deeper_one() {
        // VS Code's `comparePaths` runs out of segments on one side and
        // returns there. Comparing the two paths as plain strings gives the
        // opposite answer, because `/` sorts below every letter — which is
        // what this codebase did before.
        assert_eq!(
            flat(&["a/b/c.rs", "a/z.rs"]),
            vec!["a/z.rs", "a/b/c.rs"],
            "the file in `a` comes before the file below `a`"
        );
        assert_eq!(flat(&["a/b.rs", "ab.rs"]), vec!["ab.rs", "a/b.rs"]);
    }

    #[test]
    fn a_flat_list_sorts_by_the_whole_path() {
        assert_eq!(
            flat(&["src/z.rs", "README.md", "src/a.rs", "docs/plan.md"]),
            vec!["README.md", "docs/plan.md", "src/a.rs", "src/z.rs"],
            "the file at the top comes before anything in a directory"
        );
    }

    #[test]
    fn numbers_read_as_numbers_and_not_as_text() {
        // The failure this prevents: `file10.rs` between `file1.rs` and
        // `file9.rs`, which is where every naive sort puts it.
        assert_eq!(
            flat(&["file10.rs", "file9.rs", "file1.rs"]),
            vec!["file1.rs", "file9.rs", "file10.rs"]
        );
        assert_eq!(
            in_tree(&[(false, "v10.rs"), (false, "v9.rs")]),
            vec!["v9.rs", "v10.rs"],
            "and in a tree, which sorts names the same way"
        );
    }

    #[test]
    fn leading_zeros_are_not_part_of_the_value() {
        // `01` and `1` are one number, so the key cannot separate them; the
        // path beside it breaks the tie, as a collator's fallback does.
        assert_eq!(tree_key(false, "f01"), tree_key(false, "f1"));
        assert_eq!(flat(&["f1.rs", "f01.rs"]), vec!["f01.rs", "f1.rs"]);
    }

    #[test]
    fn case_is_a_tie_break_and_not_the_sort() {
        // Byte order would put every capital before every lowercase, so
        // `README.md` would sort before `docs/`, which reads as random.
        assert_eq!(
            flat(&["b.rs", "A.rs", "a.rs"]),
            vec!["A.rs", "a.rs", "b.rs"]
        );
    }

    #[test]
    fn a_tree_puts_directories_above_files() {
        assert_eq!(
            in_tree(&[(false, "a.rs"), (true, "zebra"), (false, "b.rs")]),
            vec!["zebra", "a.rs", "b.rs"]
        );
    }

    #[test]
    fn a_key_is_not_a_total_order_and_says_so() {
        // Two spellings of one name fold to one key, so whatever sorts must
        // carry the original beside it. Without that their order would depend
        // on the sort's implementation, and rows would move under the reader
        // between refreshes.
        assert_eq!(path_key("readme"), path_key("README"));
        assert_eq!(flat(&["readme", "README"]), vec!["README", "readme"]);
    }

    #[test]
    fn a_name_that_is_not_ascii_still_folds() {
        assert_eq!(tree_key(false, "ÜNÏCODÉ"), tree_key(false, "ünïcodé"));
        assert_eq!(
            flat(&["ünïcodé.txt", "a.txt"]),
            vec!["a.txt", "ünïcodé.txt"]
        );
    }

    #[test]
    fn a_digit_run_sorts_before_text_at_the_same_place() {
        // Where a collator puts them, and where `NUMBER` being below every
        // text byte puts them here.
        assert_eq!(flat(&["fa.rs", "f1.rs"]), vec!["f1.rs", "fa.rs"]);
    }
}
