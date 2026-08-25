//! One line of the file list, painted onto cells.

use std::rc::Rc;

use file_types::File;
use loom::{Basis, Canvas, CanvasProps, Layout, Node as LoomNode, Scope, component, rsx, use_context};
use ratatui::style::Modifier;

use crate::components::context::Ui;
use crate::cells;
use crate::theme::icon;

#[derive(Clone)]
pub enum Node {
    Directory { indent: String, name: String, open: bool },
    File { indent: String, name: String, file: File },
}

#[component]
pub fn Entry(scope: &mut Scope, node: Rc<Node>, selected: bool) -> LoomNode {
    let theme = use_context::<Ui>(scope).theme;
    let base = if *selected {
        theme.normal.patch(theme.cursor_line)
    } else {
        theme.normal
    };

    let node = Rc::clone(node);
    let selected = *selected;
    let _ = selected;

    rsx! {
        Canvas {
            layout: Layout { basis: Basis::Length(1), shrink: 0, fill: Some(base), ..Default::default() },
            paint: Rc::new(move |paint: &mut loom::Paint<'_>| {
                let area = paint.area();
                cells::fill(paint.cells(), area, base);

                match node.as_ref() {
                    Node::Directory { indent, name, open } => {
                        let mut at = cells::write(paint.cells(), area, 0, indent, base.fg(theme.tree.marker));
                        let ic = icon::folder(*open);
                        at = cells::write(paint.cells(), area, at, &format!("{} ", ic.glyph), base.fg(ic.color));
                        cells::write(paint.cells(), area, at, name, base.fg(theme.tree.directory));
                    }
                    Node::File { indent, name, file } => {
                        let mut at = cells::write(paint.cells(), area, 0, indent, base.fg(theme.tree.marker));
                        let ic = icon::file(name);
                        at = cells::write(paint.cells(), area, at, &format!("{} ", ic.glyph), base.fg(ic.color));
                        at = cells::write(paint.cells(), area, at, name, base.fg(theme.tree.name));

                        let change = file.get_change_type();
                        let letter = crate::components::explorer::letter(change);
                        let letter_style = base.fg(theme.change.of(change)).add_modifier(Modifier::BOLD);

                        let mut right = String::new();
                        if let Some(stats) = file.get_stats().filter(|s| !s.is_empty()) {
                            if stats.added > 0 {
                                right.push_str(&format!("+{}", stats.added));
                            }
                            if stats.removed > 0 {
                                if !right.is_empty() { right.push(' '); }
                                right.push_str(&format!("-{}", stats.removed));
                            }
                            right.push(' ');
                        }
                        right.push_str(letter);

                        let right_width = right.chars().count() as u16;
                        let right_at = area.width.saturating_sub(right_width);

                        if right_at > at + 1 {
                            cells::write(paint.cells(), area, right_at, &right, letter_style);
                        }
                    }
                }
            }),
            ..
        }
    }
}
