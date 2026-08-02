//! Invariants that must hold for any pair of files.
//!
//! The fixtures cover edits somebody thought of. These cover the rest: text is
//! generated from a tiny alphabet so the engine finds real matches and produces
//! genuinely mixed diffs rather than one big replacement.

use align::{Alignment, RowKind, Side};
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

    for row in alignment.rows() {
        // 1. no row is blank on both sides
        prop_assert!(!(row.original.is_filler() && row.modified.is_filler()));

        // 2. a filler appears exactly when the kind says one should
        let has_filler = row.original.is_filler() || row.modified.is_filler();
        prop_assert_eq!(
            has_filler,
            matches!(row.kind, RowKind::Deleted | RowKind::Inserted)
        );

        if let Some(n) = row.original.line() {
            // 3. line numbers advance by one and never repeat
            prop_assert_eq!(n, last_original + 1);
            last_original = n;
            left.push(alignment.line(Side::Original, n).expect("line exists"));
        }
        if let Some(n) = row.modified.line() {
            prop_assert_eq!(n, last_modified + 1);
            last_modified = n;
            right.push(alignment.line(Side::Modified, n).expect("line exists"));
        }

        // 4. an unchanged row really does hold the same text on both sides
        if row.kind == RowKind::Unchanged {
            let (o, m) = row.both().expect("an unchanged row has both sides");
            prop_assert_eq!(
                alignment.line(Side::Original, o),
                alignment.line(Side::Modified, m)
            );
        }
    }

    // 5. each column reads back as the file it came from.
    //    Compared against `lines()` rather than the input, because an empty
    //    file is normalised to a single empty line — the engine's model of one,
    //    and what the diff's line numbers refer to.
    prop_assert_eq!(left.as_slice(), alignment.lines(Side::Original));
    prop_assert_eq!(right.as_slice(), alignment.lines(Side::Modified));

    // 6. the advertised row count is the number of rows produced
    prop_assert_eq!(alignment.row_count() as usize, alignment.rows().count());

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

        prop_assert_eq!(alignment.row_count() as usize, alignment.lines(Side::Original).len());
        for row in alignment.rows() {
            prop_assert_eq!(row.kind, RowKind::Unchanged);
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

        for (side, lines) in [(Side::Original, &original), (Side::Modified, &modified)] {
            for number in 1..=lines.len() as u32 {
                let text = alignment.line(side, number).expect("line exists");
                for span in alignment.spans(side, number) {
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
