//! Dividing the screen.
//!
//! Pure arithmetic on rectangles, with no drawing and no ratatui widgets, so
//! that "is the divider in the right place" can be asked of a number rather than
//! of a screenshot.

use align::DiffVersion;
use ratatui::layout::Rect;

/// Where the columns of one pane go.
///
/// Always two. A file that exists on only one side has nothing to compare
/// against, so it is not a diff at all — it is a [`Text`] buffer, drawn by
/// `render::text` in a single column. There is no such thing here as a diff
/// with one column, which is why neither field is optional. VSCode reached the
/// same conclusion and stopped opening a diff editor at all for added,
/// untracked and deleted files: an empty left-hand side "did not provide much
/// value". See D23.
///
/// [`Text`]: crate::view::buffer::Text
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
