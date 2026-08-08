//! Hiding rows by a glob.
//!
//! Globs rather than regular expressions, because the thing being matched is a
//! path and everyone already knows what `src/**/*.rs` means. Written by hand
//! rather than taken from a crate: the whole grammar is three wildcards, and a
//! dependency would bring a regular expression engine into a crate that has no
//! other reason to have one.
//!
//! One rule worth stating, because it is the one people get wrong: `*` stops
//! at a directory separator and `**` does not. So `src/*.rs` matches
//! `src/main.rs` but not `src/bin/main.rs`, and `src/**/*.rs` matches both.

/// Whether `path` matches `pattern`.
///
/// A pattern with no separator in it is matched against the file name as well
/// as the whole path, so typing `*.rs` finds every Rust file at any depth.
/// That is what a reader means, and requiring `**/*.rs` for it would make the
/// common case the awkward one.
pub fn matches(pattern: &str, path: &str) -> bool {
    if pattern.is_empty() {
        return true;
    }
    if glob(pattern, path) {
        return true;
    }
    if !pattern.contains('/') {
        let name = path.rsplit('/').next().unwrap_or(path);
        return glob(pattern, name);
    }
    false
}

/// Matches one glob against one string, anchored at both ends.
fn glob(pattern: &str, text: &str) -> bool {
    let pattern: Vec<char> = pattern.chars().collect();
    let text: Vec<char> = text.chars().collect();
    // One `seen` for the whole match. Plain backtracking re-tries the same
    // (pattern, text) pair once per path through the stars above it, which a
    // pattern like `a**a**a**a**z` turns into minutes of work on a short path
    // — measured at seventeen seconds for eight groups. Remembering which
    // pairs have already failed makes it the product of the two lengths.
    let mut seen = vec![false; (pattern.len() + 1) * (text.len() + 1)];
    walk(&pattern, &text, &mut seen, text.len() + 1)
}

/// The ordinary backtracking match, with `**` given its own case.
///
/// `seen` marks the `(pattern, text)` pairs already known to fail. It is only
/// ever set, never cleared: a pair that failed once fails every time, because
/// what happens from here depends on nothing else.
fn walk(pattern: &[char], text: &[char], seen: &mut [bool], stride: usize) -> bool {
    match pattern.first() {
        None => text.is_empty(),
        Some('*') => {
            // `**` crosses separators, `*` does not. `**/` also matches no
            // directory at all, so `src/**/*.rs` finds `src/main.rs`.
            let (rest, crosses) = match pattern.get(1) {
                Some('*') => (skip_slash(&pattern[2..]), true),
                _ => (&pattern[1..], false),
            };
            for taken in 0..=text.len() {
                if !crosses && text[..taken].contains(&'/') {
                    break;
                }
                // Indexed by what is *left* of the pattern, which is what
                // identifies the suffix. Indexing by the length of the star
                // token just consumed — 1, 2 or 3 — made unrelated pairs share
                // a cell, so one failure suppressed a branch that would have
                // matched: `**/*r*/*.rs` stopped finding
                // `crates/ui/src/render/explorer.rs`.
                let cell = rest.len() * stride + (text.len() - taken);
                if seen[cell] {
                    continue;
                }
                if walk(rest, &text[taken..], seen, stride) {
                    return true;
                }
                seen[cell] = true;
            }
            false
        }
        Some('?') => {
            !text.is_empty() && text[0] != '/' && walk(&pattern[1..], &text[1..], seen, stride)
        }
        Some(&expected) => {
            !text.is_empty() && text[0] == expected && walk(&pattern[1..], &text[1..], seen, stride)
        }
    }
}

/// Steps over the separator after `**`, so that it may match nothing.
fn skip_slash(pattern: &[char]) -> &[char] {
    match pattern.first() {
        Some('/') => &pattern[1..],
        _ => pattern,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_star_stops_at_a_separator_and_a_double_star_does_not() {
        assert!(matches("src/*.rs", "src/main.rs"));
        assert!(!matches("src/*.rs", "src/bin/main.rs"));
        assert!(matches("src/**/*.rs", "src/bin/main.rs"));
    }

    #[test]
    fn a_double_star_also_matches_no_directory_at_all() {
        // Without this, `src/**/*.rs` would silently miss the files directly
        // in `src`, which is not what anyone typing it means.
        assert!(matches("src/**/*.rs", "src/main.rs"));
    }

    #[test]
    fn a_bare_pattern_is_matched_against_the_file_name() {
        assert!(matches("*.rs", "crates/ui/src/app.rs"));
        assert!(!matches("*.rs", "crates/ui/README.md"));
    }

    #[test]
    fn a_question_mark_is_one_character_and_never_a_separator() {
        assert!(matches("a?c.rs", "abc.rs"));
        assert!(!matches("a?c.rs", "ac.rs"));
        assert!(!matches("a?c", "a/c"));
    }

    #[test]
    fn a_pattern_full_of_stars_finishes_at_once() {
        // Plain backtracking took seventeen seconds on this, and two more
        // groups put it into minutes. A live filter box runs this once per
        // entry per keystroke.
        let path = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa/b.rs";
        let start = std::time::Instant::now();
        assert!(!matches("a**a**a**a**a**a**a**a**z", path));
        assert!(matches("a**a**a**a**a**a**a**a**s", path));
        assert!(start.elapsed().as_secs() < 2, "took {:?}", start.elapsed());
    }

    #[test]
    fn an_empty_pattern_hides_nothing() {
        assert!(matches("", "anything at all"));
    }

    #[test]
    fn a_pattern_must_match_the_whole_path_and_not_part_of_it() {
        // The failure this prevents: typing `main` and getting every file
        // whose path merely contains it.
        assert!(!matches("main", "src/main.rs"));
        assert!(matches("main.rs", "src/main.rs"), "by file name");
    }

    // ---- Against a reference that does the same thing slowly. ----
    //
    // Memoising the matcher made it fast and, at first, wrong: the table was
    // indexed by the length of the star token just consumed rather than by the
    // pattern left to match, so unrelated pairs shared a cell and one failure
    // suppressed a branch that would have matched. Every test above still
    // passed, because each uses at most one star per segment.
    //
    // A reference implementation is the only thing that catches that. The one
    // below is deliberately the naive exponential walk — it is here to be
    // obviously right, not to be fast, and the inputs are kept small enough
    // that it finishes.

    /// The same rules, without the table.
    fn naive(pattern: &str, path: &str) -> bool {
        if pattern.is_empty() {
            return true;
        }
        if walk(&chars(pattern), &chars(path)) {
            return true;
        }
        if !pattern.contains('/') {
            let name = path.rsplit('/').next().unwrap_or(path);
            return walk(&chars(pattern), &chars(name));
        }
        false
    }

    fn chars(text: &str) -> Vec<char> {
        text.chars().collect()
    }

    fn walk(pattern: &[char], text: &[char]) -> bool {
        match pattern.first() {
            None => text.is_empty(),
            Some('*') => {
                let (rest, crosses) = match pattern.get(1) {
                    Some('*') => (skip_slash(&pattern[2..]), true),
                    _ => (&pattern[1..], false),
                };
                for taken in 0..=text.len() {
                    if !crosses && text[..taken].contains(&'/') {
                        break;
                    }
                    if walk(rest, &text[taken..]) {
                        return true;
                    }
                }
                false
            }
            Some('?') => !text.is_empty() && text[0] != '/' && walk(&pattern[1..], &text[1..]),
            Some(&expected) => {
                !text.is_empty() && text[0] == expected && walk(&pattern[1..], &text[1..])
            }
        }
    }

    fn skip_slash(pattern: &[char]) -> &[char] {
        match pattern.first() {
            Some('/') => &pattern[1..],
            _ => pattern,
        }
    }

    /// Every string of `alphabet` up to `longest` characters.
    fn every(alphabet: &[char], longest: usize) -> Vec<String> {
        let mut all = vec![String::new()];
        let mut edge = vec![String::new()];
        for _ in 0..longest {
            let mut next = Vec::new();
            for word in &edge {
                for &letter in alphabet {
                    let mut longer = word.clone();
                    longer.push(letter);
                    next.push(longer);
                }
            }
            all.extend(next.iter().cloned());
            edge = next;
        }
        all
    }

    #[test]
    fn the_fast_matcher_answers_what_the_slow_one_answers() {
        let patterns = every(&['a', 'b', '*', '/', '?'], 5);
        let texts = every(&['a', 'b', '/'], 4);
        let mut checked = 0usize;
        for pattern in &patterns {
            for text in &texts {
                assert_eq!(
                    matches(pattern, text),
                    naive(pattern, text),
                    "pattern {pattern:?} against {text:?}"
                );
                checked += 1;
            }
        }
        assert!(checked > 100_000, "only {checked} pairs were compared");
    }

    #[test]
    fn the_patterns_a_reviewer_would_actually_type_match() {
        // `**/*r*/*.rs` was the one that stopped working, and it is not exotic.
        let path = "crates/ui/src/render/explorer.rs";
        for pattern in [
            "**/*r*/*.rs",
            "**/*e*/*.rs",
            "crates/**/*.rs",
            "*.rs",
            "**/render/*",
        ] {
            assert!(matches(pattern, path), "{pattern} should match {path}");
            assert_eq!(matches(pattern, path), naive(pattern, path));
        }
        for pattern in ["**/*z*/*.rs", "crates/*.rs", "*.md"] {
            assert!(!matches(pattern, path), "{pattern} should not match {path}");
        }
    }
}
