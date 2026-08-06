//! The glob matcher against a reference that does the same thing slowly.
//!
//! Memoising it made it fast and, at first, wrong: the table was indexed by
//! the length of the star token just consumed rather than by the pattern left
//! to match, so unrelated pairs shared a cell and one failure suppressed a
//! branch that would have matched. Every existing test still passed, because
//! each used at most one star per segment.
//!
//! A reference implementation is the only thing that catches that. This one is
//! deliberately the naive exponential walk — it is here to be obviously right,
//! not to be fast, and the inputs are kept small enough that it finishes.

use explorer::matches;

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
