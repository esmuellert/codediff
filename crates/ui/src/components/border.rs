//! A rounded box around its children.
//!
//! Detects focus through loom's bubbling `on_focus` / `on_blur` — when
//! anything inside the box takes focus, the border brightens.

use std::rc::Rc;

use loom::{
    Bubble, Canvas, CanvasProps, Column, ColumnProps, Edges, Layout, Listeners, Node, Scope, Stack,
    StackProps, component, rsx, use_context, use_state,
};
use ratatui::layout::Rect;
use ratatui::style::Style;

use super::context::Ui;

fn draw_one_border(buf: &mut ratatui::buffer::Buffer, rect: Rect, style: Style) {
    if rect.width < 2 || rect.height < 2 {
        return;
    }
    let (left, right) = (rect.x, rect.right() - 1);
    let (top, bottom) = (rect.y, rect.bottom() - 1);
    for x in left..=right {
        set_cell(buf, x, top, "─", style);
        set_cell(buf, x, bottom, "─", style);
    }
    for y in top..=bottom {
        set_cell(buf, left, y, "│", style);
        set_cell(buf, right, y, "│", style);
    }
    set_cell(buf, left, top, "╭", style);
    set_cell(buf, right, top, "╮", style);
    set_cell(buf, left, bottom, "╰", style);
    set_cell(buf, right, bottom, "╯", style);
}

fn set_cell(buf: &mut ratatui::buffer::Buffer, x: u16, y: u16, symbol: &str, style: Style) {
    if let Some(cell) = buf.cell_mut((x, y)) {
        cell.set_symbol(symbol);
        cell.set_style(style);
    }
}

#[component]
pub fn Border(scope: &mut Scope, layout: Layout, children: loom::Children) -> Node {
    let theme = use_context::<Ui>(scope).theme;

    let (focused, set_focused) = use_state(scope, || false);
    let on = set_focused;
    let off = on;
    let listeners = Listeners::new()
        .on_focus(move |_| {
            on(&|_| true);
            Bubble::Continue
        })
        .on_blur(move |_| {
            off(&|_| false);
            Bubble::Continue
        });

    let border_style = if focused {
        theme.normal.patch(theme.border_focused)
    } else {
        theme.normal.patch(theme.border)
    };
    let fill = theme.normal;

    let mut outer = *layout;
    if let loom::Basis::Length(w) = outer.basis {
        outer.basis = loom::Basis::Length(w + 4);
    }
    outer.min_width = outer.min_width.max(3);
    outer.min_height = outer.min_height.max(3);
    if outer.fill.is_none() {
        outer.fill = Some(fill);
    }

    rsx! {
        Stack {
            layout: outer,
            listeners: listeners,
            ..,
            Canvas {
                layout: Layout { grow: 1, ..Default::default() },
                paint: Rc::new(move |paint: &mut loom::Paint<'_>| {
                    let area = paint.area();
                    draw_one_border(paint.cells(), area, border_style);
                }),
                ..
            }
            Column {
                layout: Layout { grow: 1, pad: Edges { top: 1, right: 1, bottom: 1, left: 1 }, ..Default::default() },
                ..,
                { children.clone() }
            }
        }
    }
}
