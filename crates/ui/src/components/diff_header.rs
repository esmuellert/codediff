//! One-line header and floating view-options menu for `DiffViewer`.

use std::rc::Rc;

use file_types::{DiffType, File};
use loom::{
    Basis, Bubble, Canvas, CanvasProps, Column, ColumnProps, Divider, DividerProps, Edges, Gap,
    GapProps, Layout, Listeners, Node, Row, RowProps, Scope, SetState, Stack, StackProps, Text,
    TextProps, component, crossterm::event::MouseButton, rsx, use_context,
};
use ratatui::style::{Modifier, Style};

use super::context::{DirectorySelection, Ui};
use crate::theme::{Theme, icon};

const MENU_INNER_WIDTH: u16 = 22;

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

fn trigger_style(theme: &Theme, open: bool) -> Style {
    if open {
        let mut style = theme
            .normal
            .patch(theme.cursor_line)
            .fg(theme.tree.name)
            .add_modifier(Modifier::UNDERLINED);
        if let Some(colour) = theme.border_focused.fg {
            style = style.underline_color(colour);
        }
        style
    } else {
        theme.normal.fg(theme.tree.previous)
    }
}

fn trigger_button(open: bool, theme: &Theme, set_menu_open: SetState<bool>) -> Node {
    let text = " View ▾ ";
    let width = cell_width(text);
    let style = trigger_style(theme, open);
    let listeners = Listeners::new().on_mouse_down(move |mouse| {
        if mouse.button == Some(MouseButton::Left) {
            set_menu_open.update(|open| *open = !*open);
            Bubble::Stop
        } else {
            Bubble::Continue
        }
    });
    rsx! {
        Row {
            key: 2u32,
            listeners,
            layout: Layout {
                basis: Basis::Length(width),
                shrink: 0,
                fill: Some(style),
                ..Default::default()
            },
            ..,
            Text {
                text: Rc::from(text),
                style,
                layout: Layout {
                    basis: Basis::Length(width),
                    shrink: 0,
                    ..Default::default()
                },
                ..
            }
        }
    }
}

fn file_title(file: &File, theme: &Theme) -> Node {
    let file = file.clone();
    let theme = *theme;
    rsx! {
        Canvas {
            key: 0u32,
            layout: Layout {
                grow: 1,
                shrink: 1,
                clip: true,
                fill: Some(theme.normal),
                ..Default::default()
            },
            paint: Rc::new(move |paint: &mut loom::Paint<'_>| {
                let area = paint.area();
                if area.width == 0 || area.height == 0 {
                    return;
                }
                for x in area.x..area.right() {
                    paint.set(x, area.y, " ", theme.normal);
                }

                let change = file.get_change_type();
                let symbol = format!(" {}  ", super::explorer::letter(change));
                let symbol_style = theme.normal.fg(theme.change.of(change));
                let symbol_width = cell_width(&symbol);
                if area.width <= symbol_width {
                    paint.write(area.x, area.y, &symbol, symbol_style);
                    return;
                }
                let mut at = area
                    .x
                    .saturating_add(paint.write(area.x, area.y, &symbol, symbol_style));
                let path_budget = area.right().saturating_sub(at);

                let path = file.path().as_str();
                let path_style = theme.normal.fg(theme.tree.name);
                let path_width = cell_width(path);
                let previous = file.previous_path().map(|prev| format!(" ← {prev}"));
                let previous_width = previous.as_deref().map_or(0, cell_width);
                let previous_style = theme.normal.fg(theme.tree.previous);

                if let Some(previous) = previous
                    && path_width.saturating_add(previous_width) <= path_budget
                {
                    at = at.saturating_add(paint.write(at, area.y, path, path_style));
                    paint.write(at, area.y, &previous, previous_style);
                } else {
                    let cut = truncate(path, path_budget);
                    paint.write(at, area.y, &cut, path_style);
                }
            }),
            ..
        }
    }
}

fn directory_title(directory: &DirectorySelection, theme: &Theme) -> Node {
    let path = directory.path.as_str().to_owned();
    let count = directory.files.len();
    let theme = *theme;
    rsx! {
        Canvas {
            key: 0u32,
            layout: Layout {
                grow: 1,
                shrink: 1,
                clip: true,
                fill: Some(theme.normal),
                ..Default::default()
            },
            paint: Rc::new(move |paint: &mut loom::Paint<'_>| {
                let area = paint.area();
                if area.width == 0 || area.height == 0 {
                    return;
                }
                for x in area.x..area.right() {
                    paint.set(x, area.y, " ", theme.normal);
                }

                let folder = icon::folder(true);
                let prefix = format!(" {} ", folder.glyph);
                let prefix_style = theme.normal.fg(folder.color);
                let prefix_width = cell_width(&prefix);
                if area.width <= prefix_width {
                    paint.write(area.x, area.y, &prefix, prefix_style);
                    return;
                }
                let mut at = area
                    .x
                    .saturating_add(paint.write(area.x, area.y, &prefix, prefix_style));
                let remaining = area.right().saturating_sub(at);

                let count_text = format!(" ({count})");
                let count_width = cell_width(&count_text);
                let show_count = remaining > count_width.saturating_add(2);
                let path_budget = if show_count {
                    remaining.saturating_sub(count_width)
                } else {
                    remaining
                };

                let cut = truncate(&path, path_budget);
                at = at.saturating_add(paint.write(
                    at,
                    area.y,
                    &cut,
                    theme.normal.fg(theme.tree.directory),
                ));
                if show_count {
                    paint.write(at, area.y, &count_text, theme.normal.fg(theme.tree.count));
                }
            }),
            ..
        }
    }
}

#[component]
pub fn DiffHeader(scope: &mut Scope, menu_open: bool, set_menu_open: SetState<bool>) -> Node {
    let ctx = use_context::<Ui>(scope);
    let theme = &ctx.theme;
    let left = if let Some(directory) = ctx.directory.as_deref() {
        directory_title(directory, theme)
    } else if let Some(file) = ctx.file.as_deref() {
        file_title(file, theme)
    } else {
        return Node::Empty;
    };

    let children = vec![
        left,
        rsx! {
            Gap {
                key: 1u32,
                layout: Layout {
                    basis: Basis::Length(1),
                    shrink: 0,
                    ..Default::default()
                },
                ..
            }
        },
        trigger_button(*menu_open, theme, *set_menu_open),
        rsx! {
            Gap {
                key: 3u32,
                layout: Layout {
                    basis: Basis::Length(1),
                    shrink: 0,
                    ..Default::default()
                },
                ..
            }
        },
    ];

    rsx! {
        Row {
            layout: Layout {
                basis: Basis::Length(1),
                shrink: 0,
                clip: true,
                fill: Some(theme.normal),
                ..Default::default()
            },
            ..,
            { children }
        }
    }
}

fn draw_menu_border(paint: &mut loom::Paint<'_>, style: Style) {
    let area = paint.area();
    if area.width < 2 || area.height < 2 {
        return;
    }
    let left = area.x;
    let top = area.y;
    let right = area.right() - 1;
    let bottom = area.bottom() - 1;
    for x in left..=right {
        paint.set(x, top, "─", style);
        paint.set(x, bottom, "─", style);
    }
    for y in top..=bottom {
        paint.set(left, y, "│", style);
        paint.set(right, y, "│", style);
    }
    paint.set(left, top, "╭", style);
    paint.set(right, top, "╮", style);
    paint.set(left, bottom, "╰", style);
    paint.set(right, bottom, "╯", style);
}

fn menu_row(
    key: u32,
    marker: &'static str,
    marker_active: bool,
    label: &'static str,
    shortcut: &'static str,
    theme: &Theme,
    on_select: impl Fn() + 'static,
) -> Node {
    let theme = *theme;
    let listeners = Listeners::new().on_mouse_down(move |mouse| {
        if mouse.button == Some(MouseButton::Left) {
            on_select();
            Bubble::Stop
        } else {
            Bubble::Continue
        }
    });
    rsx! {
        Canvas {
            key,
            listeners,
            layout: Layout {
                basis: Basis::Length(1),
                shrink: 0,
                fill: Some(theme.normal),
                ..Default::default()
            },
            paint: Rc::new(move |paint: &mut loom::Paint<'_>| {
                let area = paint.area();
                if area.width == 0 || area.height == 0 {
                    return;
                }
                for x in area.x..area.right() {
                    paint.set(x, area.y, " ", theme.normal);
                }

                let marker_style = if marker_active {
                    theme.normal.patch(theme.border_focused)
                } else {
                    theme.normal.fg(theme.tree.previous)
                };
                let label_style = if marker_active {
                    theme.normal.fg(theme.tree.name)
                } else {
                    theme.normal.fg(theme.tree.previous)
                };
                let shortcut_style = theme.normal.fg(theme.tree.previous);

                let prefix = format!(" {marker} ");
                let at = area
                    .x
                    .saturating_add(paint.write(area.x, area.y, &prefix, marker_style));
                paint.write(at, area.y, label, label_style);

                let hint = format!("{shortcut} ");
                let hint_width = cell_width(&hint);
                if area.width > hint_width {
                    let hint_x = area.right().saturating_sub(hint_width);
                    paint.write(hint_x, area.y, &hint, shortcut_style);
                }
            }),
            ..
        }
    }
}

#[component]
pub fn DiffViewMenu(
    scope: &mut Scope,
    view_layout: DiffType,
    set_view_layout: SetState<DiffType>,
    wrap: bool,
    set_wrap: SetState<bool>,
    set_menu_open: SetState<bool>,
) -> Node {
    let ctx = use_context::<Ui>(scope);
    if ctx.directory.is_none() && ctx.file.is_none() {
        return Node::Empty;
    }
    let theme = &ctx.theme;
    let view_layout = *view_layout;
    let set_view_layout = *set_view_layout;
    let wrap = *wrap;
    let set_wrap = *set_wrap;
    let set_menu_open = *set_menu_open;

    let rows = vec![
        menu_row(
            0,
            if view_layout == DiffType::SideBySide {
                "●"
            } else {
                " "
            },
            view_layout == DiffType::SideBySide,
            "Side by side",
            "t",
            theme,
            move || {
                set_view_layout.set(DiffType::SideBySide);
                set_menu_open.set(false);
            },
        ),
        menu_row(
            1,
            if view_layout == DiffType::Inline {
                "●"
            } else {
                " "
            },
            view_layout == DiffType::Inline,
            "Inline",
            "t",
            theme,
            move || {
                set_view_layout.set(DiffType::Inline);
                set_menu_open.set(false);
            },
        ),
        rsx! {
            Divider {
                key: 2u32,
                symbol: "─",
                style: theme.divider,
                layout: Layout {
                    basis: Basis::Length(1),
                    shrink: 0,
                    ..Default::default()
                },
                ..
            }
        },
        menu_row(
            3,
            if wrap { "✓" } else { " " },
            wrap,
            "Wrap lines",
            "w",
            theme,
            move || {
                set_wrap.update(|wrap| *wrap = !*wrap);
                set_menu_open.set(false);
            },
        ),
    ];

    let menu_width = MENU_INNER_WIDTH + 2;
    let menu_height = rows.len() as u16 + 2;
    let border_style = theme.normal.patch(theme.border_focused);
    let backdrop_listeners = Listeners::new()
        .on_mouse_down(move |_| {
            set_menu_open.set(false);
            Bubble::Stop
        })
        .on_wheel(move |_| {
            set_menu_open.set(false);
            Bubble::Stop
        });
    let box_listeners = Listeners::new().on_mouse_down(|_| Bubble::Stop);

    rsx! {
        Column {
            listeners: backdrop_listeners,
            layout: Layout {
                grow: 1,
                ..Default::default()
            },
            ..,
            Gap {
                key: 0u32,
                layout: Layout {
                    basis: Basis::Length(1),
                    shrink: 0,
                    ..Default::default()
                },
                ..
            }
            Row {
                key: 1u32,
                layout: Layout {
                    basis: Basis::Length(menu_height),
                    shrink: 0,
                    ..Default::default()
                },
                ..,
                Gap {
                    key: 0u32,
                    layout: Layout {
                        grow: 1,
                        shrink: 1,
                        ..Default::default()
                    },
                    ..
                }
                Stack {
                    key: 1u32,
                    listeners: box_listeners,
                    layout: Layout {
                        basis: Basis::Length(menu_width),
                        shrink: 0,
                        fill: Some(theme.normal),
                        ..Default::default()
                    },
                    ..,
                    Canvas {
                        key: 0u32,
                        layout: Layout {
                            grow: 1,
                            ..Default::default()
                        },
                        paint: Rc::new(move |paint: &mut loom::Paint<'_>| {
                            draw_menu_border(paint, border_style);
                        }),
                        ..
                    }
                    Column {
                        key: 1u32,
                        layout: Layout {
                            grow: 1,
                            pad: Edges::all(1),
                            ..Default::default()
                        },
                        ..,
                        { rows }
                    }
                }
                Gap {
                    key: 2u32,
                    layout: Layout {
                        basis: Basis::Length(1),
                        shrink: 0,
                        ..Default::default()
                    },
                    ..
                }
            }
            Gap {
                key: 2u32,
                layout: Layout {
                    grow: 1,
                    shrink: 1,
                    ..Default::default()
                },
                ..
            }
        }
    }
}
