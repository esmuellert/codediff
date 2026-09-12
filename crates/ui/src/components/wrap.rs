use std::ops::Range;

use align::{Alignment, ViewLine, ViewLineContent, ViewLineType};
use file_types::{DiffType, DiffVersion};
use line_index::{CellCol, LineIndex};

const TAB_WIDTH: u8 = 4;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WrappedViewLine {
    pub(crate) original: Vec<TerminalLine>,
    pub(crate) modified: Vec<TerminalLine>,
    pub(crate) diff_type: ViewLineType,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalLine {
    SourceCode { source_line: u32, bytes: Range<u32> },
    Filler,
}

impl WrappedViewLine {
    pub(crate) fn from_view_line(
        view_line: ViewLine,
        original_source: Option<&str>,
        modified_source: Option<&str>,
    ) -> Self {
        Self {
            original: vec![TerminalLine::from_view_line_content(
                view_line.original,
                original_source,
            )],
            modified: vec![TerminalLine::from_view_line_content(
                view_line.modified,
                modified_source,
            )],
            diff_type: view_line.kind,
        }
    }
}

impl TerminalLine {
    pub(crate) fn from_view_line_content(content: ViewLineContent, source: Option<&str>) -> Self {
        match content {
            ViewLineContent::SourceLine(source_line) => Self::SourceCode {
                source_line,
                bytes: 0..source.unwrap_or("").len() as u32,
            },
            ViewLineContent::Filler => Self::Filler,
        }
    }

    fn wrap_source_line(content: ViewLineContent, source: Option<&str>, width: u16) -> Vec<Self> {
        let ViewLineContent::SourceLine(source_line) = content else {
            return Vec::new();
        };
        wrap_ranges(source.unwrap_or(""), width)
            .into_iter()
            .map(|bytes| Self::SourceCode { source_line, bytes })
            .collect()
    }

    pub(crate) fn source_line(&self) -> Option<u32> {
        match self {
            Self::SourceCode { source_line, .. } => Some(*source_line),
            Self::Filler => None,
        }
    }
}

pub(crate) fn unwrapped_view_lines(
    alignment: &Alignment,
    diff_type: DiffType,
) -> Vec<WrappedViewLine> {
    alignment
        .view_lines_from(diff_type, 0)
        .map(|view_line| {
            let original = view_line
                .original
                .line()
                .and_then(|line| alignment.line(DiffVersion::Original, line));
            let modified = view_line
                .modified
                .line()
                .and_then(|line| alignment.line(DiffVersion::Modified, line));
            WrappedViewLine::from_view_line(view_line, original, modified)
        })
        .collect()
}

pub(crate) fn wrap_view_line(
    view_line: ViewLine,
    original_source: Option<&str>,
    modified_source: Option<&str>,
    original_width: u16,
    modified_width: u16,
    layout: DiffType,
) -> WrappedViewLine {
    let (mut original, mut modified) = if layout == DiffType::Inline {
        let modified =
            TerminalLine::wrap_source_line(view_line.modified, modified_source, modified_width);
        let original =
            TerminalLine::wrap_source_line(view_line.original, original_source, original_width);
        if view_line.modified.line().is_some() {
            (
                unwrapped_source_line(view_line.original, original_source),
                modified,
            )
        } else {
            (
                original,
                unwrapped_source_line(view_line.modified, modified_source),
            )
        }
    } else {
        (
            TerminalLine::wrap_source_line(view_line.original, original_source, original_width),
            TerminalLine::wrap_source_line(view_line.modified, modified_source, modified_width),
        )
    };
    let height = original.len().max(modified.len()).max(1);
    original.resize(height, TerminalLine::Filler);
    modified.resize(height, TerminalLine::Filler);
    WrappedViewLine {
        original,
        modified,
        diff_type: view_line.kind,
    }
}

pub(crate) fn wrap_view_lines(
    alignment: &Alignment,
    diff_type: DiffType,
    original_width: u16,
    modified_width: u16,
) -> Vec<WrappedViewLine> {
    alignment
        .view_lines_from(diff_type, 0)
        .map(|view_line| {
            let original_source = view_line
                .original
                .line()
                .and_then(|line| alignment.line(DiffVersion::Original, line));
            let modified_source = view_line
                .modified
                .line()
                .and_then(|line| alignment.line(DiffVersion::Modified, line));
            wrap_view_line(
                view_line,
                original_source,
                modified_source,
                original_width,
                modified_width,
                diff_type,
            )
        })
        .collect()
}

fn unwrapped_source_line(content: ViewLineContent, source: Option<&str>) -> Vec<TerminalLine> {
    let ViewLineContent::SourceLine(source_line) = content else {
        return Vec::new();
    };
    vec![TerminalLine::SourceCode {
        source_line,
        bytes: 0..source.unwrap_or("").len() as u32,
    }]
}

fn wrap_ranges(line: &str, width: u16) -> Vec<Range<u32>> {
    let index = LineIndex::new(line, TAB_WIDTH);
    let graphemes: Vec<_> = index.graphemes().collect();
    if graphemes.is_empty() {
        return vec![0..0];
    }

    let width = u32::from(width.max(1));
    let indent = wrapped_indent(&graphemes, width);
    let leading_end = graphemes
        .iter()
        .position(|grapheme| {
            !grapheme
                .text
                .chars()
                .next()
                .is_some_and(|character| matches!(character, ' ' | '\t'))
        })
        .unwrap_or(graphemes.len());
    let mut ranges = Vec::new();
    let mut start = 0usize;

    while start < graphemes.len() {
        let leading = if start > 0 { indent } else { 0 };
        let mut used = leading;
        let mut last_break = None;
        let mut previous: Option<line_index::Grapheme<'_>> = None;
        let mut at = start;

        while at < graphemes.len() {
            let current = graphemes[at];
            if previous
                .is_some_and(|previous| is_line_break_allowed_before(previous.text, current.text))
                && !(start == 0 && at == leading_end)
            {
                last_break = (at > start).then_some(at);
            }

            let advance = grapheme_cell_width(current.text, used, TAB_WIDTH);
            if used.saturating_add(advance) > width {
                let break_at = last_break.unwrap_or_else(|| if at > start { at } else { at + 1 });
                let end = graphemes[break_at - 1];
                ranges.push(graphemes[start].byte.get()..end.byte.get() + end.text.len() as u32);
                start = break_at;
                break;
            }

            used = used.saturating_add(advance);
            previous = Some(current);
            at += 1;
        }

        if at == graphemes.len() {
            let end = graphemes[graphemes.len() - 1];
            ranges.push(graphemes[start].byte.get()..end.byte.get() + end.text.len() as u32);
            break;
        }
    }

    ranges
}

fn wrapped_indent(graphemes: &[line_index::Grapheme<'_>], width: u32) -> u32 {
    let mut indent = 0u32;
    for grapheme in graphemes {
        let Some(character) = grapheme.text.chars().next() else {
            break;
        };
        if !matches!(character, ' ' | '\t') {
            break;
        }
        indent = indent.saturating_add(grapheme_cell_width(grapheme.text, indent, TAB_WIDTH));
    }
    if indent.saturating_add(1) > width {
        0
    } else {
        indent
    }
}

fn grapheme_cell_width(text: &str, column: u32, tab_width: u8) -> u32 {
    if text == "\t" {
        line_index::tab_advance(CellCol(column), tab_width)
    } else {
        line_index::grapheme_width(text)
    }
}

fn is_line_break_allowed_before(previous: &str, current: &str) -> bool {
    let Some(current) = current.chars().next() else {
        return false;
    };
    if current == ' ' {
        return false;
    }

    let previous_position = previous.chars().next().map_or(0, break_position);
    let current_position = break_position(current);
    (previous_position == BREAK_AFTER && current_position != BREAK_AFTER)
        || (previous_position != BREAK_BEFORE && current_position == BREAK_BEFORE)
        || (previous_position == BREAK_IDEOGRAPHIC && current_position != BREAK_AFTER)
        || (current_position == BREAK_IDEOGRAPHIC && previous_position != BREAK_BEFORE)
}

const BREAK_BEFORE: u8 = 1;
const BREAK_AFTER: u8 = 2;
const BREAK_IDEOGRAPHIC: u8 = 3;

fn break_position(character: char) -> u8 {
    if "([{‘“〈《「『【〔（［｛｢£¥＄￡￥+＋".contains(character) {
        BREAK_BEFORE
    } else if " \t})]?|/&.,;¢°′″‰℃、。｡､￠，．：；？！％・･ゝゞヽヾーァィゥェォッャュョヮヵヶぁぃぅぇぉっゃゅょゎゕゖㇰㇱㇲㇳㇴㇵㇶㇷㇸㇹㇺㇻㇼㇽㇾㇿ々〻ｧｨｩｪｫｬｭｮｯｰ”〉》」』】〕）］｝｣".contains(character) {
        BREAK_AFTER
    } else if is_ideographic(character) {
        BREAK_IDEOGRAPHIC
    } else {
        0
    }
}

fn is_ideographic(character: char) -> bool {
    matches!(
        character as u32,
        0x3040..=0x30ff | 0x3400..=0x4dbf | 0x4e00..=0x9fff
    )
}

fn selected_terminal_line<'a>(
    original: &'a TerminalLine,
    modified: &'a TerminalLine,
) -> Option<&'a TerminalLine> {
    match modified {
        TerminalLine::SourceCode { .. } => Some(modified),
        TerminalLine::Filler => match original {
            TerminalLine::SourceCode { .. } => Some(original),
            TerminalLine::Filler => None,
        },
    }
}

pub(crate) fn terminal_line_cells(line: &TerminalLine, source_lines: &[String]) -> u32 {
    let TerminalLine::SourceCode { source_line, bytes } = line else {
        return 0;
    };
    source_lines
        .get(source_line.saturating_sub(1) as usize)
        .and_then(|source| source.get(bytes.start as usize..bytes.end as usize))
        .map(|text| LineIndex::new(text, TAB_WIDTH).width().0)
        .unwrap_or(0)
}

pub(crate) fn longest_terminal_line_cells(
    lines: &[WrappedViewLine],
    version: DiffVersion,
    source_lines: &[String],
) -> u32 {
    lines
        .iter()
        .flat_map(|line| match version {
            DiffVersion::Original => line.original.iter(),
            DiffVersion::Modified => line.modified.iter(),
        })
        .map(|line| terminal_line_cells(line, source_lines))
        .max()
        .unwrap_or(0)
}

pub(crate) fn find_terminal_line_index(
    lines: &[WrappedViewLine],
    target: &TerminalLine,
) -> Option<u32> {
    let exact = lines
        .iter()
        .flat_map(|line| line.original.iter().zip(&line.modified))
        .position(|(original, modified)| {
            selected_terminal_line(original, modified) == Some(target)
        });
    exact
        .or_else(|| {
            lines
                .iter()
                .flat_map(|line| line.original.iter().zip(&line.modified))
                .position(|(original, modified)| {
                    let Some(fragment) = selected_terminal_line(original, modified) else {
                        return false;
                    };
                    let (
                        TerminalLine::SourceCode {
                            source_line: fragment_line,
                            bytes: fragment_bytes,
                        },
                        TerminalLine::SourceCode {
                            source_line: saved_line,
                            bytes: saved_bytes,
                        },
                    ) = (fragment, target)
                    else {
                        return false;
                    };
                    if fragment_line != saved_line {
                        return false;
                    }
                    if saved_bytes.start == saved_bytes.end {
                        fragment_bytes.start <= saved_bytes.start
                            && saved_bytes.start <= fragment_bytes.end
                    } else {
                        fragment_bytes.start <= saved_bytes.start
                            && saved_bytes.start < fragment_bytes.end
                    }
                })
        })
        .map(|index| index as u32)
}

pub(crate) fn wrapped_view_line_range_for_terminal_lines(
    lines: &[WrappedViewLine],
    start: u32,
    end: u32,
) -> Range<usize> {
    let mut offset = 0;
    let first = lines
        .iter()
        .position(|line| {
            offset += line.original.len() as u32;
            offset > start
        })
        .unwrap_or(lines.len());

    let mut offset = 0;
    let last = lines
        .iter()
        .position(|line| {
            let line_start = offset;
            offset += line.original.len() as u32;
            line_start >= end
        })
        .unwrap_or(lines.len());

    first..last
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wraps_both_sides_and_pads_the_shorter_side() {
        let view_line = ViewLine {
            original: ViewLineContent::SourceLine(1),
            modified: ViewLineContent::SourceLine(1),
            kind: ViewLineType::Modified,
        };
        let wrapped = wrap_view_line(
            view_line,
            Some("abcdef"),
            Some("abc"),
            3,
            3,
            DiffType::SideBySide,
        );

        assert_eq!(
            wrapped.original,
            vec![
                TerminalLine::SourceCode {
                    source_line: 1,
                    bytes: 0..3,
                },
                TerminalLine::SourceCode {
                    source_line: 1,
                    bytes: 3..6,
                },
            ]
        );
        assert_eq!(
            wrapped.modified,
            vec![
                TerminalLine::SourceCode {
                    source_line: 1,
                    bytes: 0..3,
                },
                TerminalLine::Filler,
            ]
        );
    }

    #[test]
    fn expands_a_logical_filler_to_the_other_side_height() {
        let view_line = ViewLine {
            original: ViewLineContent::Filler,
            modified: ViewLineContent::SourceLine(2),
            kind: ViewLineType::Inserted,
        };
        let wrapped = wrap_view_line(view_line, None, Some("abcdef"), 3, 3, DiffType::SideBySide);

        assert_eq!(
            wrapped.original,
            vec![TerminalLine::Filler, TerminalLine::Filler]
        );
        assert_eq!(
            wrapped.modified,
            vec![
                TerminalLine::SourceCode {
                    source_line: 2,
                    bytes: 0..3,
                },
                TerminalLine::SourceCode {
                    source_line: 2,
                    bytes: 3..6,
                },
            ]
        );
    }

    #[test]
    fn inline_wraps_the_visible_side_and_keeps_the_hidden_side_unwrapped() {
        let view_line = ViewLine {
            original: ViewLineContent::SourceLine(1),
            modified: ViewLineContent::SourceLine(1),
            kind: ViewLineType::Modified,
        };
        let inline = wrap_view_line(
            view_line,
            Some("abcdef"),
            Some("abc"),
            3,
            3,
            DiffType::Inline,
        );

        assert_eq!(
            inline.original,
            vec![TerminalLine::SourceCode {
                source_line: 1,
                bytes: 0..6,
            }]
        );
        assert_eq!(
            inline.modified,
            vec![TerminalLine::SourceCode {
                source_line: 1,
                bytes: 0..3,
            }]
        );
    }

    #[test]
    fn wraps_an_alignment_into_wrapped_view_lines() {
        let original = ["abcdef"];
        let modified = ["abc"];
        let diff = pipeline::diff::compute(&original, &modified).unwrap();
        let alignment = pipeline::diff::align(diff, &original, &modified).unwrap();
        let wrapped = wrap_view_lines(&alignment, DiffType::SideBySide, 3, 3);

        assert_eq!(wrapped.len(), 1);
        assert_eq!(wrapped[0].original.len(), 2);
        assert_eq!(wrapped[0].modified.len(), 2);
    }

    #[test]
    fn no_wrap_keeps_a_source_line_in_one_descriptor() {
        let view_line = ViewLine {
            original: ViewLineContent::SourceLine(1),
            modified: ViewLineContent::Filler,
            kind: ViewLineType::Deleted,
        };
        let wrapped = WrappedViewLine::from_view_line(view_line, Some("abcdef"), None);

        assert_eq!(
            wrapped.original,
            vec![TerminalLine::SourceCode {
                source_line: 1,
                bytes: 0..6,
            }]
        );
        assert_eq!(wrapped.modified, vec![TerminalLine::Filler]);
    }

    #[test]
    fn finds_a_terminal_line_in_the_flattened_rows() {
        let wrapped = WrappedViewLine {
            original: vec![
                TerminalLine::SourceCode {
                    source_line: 1,
                    bytes: 0..3,
                },
                TerminalLine::SourceCode {
                    source_line: 1,
                    bytes: 3..6,
                },
            ],
            modified: vec![
                TerminalLine::SourceCode {
                    source_line: 1,
                    bytes: 0..3,
                },
                TerminalLine::Filler,
            ],
            diff_type: ViewLineType::Modified,
        };

        assert_eq!(
            find_terminal_line_index(
                &[wrapped],
                &TerminalLine::SourceCode {
                    source_line: 1,
                    bytes: 3..6,
                },
            ),
            Some(1)
        );
    }

    #[test]
    fn restores_a_terminal_line_inside_a_new_fragment() {
        let wrapped = WrappedViewLine {
            original: vec![TerminalLine::SourceCode {
                source_line: 1,
                bytes: 0..6,
            }],
            modified: vec![TerminalLine::Filler],
            diff_type: ViewLineType::Deleted,
        };

        assert_eq!(
            find_terminal_line_index(
                &[wrapped],
                &TerminalLine::SourceCode {
                    source_line: 1,
                    bytes: 3..6,
                },
            ),
            Some(0)
        );
    }

    #[test]
    fn finds_the_wrapped_lines_for_a_terminal_viewport() {
        let lines = vec![
            wrap_view_line(
                ViewLine {
                    original: ViewLineContent::SourceLine(1),
                    modified: ViewLineContent::SourceLine(1),
                    kind: ViewLineType::Modified,
                },
                Some("abcdef"),
                Some("abc"),
                3,
                3,
                DiffType::SideBySide,
            ),
            wrap_view_line(
                ViewLine {
                    original: ViewLineContent::SourceLine(2),
                    modified: ViewLineContent::Filler,
                    kind: ViewLineType::Deleted,
                },
                Some("x"),
                None,
                3,
                3,
                DiffType::SideBySide,
            ),
        ];

        assert_eq!(
            wrapped_view_line_range_for_terminal_lines(&lines, 1, 2),
            0..1
        );
        assert_eq!(
            wrapped_view_line_range_for_terminal_lines(&lines, 2, 3),
            1..2
        );
    }

    #[test]
    fn wraps_at_grapheme_boundaries() {
        let view_line = ViewLine {
            original: ViewLineContent::SourceLine(1),
            modified: ViewLineContent::Filler,
            kind: ViewLineType::Deleted,
        };
        let wrapped = wrap_view_line(view_line, Some("日a"), None, 2, 2, DiffType::SideBySide);

        assert_eq!(
            wrapped.original,
            vec![
                TerminalLine::SourceCode {
                    source_line: 1,
                    bytes: 0..3,
                },
                TerminalLine::SourceCode {
                    source_line: 1,
                    bytes: 3..4,
                },
            ]
        );
    }
}
