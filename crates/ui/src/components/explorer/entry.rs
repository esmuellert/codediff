//! One line of the file list, painted onto cells.
//!
//! Three sections: indent, body, status. Indent is fixed by depth. Status
//! is fixed by content. Body absorbs the rest and truncates with `…`.

use std::rc::Rc;

use loom::{Basis, Canvas, CanvasProps, Layout, Node as LoomNode, Scope, component, rsx, use_context};
use ratatui::style::{Color, Modifier};

use crate::components::cells;
use crate::components::context::Ui;
use crate::theme::icon::Icon;

/// The tree guides to the left of the name.
#[derive(Clone, PartialEq)]
pub struct Indent {
    pub markers: Rc<str>,
}

/// The icon and name.
#[derive(Clone, PartialEq)]
pub struct Body {
    pub icon: Option<Icon>,
    pub text: Rc<str>,
    pub text_color: Color,
    pub bold: bool,
    pub suffix: Vec<(Rc<str>, Color)>,
}

/// Line counts and change letter.
#[derive(Clone, Copy, PartialEq)]
pub struct Status {
    pub added: u32,
    pub removed: u32,
    pub symbol: &'static str,
    pub symbol_color: Color,
    pub added_color: Color,
    pub removed_color: Color,
}

/// Width of a string in terminal cells.
fn cell_width(s: &str) -> u16 {
    line_index::LineIndex::new(s, 1).width().0 as u16
}

/// Truncates `s` to at most `cells` columns and appends `…`.
fn truncate(s: &str, cells: u16) -> String {
    let line = line_index::LineIndex::new(s, 1);
    let full = line.width().0 as u16;
    if full <= cells {
        return s.to_string();
    }
    if cells == 0 {
        return String::new();
    }
    let keep = cells.saturating_sub(1);
    let end = line.cell_to_byte(line_index::CellCol(keep as u32));
    let mut out = s[..end.0 as usize].to_string();
    out.push('…');
    out
}

#[component]
pub fn Entry(
    scope: &mut Scope,
    indent: Indent,
    body: Body,
    status: Option<Status>,
    selected: bool,
) -> LoomNode {
    let theme = use_context::<Ui>(scope).theme;
    let base = if *selected {
        theme.normal.patch(theme.cursor_line)
    } else {
        theme.normal
    };

    let indent = indent.clone();
    let body = body.clone();
    let status = *status;

    rsx! {
        Canvas {
            layout: Layout { basis: Basis::Length(1), shrink: 0, fill: Some(base), ..Default::default() },
            paint: Rc::new(move |paint: &mut loom::Paint<'_>| {
                let area = paint.area();
                let width = area.width;
                cells::fill(paint.cells(), area, base);

                // Section 1: indent.
                let indent_width = cell_width(&indent.markers);
                let mut at = cells::write(paint.cells(), area, 0, &indent.markers, base.fg(theme.tree.marker));

                // Section 3: status — measure first to know how much body gets.
                let status_width = if let Some(st) = status {
                    let mut w = cell_width(st.symbol);
                    if st.added > 0 { w += cell_width(&format!("+{}", st.added)); }
                    if st.removed > 0 {
                        if st.added > 0 { w += 1; }
                        w += cell_width(&format!("-{}", st.removed));
                    }
                    if st.added > 0 || st.removed > 0 { w += 1; }
                    w
                } else {
                    0
                };

                let gap = if status_width > 0 { 1u16 } else { 0 };
                let body_budget = width
                    .saturating_sub(indent_width)
                    .saturating_sub(status_width)
                    .saturating_sub(gap);

                // Section 2: body.
                let (icon_width, text_style) = if let Some(ref ic) = body.icon {
                    let icon_str = format!("{} ", ic.glyph);
                    let w = cell_width(&icon_str);
                    at = cells::write(paint.cells(), area, at, &icon_str, base.fg(ic.color));
                    (w, base.fg(body.text_color))
                } else {
                    (0, base.fg(body.text_color))
                };
                let text_style = if body.bold { text_style.add_modifier(Modifier::BOLD) } else { text_style };

                let name_budget = body_budget.saturating_sub(icon_width);

                let suffix_width: u16 = body.suffix.iter().map(|(t, _)| cell_width(t)).sum();
                let name_width = cell_width(&body.text);

                if !body.suffix.is_empty() {
                    if name_width + suffix_width <= name_budget {
                        at = cells::write(paint.cells(), area, at, &body.text, text_style);
                        for (text, color) in &body.suffix {
                            at = cells::write(paint.cells(), area, at, text, base.fg(*color));
                        }
                    } else if name_width <= name_budget {
                        at = cells::write(paint.cells(), area, at, &body.text, text_style);
                    } else {
                        let cut = truncate(&body.text, name_budget);
                        at = cells::write(paint.cells(), area, at, &cut, text_style);
                    }
                } else {
                    let cut = truncate(&body.text, name_budget);
                    at = cells::write(paint.cells(), area, at, &cut, text_style);
                }

                // Section 3: status — right-aligned.
                if let Some(st) = status {
                    let _ = at;
                    let mut right_at = width.saturating_sub(status_width);

                    if st.added > 0 {
                        let added = format!("+{}", st.added);
                        right_at = cells::write(paint.cells(), area, right_at, &added, base.fg(st.added_color));
                        if st.removed > 0 {
                            right_at = cells::write(paint.cells(), area, right_at, " ", base);
                        }
                    }
                    if st.removed > 0 {
                        let removed = format!("-{}", st.removed);
                        right_at = cells::write(paint.cells(), area, right_at, &removed, base.fg(st.removed_color));
                    }
                    if st.added > 0 || st.removed > 0 {
                        right_at = cells::write(paint.cells(), area, right_at, " ", base);
                    }
                    cells::write(
                        paint.cells(), area, right_at, st.symbol,
                        base.fg(st.symbol_color),
                    );
                }
            }),
            ..
        }
    }
}
