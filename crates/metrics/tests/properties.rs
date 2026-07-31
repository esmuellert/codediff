//! Invariants that must hold for any input.
//!
//! The hand-computed cases in `coordinates.rs` say what specific text measures.
//! These say what must be true of *all* text, which is how the cases nobody
//! thought to write down get covered.

use metrics::{ByteOff, CellCol, LineMetrics, Utf16Col};
use proptest::prelude::*;

const TAB: u8 = 4;

fn m(text: &str) -> LineMetrics<'_> {
    LineMetrics::new(text, TAB)
}

// ---------------------------------------------------------------------------

/// Text built from the characters where the coordinate systems disagree.
///
/// `proptest`'s `.` is drawn from the whole of Unicode, but the *arrangements*
/// that break conversions — a range landing inside a surrogate pair, a
/// combining mark trailing a line, a tab after a wide character — need those
/// characters next to each other, which random sampling rarely produces. This
/// makes them common, while the last arm keeps arbitrary text in the mix so
/// the properties still see input nobody thought to list.
fn tricky_text() -> impl Strategy<Value = String> {
    proptest::collection::vec(
        prop_oneof![
            3 => "[a-z ]",
            2 => "[\u{4e00}-\u{9fff}]",   // CJK: two cells, one utf16 unit
            2 => "[\u{1f300}-\u{1f6ff}]", // astral: two utf16 units, two cells
            1 => Just("\t".to_owned()),
            1 => Just("\u{0301}".to_owned()), // combining acute: zero cells
            1 => Just("\u{fe00}".to_owned()), // variation selector: zero cells
            2 => ".{0,3}",                    // anything at all
        ],
        0..24,
    )
    .prop_map(|parts| parts.concat())
}

proptest! {
    #[test]
    fn cell_position_never_decreases(text in tricky_text()) {
        let line = m(&text);
        let mut previous = 0;
        for g in line.graphemes() {
            prop_assert!(g.cell.get() >= previous);
            previous = g.cell.get();
        }
    }

    #[test]
    fn grapheme_positions_are_real_character_boundaries(text in tricky_text()) {
        let line = m(&text);
        for g in line.graphemes() {
            prop_assert!(text.is_char_boundary(g.byte.get() as usize));
            prop_assert_eq!(&text[g.byte.get() as usize..][..g.text.len()], g.text);
        }
    }

    #[test]
    fn totals_equal_the_sum_of_the_parts(text in tricky_text()) {
        let line = m(&text);
        let bytes: u32 = line.graphemes().map(|g| g.text.len() as u32).sum();
        let cells: u32 = line.graphemes().map(|g| g.width).sum();
        prop_assert_eq!(bytes, line.byte_len().get());
        prop_assert_eq!(cells, line.width().get());
        prop_assert_eq!(bytes, text.len() as u32);
    }

    #[test]
    fn utf16_round_trips_at_every_grapheme_boundary(text in tricky_text()) {
        let line = m(&text);
        for g in line.graphemes() {
            prop_assert_eq!(line.utf16_to_byte(g.utf16), g.byte);
            prop_assert_eq!(line.byte_to_utf16(g.byte), g.utf16);
        }
    }

    #[test]
    fn cells_round_trip_at_every_grapheme_boundary(text in tricky_text()) {
        let line = m(&text);
        for g in line.graphemes() {
            prop_assert_eq!(line.byte_to_cell(g.byte), g.cell);
            // Zero-width clusters share a cell with their neighbours, so the
            // reverse direction yields the first byte at that cell rather than
            // this particular one.
            let back = line.cell_to_byte(g.cell);
            prop_assert!(back <= g.byte);
            prop_assert_eq!(line.byte_to_cell(back), g.cell);
        }
    }

    #[test]
    fn cell_to_byte_returns_the_first_byte_at_that_cell(text in tricky_text()) {
        let line = m(&text);
        for g in line.graphemes() {
            let first = line.cell_to_byte(g.cell);
            // No earlier grapheme may also occupy that cell.
            let earlier = line
                .graphemes()
                .filter(|other| other.cell == g.cell)
                .map(|other| other.byte)
                .min();
            prop_assert_eq!(Some(first), earlier);
        }
    }

    #[test]
    fn every_conversion_stays_within_the_line(text in tricky_text(), probe in 0u32..200) {
        let line = m(&text);
        prop_assert!(line.utf16_to_byte(Utf16Col(probe)) <= line.byte_len());
        prop_assert!(line.cell_to_byte(CellCol(probe)) <= line.byte_len());
        prop_assert!(line.byte_to_cell(ByteOff(probe)) <= line.width());
        prop_assert!(line.byte_to_utf16(ByteOff(probe)) <= line.utf16_len());
    }

    #[test]
    fn a_cell_window_holds_exactly_the_clusters_that_overlap_it(text in tricky_text(), start in 0u32..40, len in 1u32..40) {
        let line = m(&text);
        let (from, to) = (start, start + len);
        let got: Vec<_> = line.graphemes_in_cells(CellCol(from)..CellCol(to)).collect();
        // The window binary-searches a starting point rather than walking from
        // column zero. Filtering the whole line is the same answer computed
        // the slow, obvious way, so this checks the shortcut.
        let want: Vec<_> = line
            .graphemes()
            .filter(|g| g.cells().end > from && g.cell.get() < to)
            .collect();
        prop_assert_eq!(got, want);
    }

    #[test]
    fn a_non_empty_utf16_range_never_maps_to_an_empty_byte_range(
        text in tricky_text(), lo in 0u32..30, span in 0u32..5
    ) {
        // Spans are deliberately short. The engine reports inner changes a few
        // units wide, and a one-unit span is the case that breaks: both ends
        // can land inside the same character.
        let line = m(&text);
        let hi = lo + span;
        let range = line.utf16_range_to_bytes(Utf16Col(lo)..Utf16Col(hi));

        prop_assert!(range.start <= range.end);
        prop_assert!(range.end <= line.byte_len());
        // The result must be sliceable, or the renderer panics.
        prop_assert!(text.is_char_boundary(range.start.get() as usize));
        prop_assert!(text.is_char_boundary(range.end.get() as usize));

        if lo < hi && lo < line.utf16_len().get() {
            // Some real text lies in the span, so some real bytes must too —
            // even when both ends land inside one character.
            prop_assert!(range.start < range.end, "{:?} cols {}..{} collapsed", text, lo, hi);
        }
        if lo == hi {
            prop_assert_eq!(range.start, range.end);
        }
    }

    #[test]
    fn ceiling_a_column_never_moves_it_before_the_floor(text in tricky_text(), col in 0u32..80) {
        let line = m(&text);
        let col = Utf16Col(col);
        prop_assert!(line.utf16_to_byte_ceil(col) >= line.utf16_to_byte(col));
        prop_assert!(line.utf16_to_byte_ceil(col) <= line.byte_len());
    }
}
