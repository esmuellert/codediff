//! A box around a pane.
//!
//! Rounded corners. Each pane gets its own box, and two side by side touch:
//! the right edge of one stands in the column before the left edge of the
//! next.

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::Style;

/// The smallest box with anything inside it.
pub const MIN: u16 = 3;

/// Draws a rounded box around the edge of `rect`.
///
/// Nothing is drawn if there is no edge to draw on: a box needs two rows and
/// two columns of its own.
pub fn draw(buf: &mut Buffer, rect: Rect, style: Style) {
    if rect.width < 2 || rect.height < 2 {
        return;
    }
    let (left, right) = (rect.x, rect.right() - 1);
    let (top, bottom) = (rect.y, rect.bottom() - 1);
    for x in left..=right {
        put(buf, x, top, "─", style);
        put(buf, x, bottom, "─", style);
    }
    for y in top..=bottom {
        put(buf, left, y, "│", style);
        put(buf, right, y, "│", style);
    }
    for (x, y, corner) in [
        (left, top, "╭"),
        (right, top, "╮"),
        (left, bottom, "╰"),
        (right, bottom, "╯"),
    ] {
        put(buf, x, y, corner, style);
    }
}

/// What is left of `rect` once its box has taken the edge.
pub fn inner(rect: Rect) -> Rect {
    Rect {
        x: rect.x.saturating_add(1),
        y: rect.y.saturating_add(1),
        width: rect.width.saturating_sub(2),
        height: rect.height.saturating_sub(2),
    }
}

fn put(buf: &mut Buffer, x: u16, y: u16, symbol: &str, style: Style) {
    if let Some(cell) = buf.cell_mut((x, y)) {
        cell.set_symbol(symbol);
        cell.set_style(style);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::style::Color;

    const PLAIN: Style = Style::new();

    fn grid(width: u16, height: u16) -> Buffer {
        Buffer::empty(Rect::new(0, 0, width, height))
    }

    fn rows(buf: &Buffer) -> Vec<String> {
        let area = *buf.area();
        (area.y..area.bottom())
            .map(|y| {
                (area.x..area.right())
                    .map(|x| buf[(x, y)].symbol())
                    .collect()
            })
            .collect()
    }

    #[test]
    fn a_box_is_drawn_round_the_edge_and_leaves_the_middle_alone() {
        let mut buf = grid(5, 4);
        draw(&mut buf, Rect::new(0, 0, 5, 4), PLAIN);
        assert_eq!(rows(&buf), ["╭───╮", "│   │", "│   │", "╰───╯"]);
    }

    #[test]
    fn a_box_smaller_than_the_area_leaves_the_rest_untouched() {
        let mut buf = grid(6, 3);
        draw(&mut buf, Rect::new(1, 0, 4, 3), PLAIN);
        assert_eq!(rows(&buf), [" ╭──╮ ", " │  │ ", " ╰──╯ "]);
    }

    #[test]
    fn a_rectangle_with_no_inside_draws_nothing_rather_than_half_a_box() {
        for rect in [Rect::new(0, 0, 1, 4), Rect::new(0, 0, 4, 1)] {
            let mut buf = grid(5, 4);
            draw(&mut buf, rect, PLAIN);
            assert_eq!(rows(&buf), ["     ", "     ", "     ", "     "], "{rect:?}");
        }
    }

    #[test]
    fn two_boxes_with_a_column_between_them_stand_apart() {
        let mut buf = grid(9, 3);
        draw(&mut buf, Rect::new(0, 0, 4, 3), PLAIN);
        draw(&mut buf, Rect::new(5, 0, 4, 3), PLAIN);
        assert_eq!(rows(&buf), ["╭──╮ ╭──╮", "│  │ │  │", "╰──╯ ╰──╯"]);
    }

    #[test]
    fn a_box_carries_the_style_it_was_given() {
        // Which is how the focused pane's box is the brighter of the two.
        let mut buf = grid(9, 3);
        draw(&mut buf, Rect::new(0, 0, 4, 3), Style::new().fg(Color::Red));
        draw(
            &mut buf,
            Rect::new(5, 0, 4, 3),
            Style::new().fg(Color::Blue),
        );
        assert_eq!(buf[(0, 0)].fg, Color::Red);
        assert_eq!(buf[(3, 1)].fg, Color::Red);
        assert_eq!(buf[(5, 0)].fg, Color::Blue);
    }

    #[test]
    fn the_inside_is_the_rectangle_without_its_edge() {
        assert_eq!(inner(Rect::new(4, 2, 10, 6)), Rect::new(5, 3, 8, 4));
        // A box exactly `MIN` across has one row and one column inside it.
        let smallest = inner(Rect::new(0, 0, MIN, MIN));
        assert_eq!((smallest.width, smallest.height), (1, 1));
    }
}
