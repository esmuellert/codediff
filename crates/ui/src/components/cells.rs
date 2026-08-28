//! Putting one line of a file onto one row of the grid.
//!
//! Everything awkward about a terminal lives here: a double-width character
//! that straddles the left edge, a tab whose width depends on what precedes
//! it, an escape sequence the file would like the terminal to obey, and a
//! cluster that would hang off the right edge. Each is answered by drawing a
//! layout, which is always exactly one column, rather than by drawing half of
//! something.

use line_index::{CellCol, LineIndex};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};

use crate::theme::Code;

/// A byte range of the line to draw in the emphasis style.
pub type Emphasis = std::ops::Range<u32>;

/// How a line is coloured: diff background + syntax foreground.
///
/// The diff owns the background (`base` for the row, `emphasis` for inner
/// changes). Syntax is patched on top as foreground only.
#[derive(Debug, Clone, Copy)]
pub struct Ink<'a> {
    /// The whole row, including past the end of the text.
    pub base: Style,
    /// The parts named by `spans`.
    pub emphasis: Style,
    pub spans: &'a [Emphasis],
    /// What the language says each run of the line is, in byte order.
    ///
    /// Empty when the language is unknown, when the reader has switched syntax
    /// off, or when this line has not been coloured yet — all three draw the
    /// same, which is why none of them is a special case here.
    pub syntax: &'a [syntax::Span],
    /// Which colour each of those runs names.
    pub code: &'a Code,
}

/// Draws `line` into a single row.
///
/// `left` is the horizontal scroll in cells. `base` is painted across the
/// whole row first — including past the end of the text — because a diff
/// background that stopped at the last character would make a short changed
/// line look like a ragged stripe rather than a highlighted row. Neovim calls
/// this `hl_eol`, and the plugin sets it.
pub fn paint(buf: &mut Buffer, row: Rect, line: &str, tab_width: u8, left: u32, ink: Ink<'_>) {
    let Ink {
        base,
        emphasis,
        spans,
        syntax,
        code,
    } = ink;
    fill(buf, row, base);
    if row.width == 0 {
        return;
    }

    let index = LineIndex::new(line, tab_width);
    let right = left + u32::from(row.width);

    for g in index.graphemes_in_cells(CellCol(left)..CellCol(right)) {
        let cells = g.cells();
        let byte = g.byte.get();
        let under = if in_any(spans, byte) { emphasis } else { base };
        let style = under.patch(written(syntax, code, byte));

        // A cluster the left edge cuts through, or one the right edge would:
        // its columns are on screen but the character is not drawable there,
        // so the columns get spaces in the right style.
        let clipped_left = cells.start < left;
        let clipped_right = cells.end > right;
        let from = cells.start.max(left);
        let to = cells.end.min(right);

        if clipped_left || clipped_right || g.is_tab() {
            for cell in from..to {
                put(buf, row, cell - left, " ", style);
            }
            continue;
        }

        let symbol = line_index::sanitize(g.text);
        put(buf, row, from - left, &symbol, style);
        // A wide character owns the columns after it. Ratatui expects those to
        // hold an empty symbol; leaving the fill's space there would make the
        // terminal draw one column too many.
        for cell in (from + 1)..to {
            put(buf, row, cell - left, "", style);
        }
    }
}

/// What the language says about the byte at `byte`, as a style to patch on.
///
/// A foreground and modifiers only. `Style::new()` — which changes nothing —
/// is the answer for an uncoloured byte, so a line with no spans at all costs
/// one comparison per character and paints exactly as it did before syntax
/// existed.
fn written(spans: &[syntax::Span], code: &Code, byte: u32) -> Style {
    let Some(span) = spans.iter().find(|span| span.bytes.contains(&byte)) else {
        return Style::new();
    };
    let mut style = match code.pen(span.style.pen) {
        Some(colour) => Style::new().fg(colour),
        // A rule that sets only `markup.bold` has no colour of its own, and
        // must keep the one underneath rather than fall back to a default.
        None => Style::new(),
    };
    for (on, modifier) in [
        (span.style.bold, Modifier::BOLD),
        (span.style.italic, Modifier::ITALIC),
        (span.style.underline, Modifier::UNDERLINED),
        (span.style.strikethrough, Modifier::CROSSED_OUT),
    ] {
        if on {
            style = style.add_modifier(modifier);
        }
    }
    style
}

/// Paints a whole row in one style, text or not.
pub fn fill(buf: &mut Buffer, row: Rect, style: Style) {
    for x in row.x..row.right() {
        if let Some(cell) = buf.cell_mut((x, row.y)) {
            cell.set_symbol(" ");
            cell.set_style(style);
        }
    }
}

/// Repeats one character across a row.
///
/// Fills a row with a repeated symbol. Used where a side has no line at all.
pub fn fill_repeat_pattern(buf: &mut Buffer, row: Rect, symbol: &str, style: Style) {
    for x in row.x..row.right() {
        if let Some(cell) = buf.cell_mut((x, row.y)) {
            cell.set_symbol(symbol);
            cell.set_style(style);
        }
    }
}

/// Draws text at a fixed offset, clipped to the row, with no scrolling.
///
/// For interface text — line numbers, the status line — which we generate and
/// therefore need not neutralise, but which still has to respect wide
/// characters in a path.
pub fn write(buf: &mut Buffer, row: Rect, offset: u16, text: &str, style: Style) -> u16 {
    let index = LineIndex::new(text, 1);
    let mut used = 0;
    for g in index.graphemes() {
        let at = offset + used as u16;
        if at >= row.width {
            break;
        }
        if u32::from(at) + g.width > u32::from(row.width) {
            break;
        }
        put(
            buf,
            row,
            u32::from(at),
            &line_index::sanitize(g.text),
            style,
        );
        for extra in 1..g.width {
            put(buf, row, u32::from(at) + extra, "", style);
        }
        used += g.width;
    }
    offset + used as u16
}

fn put(buf: &mut Buffer, row: Rect, offset: u32, symbol: &str, style: Style) {
    let Ok(offset) = u16::try_from(offset) else {
        return;
    };
    if offset >= row.width {
        return;
    }
    if let Some(cell) = buf.cell_mut((row.x + offset, row.y)) {
        cell.set_symbol(symbol);
        cell.set_style(style);
    }
}

fn in_any(spans: &[Emphasis], byte: u32) -> bool {
    spans.iter().any(|s| s.contains(&byte))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::Theme;
    use ratatui::style::Color;

    const PLAIN: Style = Style::new();

    fn row(width: u16) -> (Buffer, Rect) {
        let area = Rect::new(0, 0, width, 1);
        (Buffer::empty(area), area)
    }

    fn text(buf: &Buffer, area: Rect) -> String {
        (area.x..area.right())
            .map(|x| buf[(x, area.y)].symbol())
            .collect()
    }

    fn plain() -> Ink<'static> {
        Ink {
            base: PLAIN,
            emphasis: PLAIN,
            spans: &[],
            syntax: &[],
            code: &Theme::DARK.code,
        }
    }

    /// Which columns came out emphasised, as a mask to read under the text.
    fn marks(width: u16, line: &str, left: u32, span: Emphasis) -> String {
        let (mut buf, area) = row(width);
        let ink = Ink {
            base: Style::new().bg(Color::Blue),
            emphasis: Style::new().bg(Color::Red),
            spans: &[span],
            syntax: &[],
            code: &Theme::DARK.code,
        };
        paint(&mut buf, area, line, 4, left, ink);
        (area.x..area.right())
            .map(|x| {
                if buf[(x, area.y)].style().bg == Some(Color::Red) {
                    '^'
                } else {
                    '.'
                }
            })
            .collect()
    }

    fn draw(width: u16, line: &str, left: u32) -> String {
        let (mut buf, area) = row(width);
        paint(&mut buf, area, line, 4, left, plain());
        text(&buf, area)
    }

    #[test]
    fn a_plain_line_is_drawn_and_the_rest_of_the_row_filled() {
        assert_eq!(draw(10, "hello", 0), "hello     ");
    }

    #[test]
    fn scrolling_right_drops_columns_from_the_left() {
        assert_eq!(draw(10, "abcdefghij", 3), "defghij   ");
    }

    #[test]
    fn a_tab_becomes_the_spaces_it_measures_as() {
        assert_eq!(draw(10, "\tx", 0), "    x     ");
        assert_eq!(draw(10, "ab\tx", 0), "ab  x     ");
    }

    #[test]
    fn a_wide_character_owns_two_columns() {
        let (mut buf, area) = row(6);
        paint(&mut buf, area, "日本", 4, 0, plain());
        assert_eq!(buf[(0, 0)].symbol(), "日");
        assert_eq!(buf[(1, 0)].symbol(), "", "the continuation column");
        assert_eq!(buf[(2, 0)].symbol(), "本");
        assert_eq!(buf[(3, 0)].symbol(), "");
    }

    #[test]
    fn a_wide_character_cut_by_the_left_edge_becomes_a_space() {
        // Scrolling one column into a two-column character: the second half
        // cannot be drawn, and drawing the whole thing would shift the line.
        assert_eq!(draw(4, "日本", 1), " 本 ");
    }

    #[test]
    fn a_wide_character_cut_by_the_right_edge_becomes_a_space() {
        assert_eq!(draw(3, "日本", 0), "日 ");
    }

    #[test]
    fn an_escape_sequence_is_neutralised_before_it_is_drawn() {
        let drawn = draw(12, "\u{1b}[31mred", 0);
        assert!(!drawn.contains('\u{1b}'), "{drawn:?}");
        assert!(drawn.starts_with('\u{241b}'));
    }

    #[test]
    fn emphasis_covers_exactly_the_bytes_it_was_given() {
        let (mut buf, area) = row(10);
        let ink = Ink {
            base: Style::new().bg(Color::Blue),
            emphasis: Style::new().bg(Color::Red),
            spans: &[1..2, 3..5],
            syntax: &[],
            code: &Theme::DARK.code,
        };
        paint(&mut buf, area, "abcdef", 4, 0, ink);
        let bg = |x| buf[(x, 0)].style().bg;
        assert_eq!(bg(0), Some(Color::Blue));
        assert_eq!(bg(1), Some(Color::Red));
        assert_eq!(bg(2), Some(Color::Blue));
        assert_eq!(bg(3), Some(Color::Red));
        assert_eq!(bg(4), Some(Color::Red));
        assert_eq!(bg(5), Some(Color::Blue));
    }

    #[test]
    fn emphasis_stays_on_its_characters_when_the_line_is_scrolled() {
        // Only the `3` changed, which the engine reports as bytes 20..21.
        let line = "let total = price * 3;";
        assert_eq!(draw(22, line, 0), "let total = price * 3;");
        assert_eq!(marks(22, line, 0, 20..21), "....................^.");
        // Scrolled eight columns: the same byte is now screen column 12.
        assert_eq!(draw(14, line, 8), "l = price * 3;");
        assert_eq!(marks(14, line, 8, 20..21), "............^.");
    }

    #[test]
    fn a_tab_before_the_span_moves_the_columns_but_not_the_bytes() {
        // The tab is one byte and four cells, so byte 3 sits at cell 6.
        assert_eq!(marks(10, "\tabc", 0, 3..4), "......^...");
        assert_eq!(marks(10, "\tabc", 2, 3..4), "....^.....");
    }

    #[test]
    fn a_wide_character_before_the_span_moves_the_columns_but_not_the_bytes() {
        // Each kanji is three bytes and two cells, so byte 6 sits at cell 4.
        assert_eq!(marks(8, "日本x", 0, 6..7), "....^...");
        assert_eq!(marks(8, "日本x", 3, 6..7), ".^......");
    }

    #[test]
    fn the_background_runs_past_the_end_of_a_short_line() {
        // A changed line shorter than the pane must still read as a full row.
        let (mut buf, area) = row(10);
        let marked = Style::new().bg(Color::Green);
        let ink = Ink {
            base: marked,
            emphasis: marked,
            spans: &[],
            syntax: &[],
            code: &Theme::DARK.code,
        };
        paint(&mut buf, area, "ab", 4, 0, ink);
        assert_eq!(buf[(9, 0)].style().bg, Some(Color::Green));
    }

    #[test]
    fn an_empty_line_still_paints_its_background() {
        let (mut buf, area) = row(5);
        let marked = Style::new().bg(Color::Green);
        let ink = Ink {
            base: marked,
            emphasis: marked,
            spans: &[],
            syntax: &[],
            code: &Theme::DARK.code,
        };
        paint(&mut buf, area, "", 4, 0, ink);
        assert_eq!(text(&buf, area), "     ");
        assert_eq!(buf[(0, 0)].style().bg, Some(Color::Green));
    }

    #[test]
    fn scrolling_past_the_end_of_the_line_leaves_the_row_blank() {
        assert_eq!(draw(5, "short", 99), "     ");
    }

    #[test]
    fn writing_interface_text_reports_where_it_stopped() {
        let (mut buf, area) = row(20);
        let end = write(&mut buf, area, 0, "abc", PLAIN);
        assert_eq!(end, 3);
        let end = write(&mut buf, area, end, "de", PLAIN);
        assert_eq!(end, 5);
        assert_eq!(text(&buf, area).trim_end(), "abcde");
    }

    #[test]
    fn interface_text_never_overruns_its_row() {
        let (mut buf, area) = row(4);
        let end = write(&mut buf, area, 0, "abcdefgh", PLAIN);
        assert_eq!(end, 4);
        assert_eq!(text(&buf, area), "abcd");
    }
}
