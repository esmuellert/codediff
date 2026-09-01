//! VS Code diff decorations resolved for one terminal line.
//!
//! This is the fixed-width counterpart of `DiffEditorDecorations` at the VS
//! Code revision pinned by the parity verifier. Line and gutter backgrounds
//! come from each non-empty side of a line mapping. Character decorations are
//! whole-line for pure insertions/deletions and otherwise come from the
//! mapping's inner ranges.

use std::ops::Range;

use diff_types::{CharRange, DetailedLineRangeMapping, LineRange, LinesDiff};
use file_types::DiffVersion;
use line_index::{LineIndex, Utf16Col};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CharacterDecoration {
    /// Byte offsets into the line's UTF-8 text.
    pub bytes: Range<u32>,
    /// The range crosses this line's line break, so its colour continues to
    /// the edge of the terminal row.
    pub fill_to_edge: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct LineDecorations {
    pub line_background: bool,
    pub gutter_background: bool,
    pub characters: Vec<CharacterDecoration>,
    /// Byte positions where VS Code draws its three-pixel empty-range marker.
    pub empty_markers: Vec<u32>,
}

pub(crate) fn decorations(
    diff: &LinesDiff,
    lines: &[String],
    tab_width: u8,
    version: DiffVersion,
    line: u32,
) -> LineDecorations {
    let mut out = LineDecorations::default();
    let Some(text) = line.checked_sub(1).and_then(|n| lines.get(n as usize)) else {
        return out;
    };
    let index = LineIndex::new(text, tab_width);

    for change in &diff.changes {
        let changed = line_range(change, version);
        if contains(changed, line) {
            out.line_background = true;
            out.gutter_background = true;
        }

        if change.original.is_empty() || change.modified.is_empty() {
            if contains(changed, line) {
                out.characters.push(CharacterDecoration {
                    bytes: 0..text.len() as u32,
                    fill_to_edge: true,
                });
            }
            continue;
        }

        for inner in &change.inner_changes {
            let range = char_range(inner, version);
            // DiffEditorDecorations deliberately tests the range's start, not
            // every line it covers, before adding the model decoration.
            if !contains(changed, range.start_line) {
                continue;
            }
            if is_empty(range) {
                if line == range.start_line {
                    let byte = index
                        .utf16_to_byte(Utf16Col::from_engine(range.start_col))
                        .get();
                    out.empty_markers.push(byte);
                }
                continue;
            }
            if let Some(character) =
                character_decoration_on_line(range, line, line as usize == lines.len(), &index)
            {
                out.characters.push(character);
            }
        }
    }

    out.characters.sort_by_key(|character| {
        (
            character.bytes.start,
            character.bytes.end,
            character.fill_to_edge,
        )
    });
    out.empty_markers.sort_unstable();
    out
}

fn character_decoration_on_line(
    range: &CharRange,
    line: u32,
    last_line: bool,
    index: &LineIndex<'_>,
) -> Option<CharacterDecoration> {
    if line < range.start_line || line > range.end_line {
        return None;
    }
    let from = if line == range.start_line {
        Utf16Col::from_engine(range.start_col)
    } else {
        Utf16Col::ZERO
    };
    let to = if line == range.end_line {
        Utf16Col::from_engine(range.end_col)
    } else {
        index.utf16_len()
    };
    let bytes = index.utf16_range_to_bytes(from..to);
    let bytes = bytes.start.get()..bytes.end.get();
    let has_characters = !bytes.is_empty();
    let fill_to_edge = line < range.end_line
        || (has_characters && to >= index.utf16_len() && (range.start_col == 1 || last_line));
    if !has_characters && !fill_to_edge {
        return None;
    }
    Some(CharacterDecoration {
        bytes,
        fill_to_edge,
    })
}

fn line_range(change: &DetailedLineRangeMapping, version: DiffVersion) -> &LineRange {
    match version {
        DiffVersion::Original => &change.original,
        DiffVersion::Modified => &change.modified,
    }
}

fn char_range(mapping: &diff_types::RangeMapping, version: DiffVersion) -> &CharRange {
    match version {
        DiffVersion::Original => &mapping.original,
        DiffVersion::Modified => &mapping.modified,
    }
}

fn contains(range: &LineRange, line: u32) -> bool {
    line >= range.start_line && line < range.end_line
}

fn is_empty(range: &CharRange) -> bool {
    range.start_line == range.end_line && range.start_col == range.end_col
}
