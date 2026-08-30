//! Dividing the screen.
//!
//! Pure arithmetic on rectangles, with no drawing and no ratatui widgets, so
//! that "is the divider in the right place" can be asked of a number rather than
//! of a screenshot.

use align::DiffVersion;
use ratatui::layout::Rect;

/// The two columns of a side-by-side diff pane. Always two — a one-sided
/// file uses a `SingleFile` buffer instead.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Frame {
    pub original: Column,
    /// The single cell of width between them.
    pub divider: Rect,
    pub modified: Column,
}

impl Frame {
    /// The columns to draw, with the side each shows.
    pub fn columns(&self) -> impl Iterator<Item = (DiffVersion, Column)> {
        [
            (DiffVersion::Original, self.original),
            (DiffVersion::Modified, self.modified),
        ]
        .into_iter()
    }
}

/// One column of a pane: its line numbers and its text.
///
/// A place on screen, not a version — [`DiffVersion`] is which file a column
/// shows, and inline mode puts both in one column.
///
/// [`DiffVersion`]: align::DiffVersion
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Column {
    pub gutter: Rect,
    pub text: Rect,
}

impl Column {
    /// The two together, for filling a row's background edge to edge.
    pub fn row(&self, y: u16) -> Rect {
        Rect {
            x: self.gutter.x,
            y,
            width: self.gutter.width + self.text.width,
            height: 1,
        }
    }
}

/// Space taken by the line numbers, given the highest one that can appear.
///
/// Sized to the file rather than fixed, so a short file does not carry six
/// blank columns and a long one does not have its numbers truncated. One
/// trailing space separates the number from the text.
pub fn gutter_width(max_line: u32) -> u16 {
    let digits = max_line.max(1).ilog10() + 1;
    (digits as u16).max(3) + 1
}

/// Splits the screen into the area the tab gets and the status line.
///
/// Screen-level, not pane-level: there is one status line however many panes
/// the tab holds, so no pane can be the thing that decides where it goes.
///
/// Returns `None` if there is no room for both.
pub fn screen(area: Rect) -> Option<(Rect, Rect)> {
    if area.height < 2 {
        return None;
    }
    let status = Rect {
        y: area.y + area.height - 1,
        height: 1,
        ..area
    };
    let body = Rect {
        height: area.height - 1,
        ..area
    };
    Some((body, status))
}

/// Splits the body between a list on the left and what it opened on the right.
///
/// The border is a column of its own, taken off the top before dividing, so
/// widening the list by one widens the list rather than the border.
///
/// Returns `None` if the screen cannot hold both, which the caller draws as
/// one pane rather than as two unusable ones.
pub fn split(area: Rect, left: u16) -> Option<(Rect, Rect, Rect)> {
    // Both sides have a floor, not just the right. Without one on the left the
    // list was squeezed to a single column at 22 columns wide — too narrow to
    // draw, so the whole screen said "terminal too small" while 21 columns
    // showed the list perfectly. A wider terminal must never show less.
    if area.width < MIN_LIST + 1 + MIN_RIGHT {
        return None;
    }
    let left = left.clamp(MIN_LIST, area.width - MIN_RIGHT - 1);
    let list = Rect {
        width: left,
        ..area
    };
    let border = Rect {
        x: area.x + left,
        width: 1,
        ..area
    };
    let rest = Rect {
        x: area.x + left + 1,
        width: area.width - left - 1,
        ..area
    };
    Some((list, border, rest))
}

/// The narrowest a diff is worth drawing beside a list.
///
/// Two of these columns are the box and two the clear ones inside it.
const MIN_RIGHT: u16 = 22;
/// The narrowest a list is worth drawing beside a diff.
///
/// Enough for the box, the clear column each side of it, a cut-off name and
/// the letter beside it. Below this the screen is better used by one pane than
/// by two useless ones.
const MIN_LIST: u16 = 10;

/// Divides one pane into its two columns.
///
/// Returns `None` if the pane is too narrow to draw anything meaningful,
/// which the caller shows as a message rather than drawing a corrupt frame.
pub fn columns(
    area: Rect,
    divider: u16,
    original_lines: u32,
    modified_lines: u32,
) -> Option<Frame> {
    if area.height == 0 {
        return None;
    }

    let left_gutter = gutter_width(original_lines);
    let right_gutter = gutter_width(modified_lines);
    if area.width < left_gutter + right_gutter + 1 + MIN_TEXT * 2 {
        return None;
    }

    // The divider is taken off the top before dividing, so widening the
    // pane by one column widens a column rather than the divider.
    let usable = area.width - 1;
    let left_width = (usable as u32 * divider as u32 / 100) as u16;
    // Both sides must keep room for their numbers plus some text, whatever the
    // divider says. The clamp is applied to the left and the right follows, so
    // the two can never sum to more than the width.
    let left_width = left_width.clamp(left_gutter + MIN_TEXT, usable - right_gutter - MIN_TEXT);
    let right_width = usable - left_width;

    Some(Frame {
        original: column(area.x, area.y, left_width, area.height, left_gutter),
        divider: Rect {
            x: area.x + left_width,
            y: area.y,
            width: 1,
            height: area.height,
        },
        modified: column(
            area.x + left_width + 1,
            area.y,
            right_width,
            area.height,
            right_gutter,
        ),
    })
}

/// Where the two gutters and the one text column of an inline pane go.
///
/// Two gutters because each row shows one line, from one version, and **the
/// empty gutter tells you which version**: no modified number means the line
/// was deleted, no original number means it was inserted. That is why there is
/// no separate sign column — it would repeat what the gutters already say.
/// GitHub and Azure DevOps both show the same two.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InlineFrame {
    pub original: Rect,
    pub modified: Rect,
    pub text: Rect,
}

impl InlineFrame {
    /// The whole row, for filling its background edge to edge.
    pub fn row(&self, y: u16) -> Rect {
        Rect {
            x: self.original.x,
            y,
            width: self.original.width + self.modified.width + self.text.width,
            height: 1,
        }
    }

    /// The gutter for each version, in the order they are drawn.
    pub fn gutters(&self) -> [(DiffVersion, Rect); 2] {
        [
            (DiffVersion::Original, self.original),
            (DiffVersion::Modified, self.modified),
        ]
    }
}

/// Divides one pane into two gutters and the text between them.
///
/// Returns `None` if the pane is too narrow, exactly as [`columns`] does.
pub fn inline(area: Rect, original_lines: u32, modified_lines: u32) -> Option<InlineFrame> {
    if area.height == 0 {
        return None;
    }
    let original = gutter_width(original_lines);
    let modified = gutter_width(modified_lines);
    if area.width < original + modified + MIN_TEXT {
        return None;
    }
    let at = |x: u16, width: u16| Rect {
        x,
        y: area.y,
        width,
        height: area.height,
    };
    Some(InlineFrame {
        original: at(area.x, original),
        modified: at(area.x + original, modified),
        text: at(
            area.x + original + modified,
            area.width - original - modified,
        ),
    })
}

/// The narrowest text a column is worth drawing.
const MIN_TEXT: u16 = 4;

fn column(x: u16, y: u16, width: u16, height: u16, gutter: u16) -> Column {
    Column {
        gutter: Rect {
            x,
            y,
            width: gutter,
            height,
        },
        text: Rect {
            x: x + gutter,
            y,
            width: width - gutter,
            height,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn area(width: u16, height: u16) -> Rect {
        Rect::new(0, 0, width, height)
    }

    /// The body of a full-screen terminal, which is what a single-pane tab
    /// hands to the only pane.
    fn body(width: u16, height: u16) -> Rect {
        screen(area(width, height))
            .expect("room for a status line")
            .0
    }

    #[test]
    fn the_columns_and_divider_exactly_fill_the_width() {
        for width in [20u16, 40, 79, 80, 81, 200] {
            for divider in [15u16, 30, 50, 70, 85] {
                let Some(f) = columns(body(width, 24), divider, 100, 100) else {
                    continue;
                };
                let (left, right) = (f.original, f.modified);
                let total = left.gutter.width
                    + left.text.width
                    + f.divider.width
                    + right.gutter.width
                    + right.text.width;
                assert_eq!(total, width, "{width} cells at {divider}%");
                assert_eq!(left.text.x, left.gutter.right());
                assert_eq!(f.divider.x, left.text.right());
                assert_eq!(right.gutter.x, f.divider.right());
                assert_eq!(right.text.x, right.gutter.right());
            }
        }
    }

    #[test]
    fn neither_side_is_squeezed_out_by_an_extreme_divider() {
        let f = columns(body(80, 24), 85, 10, 10).unwrap();
        assert!(f.modified.text.width >= MIN_TEXT);
        let f = columns(body(80, 24), 15, 10, 10).unwrap();
        assert!(f.original.text.width >= MIN_TEXT);
    }

    #[test]
    fn a_pane_too_small_to_use_is_refused_rather_than_drawn_wrong() {
        assert_eq!(columns(body(10, 24), 50, 10, 10), None);
        assert_eq!(screen(area(80, 1)), None);
        assert!(columns(body(80, 2), 50, 10, 10).is_some());
    }

    #[test]
    fn a_split_tiles_the_body_exactly() {
        for width in [40u16, 61, 80, 200] {
            let (list, border, rest) = split(body(width, 24), 40).expect("room");
            assert_eq!(list.x, 0);
            assert_eq!(border.x, list.right());
            assert_eq!(rest.x, border.right());
            assert_eq!(list.width + border.width + rest.width, width);
        }
    }

    #[test]
    fn a_screen_too_narrow_for_both_is_refused_rather_than_split() {
        // Answered with `None` so the caller can show one pane, instead of a
        // diff four columns wide that says nothing. A screen that is merely
        // tight is not refused — the list gives up columns first.
        assert_eq!(split(body(28, 24), 40), None);
        assert!(split(body(50, 24), 40).is_some(), "tight, but usable");
    }

    #[test]
    fn a_wider_screen_never_shows_less_than_a_narrower_one() {
        // At 22 columns the list was squeezed to one column, too narrow to
        // draw, so the whole screen said "terminal too small" — while 21
        // columns showed the list perfectly.
        let mut previous = 0;
        for width in 1..200u16 {
            let Some((list, _, rest)) = split(body(width, 24), 40) else {
                continue;
            };
            assert!(list.width >= MIN_LIST, "{width} columns gives {list:?}");
            assert!(rest.width >= MIN_RIGHT, "{width} columns gives {rest:?}");
            assert!(
                list.width + rest.width > previous,
                "not monotone at {width}"
            );
            previous = list.width + rest.width;
        }
    }

    #[test]
    fn a_list_wider_than_the_screen_is_pulled_back_rather_than_overflowing() {
        // Reachable by dragging a terminal narrower after widening the list.
        let (list, _, rest) = split(body(80, 24), 200).expect("room");
        assert_eq!(list.width, 57);
        assert_eq!(rest.width, MIN_RIGHT);
    }

    #[test]
    fn the_status_line_is_the_bottom_row_and_the_body_stops_above_it() {
        let (body, status) = screen(area(80, 24)).unwrap();
        assert_eq!(status.y, 23);
        assert_eq!(status.height, 1);
        assert_eq!(status.width, 80);
        assert_eq!(body.y, 0);
        assert_eq!(body.height, 23);
    }

    #[test]
    fn the_two_sides_are_numbered_independently() {
        // A one-line original against a ten-thousand-line modified: the left
        // gutter has no reason to carry the right's width.
        let f = columns(body(80, 24), 50, 1, 10_000).unwrap();
        assert_eq!(f.original.gutter.width, 4);
        assert_eq!(f.modified.gutter.width, 6);
    }

    #[test]
    fn both_columns_get_the_whole_pane_height() {
        let pane = body(80, 24);
        let f = columns(pane, 50, 10, 10).unwrap();
        for (version, column) in f.columns() {
            assert_eq!(column.text.height, pane.height, "{version:?}");
            assert_eq!(column.gutter.height, pane.height, "{version:?}");
        }
    }
}

#[cfg(test)]
mod inline_tests {
    use super::*;

    fn area(width: u16, height: u16) -> Rect {
        Rect::new(0, 0, width, height)
    }

    #[test]
    fn the_two_gutters_are_sized_to_their_own_files() {
        // A change can make one version much longer than the other, and a
        // gutter sized to the wrong one would either waste columns or truncate
        // a number.
        let frame = inline(area(80, 10), 9, 1200).expect("room");
        assert_eq!(frame.original.width, 4, "three digits minimum plus a space");
        assert_eq!(frame.modified.width, 5, "four digits plus a space");
        assert_eq!(frame.text.x, 9, "text starts after both");
        assert_eq!(frame.text.width, 71);
    }

    #[test]
    fn the_gutters_and_the_text_tile_the_pane_exactly() {
        for width in [12u16, 13, 40, 81, 200] {
            let frame = inline(area(width, 5), 30, 30).expect("room");
            assert_eq!(frame.original.x, 0);
            assert_eq!(frame.modified.x, frame.original.width);
            assert_eq!(frame.text.x, frame.original.width + frame.modified.width);
            assert_eq!(frame.row(0).width, width, "width {width} leaves a gap");
        }
    }

    #[test]
    fn a_pane_too_narrow_for_both_gutters_draws_nothing() {
        // Answered with `None` rather than a squeezed frame, so the caller can
        // say so instead of drawing something unreadable.
        assert!(inline(area(11, 5), 30, 30).is_none());
        assert!(inline(area(12, 5), 30, 30).is_some());
        assert!(inline(area(40, 0), 30, 30).is_none());
    }

    #[test]
    fn inline_is_wider_for_text_than_two_columns_of_the_same_pane() {
        // The point of the layout: one text column rather than two, so a long
        // line needs less horizontal scrolling.
        let both = columns(area(80, 10), 50, 30, 30).expect("room");
        let one = inline(area(80, 10), 30, 30).expect("room");
        assert!(one.text.width > both.original.text.width + both.modified.text.width);
    }
}
