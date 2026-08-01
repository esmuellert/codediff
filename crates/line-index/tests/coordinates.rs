//! Conversion between coordinate systems, checked against hand-computed values
//! and against invariants that must hold for any input.

use line_index::{ByteOff, CellCol, LineIndex, Utf16Col};

const TAB: u8 = 4;

fn m(text: &str) -> LineIndex<'_> {
    LineIndex::new(text, TAB)
}

// ---------------------------------------------------------------------------
// Hand-computed cases. Every number below was worked out from the Unicode
// properties of the characters involved, not recorded from output.
// ---------------------------------------------------------------------------

#[test]
fn ascii_agrees_in_every_coordinate_system() {
    // The reason coordinate bugs survive testing: here they are all equal.
    let line = m("let x = 1;");
    assert_eq!(line.byte_len().get(), 10);
    assert_eq!(line.utf16_len().get(), 10);
    assert_eq!(line.width().get(), 10);

    for i in 0..=10u32 {
        assert_eq!(line.utf16_to_byte(Utf16Col(i)).get(), i);
        assert_eq!(line.byte_to_cell(ByteOff(i)).get(), i);
    }
}

#[test]
fn the_four_systems_diverge_on_mixed_content() {
    // 'a' 1 byte/1 utf16/1 cell; '日' 3/1/2; '🎉' 4/2/2; 'b' 1/1/1.
    let line = m("a日🎉b");
    assert_eq!(line.byte_len().get(), 9);
    assert_eq!(line.utf16_len().get(), 5);
    assert_eq!(line.width().get(), 6);

    // Position of 'b': byte 8, utf16 4, cell 5.
    assert_eq!(line.utf16_to_byte(Utf16Col(4)).get(), 8);
    assert_eq!(line.byte_to_cell(ByteOff(8)).get(), 5);
    assert_eq!(line.byte_to_utf16(ByteOff(8)).get(), 4);
}

#[test]
fn an_astral_character_counts_as_two_utf16_units() {
    let line = m("🎉");
    assert_eq!(line.byte_len().get(), 4);
    assert_eq!(line.utf16_len().get(), 2);
    assert_eq!(line.width().get(), 2);
}

#[test]
fn a_column_inside_a_surrogate_pair_clamps_to_the_character_start() {
    // utf16 column 1 is the low surrogate of '🎉'; no byte begins there.
    let line = m("🎉b");
    assert_eq!(line.utf16_to_byte(Utf16Col(0)).get(), 0);
    assert_eq!(line.utf16_to_byte(Utf16Col(1)).get(), 0);
    assert_eq!(line.utf16_to_byte(Utf16Col(2)).get(), 4);
}

#[test]
fn a_range_covering_half_a_surrogate_pair_still_covers_the_character() {
    // The engine compares individual UTF-16 code units, so it reports changes
    // that begin or end inside a character. '😀' (D83D DE00) and '🨀' (D83E
    // DE00) differ only in their *high* surrogate, and the engine reports
    // exactly that one unit: L1:C1-L1:C2.
    //
    // Rounding both ends down would give 0..0 and the change would be
    // highlighted nowhere at all.
    let line = m("😀");
    let bytes = line.utf16_range_to_bytes(Utf16Col::from_engine(1)..Utf16Col::from_engine(2));
    assert_eq!(bytes.start.get()..bytes.end.get(), 0..4);
    assert_eq!(
        &line.text()[bytes.start.get() as usize..bytes.end.get() as usize],
        "😀"
    );
}

#[test]
fn a_range_over_the_low_surrogate_alone_also_covers_the_character() {
    // 'a' is C1, the emoji spans C2-C3, 'b' is C4. A change reported over the
    // emoji's second unit only must still highlight the whole emoji.
    let line = m("a😀b");
    let bytes = line.utf16_range_to_bytes(Utf16Col::from_engine(3)..Utf16Col::from_engine(4));
    assert_eq!(bytes.start.get()..bytes.end.get(), 1..5);
    assert_eq!(
        &line.text()[bytes.start.get() as usize..bytes.end.get() as usize],
        "😀"
    );
}

#[test]
fn an_empty_range_stays_empty() {
    // A caret between two characters marks no text, and must not be widened
    // into a spurious one-character highlight.
    let line = m("a😀b");
    for col in 0..=5u32 {
        let bytes = line.utf16_range_to_bytes(Utf16Col(col)..Utf16Col(col));
        assert_eq!(
            bytes.start, bytes.end,
            "column {col} should map to an empty range"
        );
    }
}

#[test]
fn a_range_on_whole_characters_is_unchanged() {
    let line = m("a日🎉b");
    let bytes = line.utf16_range_to_bytes(Utf16Col(1)..Utf16Col(4));
    assert_eq!(bytes.start.get()..bytes.end.get(), 1..8);
    assert_eq!(
        &line.text()[bytes.start.get() as usize..bytes.end.get() as usize],
        "日🎉"
    );
}

#[test]
fn a_cell_inside_a_wide_character_clamps_to_its_start() {
    // '日' is drawn across cells 0 and 1; only cell 0 has a byte.
    let line = m("日x");
    assert_eq!(line.cell_to_byte(CellCol(0)).get(), 0);
    assert_eq!(line.cell_to_byte(CellCol(1)).get(), 0);
    assert_eq!(line.cell_to_byte(CellCol(2)).get(), 3);
}

#[test]
fn a_combining_mark_adds_no_width_and_is_not_split() {
    // "e" + COMBINING ACUTE ACCENT: 3 bytes, 2 utf16 units, 1 cell, 1 grapheme.
    let line = m("e\u{0301}x");
    assert_eq!(line.byte_len().get(), 4);
    assert_eq!(line.utf16_len().get(), 3);
    assert_eq!(line.width().get(), 2);
    assert_eq!(line.graphemes().count(), 2);
}

#[test]
fn tab_width_depends_on_the_preceding_columns() {
    assert_eq!(m("\tx").width().get(), 5); // tab at column 0 -> 4 columns
    assert_eq!(m("a\tx").width().get(), 5); // tab at column 1 -> 3 columns
    assert_eq!(m("abc\tx").width().get(), 5); // tab at column 3 -> 1 column
    assert_eq!(m("abcd\tx").width().get(), 9); // tab at column 4 -> 4 columns
}

#[test]
fn tabs_do_not_affect_byte_or_utf16_offsets() {
    let line = m("a\tb");
    assert_eq!(line.byte_len().get(), 3);
    assert_eq!(line.utf16_len().get(), 3);
    assert_eq!(line.width().get(), 5);
    assert_eq!(line.byte_to_cell(ByteOff(2)).get(), 4);
}

#[test]
fn the_engines_one_based_columns_convert_correctly() {
    // `    let x = 1;` — the engine reports column 13 for the '1'.
    let line = m("    let x = 1;");
    let byte = line.utf16_to_byte(Utf16Col::from_engine(13));
    assert_eq!(byte.get(), 12);
    assert_eq!(&line.text()[byte.get() as usize..][..1], "1");
}

#[test]
fn out_of_range_positions_clamp_to_the_end() {
    let line = m("日本");
    assert_eq!(line.utf16_to_byte(Utf16Col(99)).get(), 6);
    assert_eq!(line.byte_to_cell(ByteOff(99)).get(), 4);
    assert_eq!(line.cell_to_byte(CellCol(99)).get(), 6);
}

#[test]
fn a_cell_past_a_line_ending_in_a_zero_width_cluster_clamps_to_the_end() {
    // A line whose last cluster draws nothing shares its final cell with the
    // bytes before it. Rewinding to the first byte at that cell — correct for
    // a cell *inside* the line — would answer a column past the end with the
    // start of the line, sending a horizontal scroll backwards.
    for text in ["\u{fe00}", "e\u{0301}", "a\u{0301}\u{0323}"] {
        let line = m(text);
        let end = line.byte_len();
        assert_eq!(
            line.cell_to_byte(CellCol(99)),
            end,
            "{text:?} should clamp a far column to its end"
        );
        assert_eq!(
            line.cell_to_byte(line.width().saturating_add(1)),
            end,
            "{text:?} should clamp one past its width to its end"
        );
    }
    // The trivial ASCII path has always done this; the two must agree.
    assert_eq!(m("abc").cell_to_byte(CellCol(99)).get(), 3);
}

#[test]
fn an_emoji_zwj_sequence_is_one_grapheme_of_two_cells() {
    // A known point of genuine disagreement, pinned rather than solved.
    //
    // "man + ZWJ + woman + ZWJ + girl" is one grapheme cluster. Unicode TR51
    // says it renders as a single glyph, so `unicode-width` measures it as two
    // columns and so do we. Counting the components separately would give six,
    // and some terminals do exactly that.
    //
    // No implementation is universally right. If a terminal is found where the
    // difference is visible in practice, the fix belongs in a per-terminal
    // width override, not here.
    let line = m("👨\u{200d}👩\u{200d}👧");
    assert_eq!(line.graphemes().count(), 1);
    assert_eq!(line.byte_len().get(), 18);
    assert_eq!(line.utf16_len().get(), 8);
    assert_eq!(line.width().get(), 2);
}

#[test]
fn a_regional_indicator_pair_is_one_flag() {
    // Two regional indicators combine into a single flag glyph.
    let line = m("🇯🇵");
    assert_eq!(line.graphemes().count(), 1);
    assert_eq!(line.width().get(), 2);
}

#[test]
fn an_empty_line_has_zero_extent() {
    let line = m("");
    assert_eq!(line.width().get(), 0);
    assert_eq!(line.byte_len().get(), 0);
    assert_eq!(line.graphemes().count(), 0);
}

// ---------------------------------------------------------------------------
// Horizontal scrolling
// ---------------------------------------------------------------------------

#[test]
fn a_cell_window_yields_the_clusters_it_overlaps() {
    let line = m("abcdef");
    let visible: String = line
        .graphemes_in_cells(CellCol(2)..CellCol(5))
        .map(|g| g.text)
        .collect();
    assert_eq!(visible, "cde");
}

#[test]
fn a_window_starting_inside_a_wide_character_still_includes_it() {
    // '日' spans cells 0-1. A window starting at cell 1 is halfway through it,
    // so it must be reported and the renderer pads the exposed half.
    let line = m("日本x");
    let first = line
        .graphemes_in_cells(CellCol(1)..CellCol(4))
        .next()
        .expect("the straddling cluster is included");
    assert_eq!(first.text, "日");
    assert_eq!(first.cell.get(), 0);
    assert_eq!(first.width, 2);
}

#[test]
fn a_tab_reports_its_expanded_width() {
    let line = m("ab\tc");
    let tab = line
        .graphemes()
        .find(|g| g.is_tab())
        .expect("the line contains a tab");
    assert_eq!(tab.cell.get(), 2);
    assert_eq!(tab.width, 2); // from column 2 to the stop at 4
}
