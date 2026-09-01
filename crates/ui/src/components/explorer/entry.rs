//! One line of the file list, painted onto cells.
//!
//! Entry receives a Node and reads the theme from context. It decides
//! the colours, builds the three sections (indent, body, status), and
//! paints them.

use std::rc::Rc;

use loom::{
    Basis, Canvas, CanvasProps, Layout, Node as LoomNode, Scope, component, rsx, use_context,
};
use ratatui::style::Color;

use super::build::Node;
use crate::components::cells;
use crate::components::context::Ui;
use crate::theme::icon::{self, Icon};

fn cell_width(s: &str) -> u16 {
    line_index::LineIndex::new(s, 1).width().0 as u16
}

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

struct Indent {
    markers: String,
}

struct Body {
    icon: Option<Icon>,
    text: String,
    text_color: Color,
    suffix: Vec<(String, Color)>,
}

struct Status {
    added: u32,
    removed: u32,
    symbol: &'static str,
    symbol_color: Color,
    added_color: Color,
    removed_color: Color,
}

#[component]
pub fn Entry(scope: &mut Scope, node: Node, selected: bool) -> LoomNode {
    let theme = use_context::<Ui>(scope).theme;
    let base = if *selected {
        theme.normal.patch(theme.cursor_line)
    } else {
        theme.normal
    };

    let node = node.clone();

    rsx! {
        Canvas {
            layout: Layout { basis: Basis::Length(1), shrink: 0, fill: Some(base), ..Default::default() },
            paint: Rc::new(move |paint: &mut loom::Paint<'_>| {
                let area = paint.area();
                let width = area.width;
                cells::fill(paint.cells(), area, base);

                let (indent, body, status): (Indent, Body, Option<Status>) = match &node {
                    Node::Heading { name, count, added, removed } => {
                        let mut suffix = Vec::new();
                        if *added == 0 && *removed == 0 {
                            suffix.push((format!(" ({count})"), theme.tree.count));
                        } else {
                            suffix.push((format!(" ({count} · "), theme.tree.count));
                            if *added > 0 {
                                suffix.push((format!("+{added}"), theme.change.gained));
                            }
                            if *added > 0 && *removed > 0 {
                                suffix.push((" ".to_string(), theme.tree.count));
                            }
                            if *removed > 0 {
                                suffix.push((format!("-{removed}"), theme.change.lost));
                            }
                            suffix.push((")".to_string(), theme.tree.count));
                        }
                        (
                            Indent { markers: String::new() },
                            Body {
                                icon: None,
                                text: name.to_string(),
                                text_color: theme.tree.heading,
                                suffix,
                            },
                            None,
                        )
                    }
                    Node::Directory { indent, name, open, .. } => {
                        (
                            Indent { markers: indent.clone() },
                            Body {
                                icon: Some(icon::folder(*open)),
                                text: name.clone(),
                                text_color: theme.tree.directory,
                                suffix: Vec::new(),
                            },
                            None,
                        )
                    }
                    Node::File { indent, name, file } => {
                        let change = file.get_change_type();
                        let stats = file.get_stats().filter(|s| !s.is_empty());
                        let suffix = file.previous_path()
                            .map(|p| vec![(format!(" ← {p}"), theme.tree.previous)])
                            .unwrap_or_default();
                        (
                            Indent { markers: indent.clone() },
                            Body {
                                icon: Some(icon::file(name)),
                                text: name.clone(),
                                text_color: theme.tree.name,
                                suffix,
                            },
                            Some(Status {
                                added: stats.map_or(0, |s| s.added),
                                removed: stats.map_or(0, |s| s.removed),
                                symbol: super::letter(change),
                                symbol_color: theme.change.of(change),
                                added_color: theme.change.gained,
                                removed_color: theme.change.lost,
                            }),
                        )
                    }
                };

                // Section 1: indent.
                let indent_width = cell_width(&indent.markers);
                let mut at = cells::write(paint.cells(), area, 0, &indent.markers, base.fg(theme.tree.marker));

                // Section 3: status — measure first to know how much body gets.
                let status_width = if let Some(ref st) = status {
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
