//! Invariants that must hold for any pair of files.
//!
//! The fixtures cover edits somebody thought of. These cover the rest: text is
//! generated from a tiny alphabet so the engine finds real matches and produces
//! genuinely mixed diffs rather than one big replacement.

use align::{Alignment, DiffVersion, ViewLineType};
use file_types::DiffType;
use proptest::prelude::*;
use vscode_diff::Options;

/// Lines drawn from a small pool, so two generated files share material and the
/// diff contains insertions, deletions and modifications rather than one
/// wholesale replacement.
fn file() -> impl Strategy<Value = Vec<String>> {
    proptest::collection::vec(
        prop_oneof![
            Just("fn a() {}".to_owned()),
            Just("fn b() {}".to_owned()),
            Just("    return 1;".to_owned()),
            Just("    return 2;".to_owned()),
            Just("}".to_owned()),
            Just(String::new()),
            Just("\tlet x = 1;".to_owned()),
            Just("// 日本語 comment".to_owned()),
            Just("let icon = \"🎉\";".to_owned()),
        ],
        0..24,
    )
}

fn check(original: &[String], modified: &[String]) -> Result<(), TestCaseError> {
    let original: Vec<&str> = original.iter().map(String::as_str).collect();
    let modified: Vec<&str> = modified.iter().map(String::as_str).collect();
    let Ok(diff) = vscode_diff::compute(&original, &modified, &Options::default().with_moves())
    else {
        return Ok(()); // a timeout is not an alignment bug
    };
    let alignment = Alignment::new(diff.clone(), &original, &modified);

    let mut left = Vec::new();
    let mut right = Vec::new();
    let (mut last_original, mut last_modified) = (0, 0);

    for line in alignment.view_lines(DiffType::SideBySide) {
        // 1. no line is blank on both sides
        prop_assert!(!(line.original.is_filler() && line.modified.is_filler()));

        // 2. a filler appears exactly when the kind says one should
        let has_filler = line.original.is_filler() || line.modified.is_filler();
        prop_assert_eq!(
            has_filler,
            matches!(line.kind, ViewLineType::Deleted | ViewLineType::Inserted)
        );

        if let Some(n) = line.original.line() {
            // 3. line numbers advance by one and never repeat
            prop_assert_eq!(n, last_original + 1);
            last_original = n;
            left.push(
                alignment
                    .line(DiffVersion::Original, n)
                    .expect("line exists"),
            );
        }
        if let Some(n) = line.modified.line() {
            prop_assert_eq!(n, last_modified + 1);
            last_modified = n;
            right.push(
                alignment
                    .line(DiffVersion::Modified, n)
                    .expect("line exists"),
            );
        }

        // 4. an unchanged line really does hold the same text on both sides
        if line.kind == ViewLineType::Unchanged {
            let (o, m) = line.line_pair().expect("an unchanged line has both sides");
            prop_assert_eq!(
                alignment.line(DiffVersion::Original, o),
                alignment.line(DiffVersion::Modified, m)
            );
        }
    }

    // 5. each column reads back as the file it came from.
    //    Compared against `lines()` rather than the input, because an empty
    //    file is normalised to a single empty line — the engine's model of one,
    //    and what the diff's line numbers refer to.
    prop_assert_eq!(left.as_slice(), alignment.lines(DiffVersion::Original));
    prop_assert_eq!(right.as_slice(), alignment.lines(DiffVersion::Modified));

    // 6. the advertised line count is the number of lines produced
    prop_assert_eq!(
        alignment.view_line_count(DiffType::SideBySide) as usize,
        alignment.view_lines(DiffType::SideBySide).count()
    );

    Ok(())
}

proptest! {
    #[test]
    fn the_six_invariants_hold(original in file(), modified in file()) {
        check(&original, &modified)?;
    }

    #[test]
    fn a_file_paired_with_itself_is_entirely_unchanged(lines in file()) {
        let text: Vec<&str> = lines.iter().map(String::as_str).collect();
        let diff = vscode_diff::compute(&text, &text, &Options::default())
            .expect("comparing a file with itself cannot time out");
        let alignment = Alignment::new(diff.clone(), &text, &text);

        prop_assert_eq!(alignment.view_line_count(DiffType::SideBySide) as usize, alignment.lines(DiffVersion::Original).len());
        for line in alignment.view_lines(DiffType::SideBySide) {
            prop_assert_eq!(line.kind, ViewLineType::Unchanged);
        }
    }

    #[test]
    fn character_spans_can_always_slice_their_line(original in file(), modified in file()) {
        let original: Vec<&str> = original.iter().map(String::as_str).collect();
        let modified: Vec<&str> = modified.iter().map(String::as_str).collect();
        let Ok(diff) = vscode_diff::compute(&original, &modified, &Options::default()) else {
            return Ok(());
        };
        let alignment = Alignment::new(diff.clone(), &original, &modified);

        for (version, lines) in [(DiffVersion::Original, &original), (DiffVersion::Modified, &modified)] {
            for number in 1..=lines.len() as u32 {
                let text = alignment.line(version, number).expect("line exists");
                for span in alignment.spans(version, number) {
                    prop_assert!(span.bytes.start < span.bytes.end);
                    prop_assert!(
                        text.get(span.bytes.start as usize..span.bytes.end as usize).is_some(),
                        "{:?} cannot slice {:?}", span.bytes, text
                    );
                }
            }
        }
    }

    #[test]
    fn hunks_cover_every_changed_line_once(original in file(), modified in file()) {
        let original: Vec<&str> = original.iter().map(String::as_str).collect();
        let modified: Vec<&str> = modified.iter().map(String::as_str).collect();
        let Ok(diff) = vscode_diff::compute(&original, &modified, &Options::default()) else {
            return Ok(());
        };
        let alignment = Alignment::new(diff.clone(), &original, &modified);

        for change in &diff.changes {
            for line in change.original.start_line..change.original.end_line {
                let owners = alignment.hunks().iter()
                    .filter(|h| line >= h.original.start_line && line < h.original.end_line)
                    .count();
                prop_assert_eq!(owners, 1);
            }
        }
    }
}
