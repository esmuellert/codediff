//! A rounded box around its children.
//!
//! Detects focus through loom's bubbling `on_focus` / `on_blur` — when
//! anything inside the box takes focus, the border brightens.

use std::rc::Rc;

use loom::{
    Bubble, Canvas, CanvasProps, Column, ColumnProps, Edges, Layout, Listeners, Node, Scope, Stack,
    StackProps, component, rsx, use_context, use_state,
};
use ratatui::style::Style;

use super::context::Ui;

fn draw_one_border(paint: &mut loom::Paint<'_>, style: Style) {
    let rect = paint.content_area();
    if rect.width < 2 || rect.height < 2 {
        return;
    }
    let (left, top) = paint.origin();
    let right = left.saturating_add(i64::from(rect.width) - 1);
    let bottom = top.saturating_add(i64::from(rect.height) - 1);
    for x in left..=right {
        set_border_cell(paint, x, top, "─", style);
        set_border_cell(paint, x, bottom, "─", style);
    }
    for y in top..=bottom {
        set_border_cell(paint, left, y, "│", style);
        set_border_cell(paint, right, y, "│", style);
    }
    set_border_cell(paint, left, top, "╭", style);
    set_border_cell(paint, right, top, "╮", style);
    set_border_cell(paint, left, bottom, "╰", style);
    set_border_cell(paint, right, bottom, "╯", style);
}

fn set_border_cell(paint: &mut loom::Paint<'_>, x: i64, y: i64, symbol: &str, style: Style) {
    if let (Ok(x), Ok(y)) = (u16::try_from(x), u16::try_from(y)) {
        paint.set(x, y, symbol, style);
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
            listeners,
            ..,
            Canvas {
                layout: Layout { grow: 1, ..Default::default() },
                paint: Rc::new(move |paint: &mut loom::Paint<'_>| {
                    draw_one_border(paint, border_style);
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
