//! Checks the measurement fixture against its hand-verified reference table.
//!
//! `codediff debug measure` shows cell widths, which a human can check against
//! what the terminal actually draws. It cannot show byte or UTF-16 offsets —
//! those are invisible on screen, and they are the ones the diff engine
//! depends on. This test covers them.

use line_index::{ByteOff, DEFAULT_TAB_WIDTH, LineIndex, Utf16Col};

const FIXTURE: &str = include_str!("../fixtures/nasty.txt");
const EXPECTED: &str = include_str!("../fixtures/nasty.expected");

#[derive(Debug, PartialEq, Eq)]
struct ViewLine {
    line: usize,
    bytes: u32,
    utf16: u32,
    cells: u32,
    graphemes: u32,
}

fn reference() -> Vec<ViewLine> {
    EXPECTED
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(|line| {
            let n: Vec<u32> = line
                .split_whitespace()
                .map(|field| field.parse().expect("reference table holds numbers"))
                .collect();
            assert_eq!(n.len(), 5, "expected 5 columns in {line:?}");
            ViewLine {
                line: n[0] as usize,
                bytes: n[1],
                utf16: n[2],
                cells: n[3],
                graphemes: n[4],
            }
        })
        .collect()
}

fn measured() -> Vec<ViewLine> {
    FIXTURE
        .lines()
        .enumerate()
        .map(|(index, text)| {
            let line = LineIndex::new(text, DEFAULT_TAB_WIDTH);
            ViewLine {
                line: index + 1,
                bytes: line.byte_len().get(),
                utf16: line.utf16_len().get(),
                cells: line.width().get(),
                graphemes: line.graphemes().count() as u32,
            }
        })
        .collect()
}

#[test]
fn the_fixture_matches_its_reference_table() {
    let expected = reference();
    let actual = measured();

    assert_eq!(
        expected.len(),
        actual.len(),
        "the reference table has {} rows but the fixture has {} lines",
        expected.len(),
        actual.len()
    );

    for (want, got) in expected.iter().zip(&actual) {
        assert_eq!(
            want,
            got,
            "line {} of nasty.txt no longer measures as recorded:\n  {:?}",
            want.line,
            FIXTURE.lines().nth(want.line - 1).unwrap_or_default()
        );
    }
}

#[test]
fn every_fixture_line_is_internally_consistent() {
    // Totals must equal the sum of the parts, on real-world content rather
    // than the generated input the property tests use.
    for (index, text) in FIXTURE.lines().enumerate() {
        let line = LineIndex::new(text, DEFAULT_TAB_WIDTH);
        let bytes: u32 = line.graphemes().map(|g| g.text.len() as u32).sum();
        let cells: u32 = line.graphemes().map(|g| g.width).sum();

        assert_eq!(bytes, line.byte_len().get(), "line {}", index + 1);
        assert_eq!(cells, line.width().get(), "line {}", index + 1);
        assert_eq!(bytes, text.len() as u32, "line {}", index + 1);
    }
}

/// An independent byte-to-UTF-16 table for one line, built from `std` alone.
///
/// `LineIndex` indexes by grapheme cluster and binary-searches; this walks
/// characters and accumulates `char::len_utf16`. The two share nothing but the
/// input, so agreement is evidence rather than restatement.
fn utf16_oracle(text: &str) -> Vec<(u32, u32)> {
    let mut table = Vec::new();
    let mut utf16 = 0u32;
    for (byte, ch) in text.char_indices() {
        table.push((byte as u32, utf16));
        utf16 += ch.len_utf16() as u32;
    }
    table.push((text.len() as u32, utf16));
    table
}

#[test]
fn every_character_boundary_agrees_with_an_independent_oracle() {
    // The reference table above checks per-line totals, which two different
    // errors can cancel out of. This checks every boundary in between.
    for (index, text) in FIXTURE.lines().enumerate() {
        let line = LineIndex::new(text, DEFAULT_TAB_WIDTH);
        let oracle = utf16_oracle(text);

        assert_eq!(
            line.utf16_len().get(),
            oracle
                .last()
                .expect("a table always has its terminal entry")
                .1,
            "line {} total",
            index + 1
        );

        for &(byte, utf16) in &oracle {
            assert_eq!(
                line.byte_to_utf16(ByteOff(byte)).get(),
                utf16,
                "line {}, byte {byte}",
                index + 1
            );
            assert_eq!(
                line.utf16_to_byte(Utf16Col(utf16)).get(),
                byte,
                "line {}, utf16 {utf16}",
                index + 1
            );
        }
    }
}

#[test]
fn every_engine_range_over_the_fixture_yields_sliceable_bytes() {
    // What S4 will do with the engine's inner-change spans: convert a column
    // range to a byte range and slice the line with it. A range that is not on
    // character boundaries panics; one that collapses highlights nothing.
    for (index, text) in FIXTURE.lines().enumerate() {
        let line = LineIndex::new(text, DEFAULT_TAB_WIDTH);
        let units = line.utf16_len().get();

        for start in 0..=units {
            for end in start..=units {
                let bytes = line.utf16_range_to_bytes(Utf16Col(start)..Utf16Col(end));
                let slice = text.get(bytes.start.get() as usize..bytes.end.get() as usize);
                assert!(
                    slice.is_some(),
                    "line {}, columns {start}..{end} produced {}..{}, which is not on character boundaries",
                    index + 1,
                    bytes.start.get(),
                    bytes.end.get()
                );
                if start < end {
                    assert!(
                        bytes.start < bytes.end,
                        "line {}, columns {start}..{end} collapsed to nothing",
                        index + 1
                    );
                }
            }
        }
    }
}

/// The fixture must keep containing the cases it claims to.
#[test]
fn the_fixture_still_holds_both_forms_of_an_accented_letter() {
    let text = FIXTURE.lines().nth(8).expect("line 9 exists");
    assert!(
        text.contains('\u{00e9}'),
        "line 9 should hold a precomposed U+00E9: {text:?}"
    );
    assert!(
        text.contains("e\u{0301}"),
        "line 9 should hold a decomposed e + U+0301, not the literal text of one: {text:?}"
    );
    let combining = FIXTURE.lines().nth(9).expect("line 10 exists");
    assert!(
        combining.starts_with("e\u{0301}"),
        "line 10 should begin with a base letter carrying a combining acute: {combining:?}"
    );
}
