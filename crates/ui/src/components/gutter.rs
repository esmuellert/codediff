//! One line number, right-aligned in its own column.

use std::rc::Rc;

use file_types::DiffVersion;
use loom::{Basis, Canvas, CanvasProps, Layout, Node, Paint, Scope, component, rsx};
use ratatui::style::Style;

pub(crate) fn style_for_diff(
    theme: &crate::theme::Theme,
    version: DiffVersion,
    change_background: bool,
) -> Style {
    let change_style = match version {
        DiffVersion::Original => theme.deleted,
        DiffVersion::Modified => theme.inserted,
    };
    let style = if change_background {
        theme.normal.patch(change_style)
    } else {
        theme.normal
    };
    style.patch(theme.line_number)
}

/// Digits + one trailing space, at least 4 columns.
pub(crate) fn width_for_line_count(line_count: u32) -> u16 {
    let digits = line_count.max(1).ilog10() + 1;
    (digits as u16).max(3) + 1
}

#[component]
pub fn Gutter(
    scope: &mut Scope,
    number: Option<u32>,
    style: Style,
    blank: Style,
    width: u16,
) -> Node {
    let number = *number;
    let style = *style;
    let blank = *blank;
    let width = *width;
    let _ = scope;

    rsx! {
        Canvas {
            layout: Layout { basis: Basis::Length(width), shrink: 0, ..Default::default() },
            paint: Rc::new(move |paint: &mut loom::Paint<'_>| {
                paint_gutter(paint, paint.area(), number, style, blank);
            }),
            ..
        }
    }
}

fn paint_gutter(
    paint: &mut Paint<'_>,
    area: ratatui::layout::Rect,
    number: Option<u32>,
    style: Style,
    blank: Style,
) {
    let Some(number) = number else {
        fill_gutter(paint, area, blank);
        return;
    };

    fill_gutter(paint, area, style);
    let label = number.to_string();
    let digits = label.chars().count() as u16;
    let at = area.width.saturating_sub(1).saturating_sub(digits);
    paint.write(area.x.saturating_add(at), area.y, &label, style);
}

fn fill_gutter(paint: &mut Paint<'_>, area: ratatui::layout::Rect, style: Style) {
    for y in area.y..area.bottom() {
        for x in area.x..area.right() {
            paint.set(x, y, " ", style);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::width_for_line_count;

    #[test]
    fn short_files_keep_the_minimum_width() {
        assert_eq!(width_for_line_count(0), 4);
        assert_eq!(width_for_line_count(999), 4);
    }

    #[test]
    fn the_width_grows_with_the_line_count() {
        assert_eq!(width_for_line_count(1_000), 5);
        assert_eq!(width_for_line_count(99_999), 6);
    }
}
