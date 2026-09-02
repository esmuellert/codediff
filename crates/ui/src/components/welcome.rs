//! The welcome page, shown when no file is selected.

use std::rc::Rc;

use loom::{Canvas, CanvasProps, Layout, Node, Scope, component, rsx, use_context};
use ratatui::layout::Rect;

use super::cells;
use super::context::Ui;

const LOGO: &[&str] = &[
    " ██████╗  ██████╗ ██████╗ ███████╗██████╗ ██╗███████╗███████╗",
    "██╔════╝ ██╔═══██╗██╔══██╗██╔════╝██╔══██╗██║██╔════╝██╔════╝",
    "██║      ██║   ██║██║  ██║█████╗  ██║  ██║██║█████╗  █████╗  ",
    "██║      ██║   ██║██║  ██║██╔══╝  ██║  ██║██║██╔══╝  ██╔══╝  ",
    "╚██████╗ ╚██████╔╝██████╔╝███████╗██████╔╝██║██║     ██║     ",
    " ╚═════╝  ╚═════╝ ╚═════╝ ╚══════╝╚═════╝ ╚═╝╚═╝     ╚═╝     ",
];

const HINT: &str = "Select a file to review.";
const KEYS: &str = "[j/k] Navigate  [Enter] Open  [q] Quit";

#[component]
pub fn Welcome(scope: &mut Scope) -> Node {
    let theme = use_context::<Ui>(scope).theme;
    let base = theme.normal;
    let logo_color = base.fg(theme.tree.heading);
    let hint_color = base.fg(theme.tree.count);

    rsx! {
        Canvas {
            focusable: true,
            layout: Layout { grow: 1, fill: Some(base), ..Default::default() },
            paint: Rc::new(move |paint: &mut loom::Paint<'_>| {
                let area = paint.area();
                cells::fill(paint.cells(), area, base);

                let width = area.width as usize;
                let height = area.height as usize;
                if width == 0 || height == 0 { return; }

                let content_height = LOGO.len() + 3;
                let top = height.saturating_sub(content_height) / 2;

                for (i, line) in LOGO.iter().enumerate() {
                    let y = top + i;
                    if y >= height { break; }
                    let line_width = line.chars().count();
                    let left = width.saturating_sub(line_width) / 2;
                    let row = Rect { x: area.x, y: area.y + y as u16, width: area.width, height: 1 };
                    cells::write(paint.cells(), row, left as u16, line, logo_color);
                }

                let hint_y = top + LOGO.len() + 1;
                if hint_y < height {
                    let left = width.saturating_sub(HINT.len()) / 2;
                    let row = Rect { x: area.x, y: area.y + hint_y as u16, width: area.width, height: 1 };
                    cells::write(paint.cells(), row, left as u16, HINT, hint_color);
                }

                let keys_y = hint_y + 1;
                if keys_y < height {
                    let left = width.saturating_sub(KEYS.len()) / 2;
                    let row = Rect { x: area.x, y: area.y + keys_y as u16, width: area.width, height: 1 };
                    cells::write(paint.cells(), row, left as u16, KEYS, hint_color);
                }
            }),
            ..
        }
    }
}
