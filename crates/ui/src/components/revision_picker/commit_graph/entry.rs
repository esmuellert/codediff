//! One line of the commit list, painted onto cells.

use std::rc::Rc;

use loom::{
    Basis, Canvas, CanvasProps, Layout, Node as LoomNode, Paint, Scope, component, rsx, use_context,
};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};

use super::build::{GraphCell, HIGHLIGHT_COLOUR};
use crate::Commit;
use crate::components::context::Ui;
use crate::theme::Theme;

fn cell_width(text: &str) -> u16 {
    line_index::LineIndex::new(text, 1).width().0 as u16
}

fn truncate(text: &str, cells: u16) -> String {
    let line = line_index::LineIndex::new(text, 1);
    let full = line.width().0 as u16;
    if full <= cells {
        return text.to_owned();
    }
    if cells == 0 {
        return String::new();
    }
    let keep = cells.saturating_sub(1);
    let end = line.cell_to_byte(line_index::CellCol(u32::from(keep)));
    let mut out = text[..end.0 as usize].to_owned();
    out.push('…');
    out
}

fn fill_entry_background(paint: &mut Paint<'_>, area: Rect, style: Style) {
    for x in area.x..area.right() {
        paint.set(x, area.y, " ", style);
    }
}

fn pipe_style(theme: &Theme, base: Style, index: u8) -> Style {
    if index == HIGHLIGHT_COLOUR {
        let fg = theme.line_number_current.fg.unwrap_or(theme.tree.name);
        return base.fg(fg).add_modifier(Modifier::BOLD);
    }
    let palette = [
        theme.border_focused.fg.unwrap_or(theme.tree.marker),
        theme.change.gained,
        theme.change.modified,
        theme.code.keyword,
        theme.code.property,
        theme.change.untracked,
    ];
    let colour = palette
        .get(usize::from(index))
        .copied()
        .unwrap_or(theme.tree.marker);
    base.fg(colour)
}

fn short_author(author: &str) -> &str {
    if author.contains("github-actions") {
        "GitHub Actions"
    } else if author.contains("copilot") {
        "Copilot"
    } else {
        author
    }
}

fn short_date(date: &str) -> &str {
    if date.len() == 10
        && date.as_bytes().get(4) == Some(&b'-')
        && date.as_bytes().get(7) == Some(&b'-')
    {
        &date[5..]
    } else {
        date
    }
}

fn ref_labels(commit: &Commit, theme: &Theme) -> Vec<(String, Color)> {
    let mut labels = Vec::new();
    let mut branches = commit.branches.iter();

    if commit.current {
        if let Some(first_branch) = branches.next() {
            labels.push((format!("HEAD -> {first_branch}"), theme.tree.heading));
        } else {
            labels.push(("HEAD".to_owned(), theme.tree.heading));
        }
    }

    for branch in branches {
        labels.push((branch.clone(), theme.tree.directory));
    }
    for remote in &commit.remotes {
        labels.push((remote.clone(), theme.tree.directory));
    }
    for tag in &commit.tags {
        labels.push((format!("tag: {tag}"), theme.code.constant));
    }

    labels
}

#[derive(Clone, Copy)]
struct FieldSpan {
    x: u16,
    width: u16,
}

struct EntryColumns {
    text_x: u16,
    text_width: u16,
    author: FieldSpan,
    date: FieldSpan,
    oid: FieldSpan,
}

fn entry_columns(area: Rect, graph_end: u16, has_graph: bool) -> EntryColumns {
    let right_edge = area.right();
    let text_x = if has_graph {
        graph_end.saturating_add(1)
    } else {
        graph_end
    }
    .min(right_edge);
    let available = right_edge.saturating_sub(text_x);

    let oid_width = if available >= 25 { 7 } else { 0 };
    let date_width = if available >= 50 { 5 } else { 0 };
    let author_width = if available >= 58 {
        16
    } else if available >= 35 {
        10
    } else {
        0
    };

    let mut right = right_edge;
    let mut take = |width: u16| -> FieldSpan {
        if width == 0 || right.saturating_sub(text_x) < width {
            return FieldSpan { x: right, width: 0 };
        }
        right = right.saturating_sub(width);
        let span = FieldSpan { x: right, width };
        right = right.saturating_sub(1);
        span
    };

    let oid = take(oid_width);
    let date = take(date_width);
    let author = take(author_width);
    let text_width = right.saturating_sub(text_x);

    EntryColumns {
        text_x,
        text_width,
        author,
        date,
        oid,
    }
}

fn paint_git_graph(
    paint: &mut Paint<'_>,
    area: Rect,
    cells: &[GraphCell],
    theme: &Theme,
    base: Style,
) -> u16 {
    let max_graph_width = if area.width > 24 {
        area.width.saturating_sub(20).min(area.width / 2)
    } else {
        area.width.saturating_sub(1)
    };
    let start_x = area.x.saturating_add(1);
    let right = start_x.saturating_add(max_graph_width).min(area.right());
    for (index, cell) in cells.iter().enumerate() {
        let x = start_x.saturating_add((index as u16).saturating_mul(2));
        if x >= right {
            return right;
        }
        if cell.first != " " {
            paint.set(
                x,
                area.y,
                cell.first,
                pipe_style(theme, base, cell.first_colour),
            );
        }
        if x.saturating_add(1) < right && cell.second != " " {
            paint.set(
                x.saturating_add(1),
                area.y,
                cell.second,
                pipe_style(theme, base, cell.second_colour),
            );
        }
    }
    start_x
        .saturating_add((cells.len() as u16).saturating_mul(2))
        .min(right)
}

fn paint_refs_and_subject(
    paint: &mut Paint<'_>,
    y: u16,
    x: u16,
    budget: u16,
    commit: &Commit,
    theme: &Theme,
    base: Style,
) {
    if budget == 0 {
        return;
    }

    let labels = ref_labels(commit, theme);
    if labels.is_empty() {
        let cut = truncate(&commit.subject, budget);
        paint.write(x, y, &cut, base.fg(theme.tree.name));
        return;
    }

    let total_refs_width: u16 = labels
        .iter()
        .enumerate()
        .map(|(index, (label, _))| {
            let sep = if index == 0 { 0 } else { 3 };
            sep + cell_width(label)
        })
        .sum();
    let refs_budget = if commit.subject.is_empty() || budget < 20 {
        budget
    } else if total_refs_width.saturating_add(10) <= budget {
        total_refs_width
    } else {
        ((u32::from(budget) * 7) / 10)
            .max(12)
            .min(u32::from(budget.saturating_sub(10))) as u16
    };

    let mut at = x;
    let mut used = 0u16;
    for (index, (label, colour)) in labels.iter().enumerate() {
        let remaining = refs_budget.saturating_sub(used);
        if remaining == 0 {
            break;
        }
        if index > 0 {
            let sep = " · ";
            let sep_w = cell_width(sep);
            if remaining <= sep_w {
                let cut = truncate(sep, remaining);
                let written = paint.write(at, y, &cut, base.fg(theme.tree.count));
                at = at.saturating_add(written);
                used = used.saturating_add(written);
                break;
            }
            let written = paint.write(at, y, sep, base.fg(theme.tree.count));
            at = at.saturating_add(written);
            used = used.saturating_add(written);
        }
        let remaining = refs_budget.saturating_sub(used);
        let cut = truncate(label, remaining);
        let written = paint.write(at, y, &cut, base.fg(*colour));
        at = at.saturating_add(written);
        used = used.saturating_add(written);
        if cell_width(label) > remaining {
            break;
        }
    }

    let after_refs = budget.saturating_sub(used);
    if after_refs > 2 && !commit.subject.is_empty() {
        at = at.saturating_add(paint.write(at, y, "  ", base));
        let cut = truncate(&commit.subject, after_refs.saturating_sub(2));
        paint.write(at, y, &cut, base.fg(theme.tree.name));
    }
}

#[component]
pub fn Entry(
    scope: &mut Scope,
    commit: Commit,
    cells: Rc<[GraphCell]>,
    selected: bool,
) -> LoomNode {
    let theme = use_context::<Ui>(scope).theme;
    let selected = *selected;
    let base = if selected {
        theme.normal.patch(theme.cursor_line)
    } else {
        theme.normal
    };
    let commit = commit.clone();
    let cells = Rc::clone(cells);

    rsx! {
        Canvas {
            layout: Layout {
                basis: Basis::Length(1),
                shrink: 0,
                fill: Some(base),
                ..Default::default()
            },
            paint: Rc::new(move |paint: &mut Paint<'_>| {
                let area = paint.area();
                if area.width == 0 || area.height == 0 {
                    return;
                }
                fill_entry_background(paint, area, base);
                if area.width <= 1 {
                    return;
                }

                let graph_end = paint_git_graph(paint, area, &cells, &theme, base);
                let columns = entry_columns(area, graph_end, !cells.is_empty());

                paint_refs_and_subject(
                    paint,
                    area.y,
                    columns.text_x,
                    columns.text_width,
                    &commit,
                    &theme,
                    base,
                );

                if columns.author.width > 0 {
                    let cut = truncate(short_author(&commit.author), columns.author.width);
                    paint.write(
                        columns.author.x,
                        area.y,
                        &cut,
                        base.fg(theme.tree.name),
                    );
                }
                if columns.date.width > 0 {
                    let cut = truncate(short_date(&commit.date), columns.date.width);
                    paint.write(
                        columns.date.x,
                        area.y,
                        &cut,
                        base.fg(theme.tree.heading),
                    );
                }
                if columns.oid.width > 0 {
                    let cut = truncate(commit.oid.as_str(), columns.oid.width);
                    paint.write(
                        columns.oid.x,
                        area.y,
                        &cut,
                        base.fg(theme.tree.previous),
                    );
                }
            }),
            ..
        }
    }
}
