//! Two columns showing both versions of a file, paired line by line.

use std::ops::Range;
use std::rc::Rc;

use align::{DiffVersion, ViewLineType};
use file_types::DiffType;
use loom::{
    Basis, Bubble, Column, ColumnProps, Divider, DividerProps, Layout, Listeners, Node,
    Row, RowProps, Scope, component, rsx, use_context,
};
use ratatui::style::Style;

use super::context::Ui;
use super::code_text::{CodeText, CodeTextProps};
use super::filler::{Filler, FillerProps};
use super::gutter::{Gutter, GutterProps};

use crate::hooks::use_scroll::use_scroll;
use crate::services::syntax::SyntaxService;

/// Digits + one trailing space, at least 4 columns.
fn gutter_width(max_line: u32) -> u16 {
    let digits = max_line.max(1).ilog10() + 1;
    (digits as u16).max(3) + 1
}

fn row_styles(
    theme: &crate::theme::Theme,
    kind: ViewLineType,
    version: DiffVersion,
) -> (Style, Style, Style) {
    let base = theme.normal;
    let role = match (kind, version) {
        (ViewLineType::Unchanged, _) => Style::default(),
        (ViewLineType::Modified, _) => match version {
            DiffVersion::Original => theme.deleted,
            DiffVersion::Modified => theme.inserted,
        },
        (ViewLineType::Deleted, _) => theme.deleted,
        (ViewLineType::Inserted, _) => theme.inserted,
    };
    let line_bg = base.patch(role);
    let changed_bg = match (kind, version) {
        (ViewLineType::Modified, DiffVersion::Original) => base.patch(theme.deleted_text),
        (ViewLineType::Modified, DiffVersion::Modified) => base.patch(theme.inserted_text),
        _ => line_bg,
    };
    let number_style = line_bg.patch(theme.line_number);
    (line_bg, changed_bg, number_style)
}

#[component]
pub fn SideBySide(scope: &mut Scope) -> Node {
    let ctx = use_context::<Ui>(scope);
    let theme = &ctx.theme;
    let syntax = ctx.syntax.as_deref();

    let diff = match ctx.diff.as_deref() {
        Some(pipeline::diff::DiffContent::Diff(d)) => d,
        _ => return rsx! { Column { layout: Layout { grow: 1, ..Default::default() }, .. } },
    };

    let alignment = &diff.alignment;
    let total = alignment.view_lines(DiffType::SideBySide).count() as u32;
    let original_lines = alignment.lines(DiffVersion::Original).len() as u32;
    let modified_lines = alignment.lines(DiffVersion::Modified).len() as u32;
    let original_gutter = gutter_width(original_lines);
    let modified_gutter = gutter_width(modified_lines);

    let (view, handle) = use_scroll(scope);

    let pairs: Vec<align::ViewLine> = alignment
        .view_lines_from(DiffType::SideBySide, view.view_lines.start)
        .take(view.view_lines.len())
        .collect();

    let divider_style = theme.normal.patch(theme.divider);

    let listeners = Listeners::new()
        .on_key(move |k| {
            match k {
                k if k == crokey::key!(j) || k == crokey::key!(down) => {
                    handle.down(total);
                    Bubble::Stop
                }
                k if k == crokey::key!(k) || k == crokey::key!(up) => {
                    handle.up(total);
                    Bubble::Stop
                }
                k if k == crokey::key!(left) => {
                    loom::focus_previous();
                    Bubble::Stop
                }
                _ => Bubble::Continue,
            }
        })
        .on_wheel(move |delta| {
            handle.wheel(delta, total);
            Bubble::Stop
        })
        .on_mouse_down(move |mouse| {
            handle.click(mouse.local.y as u32, total);
            Bubble::Stop
        });

    let mut rows: Vec<Node> = Vec::with_capacity(pairs.len());
    for (offset, pair) in pairs.iter().enumerate() {
        let view_line = view.view_lines.start + offset as u32;
        let make_side = |version: DiffVersion, slot: align::Slot, gw: u16| -> Vec<Node> {
            match slot.line() {
                Some(number) => {
                    let (unchanged, changed, number_style) =
                        row_styles(theme, pair.kind, version);
                    let text = alignment.line(version, number).unwrap_or("");
                    let diff_spans: Vec<Range<u32>> = alignment
                        .spans(version, number)
                        .into_iter()
                        .map(|s| s.bytes)
                        .collect();
                    vec![
                        rsx! {
                            Gutter {
                                key: 0u32,
                                number: Some(number),
                                style: number_style,
                                blank: number_style,
                                width: gw,
                            }
                        },
                        rsx! {
                            CodeText {
                                key: 1u32,
                                text: Rc::from(text),
                                diff: Rc::from(diff_spans.as_slice()),
                                syntax: Rc::from(
                                    syntax
                                        .map(|store| SyntaxService::line_spans(store, &diff.file, version, number))
                                        .unwrap_or_default()
                                        .as_slice()
                                ),
                                unchanged_style: unchanged,
                                changed_style: changed,
                                selection: None,
                            }
                        },
                    ]
                }
                None => {
                    let blank = theme.normal.patch(theme.filler);
                    vec![
                        rsx! {
                            Gutter {
                                key: 0u32,
                                number: None,
                                style: blank,
                                blank: blank,
                                width: gw,
                            }
                        },
                        rsx! { Filler { key: 1u32 } },
                    ]
                }
            }
        };

        let left = make_side(DiffVersion::Original, pair.original, original_gutter);
        let right = make_side(DiffVersion::Modified, pair.modified, modified_gutter);

        rows.push(rsx! {
            Row {
                key: view_line,
                layout: Layout { basis: Basis::Length(1), shrink: 0, ..Default::default() },
                ..,
                Row {
                    key: 0u32,
                    layout: Layout { grow: 1, ..Default::default() },
                    ..,
                    { left }
                }
                Divider {
                    key: 1u32,
                    layout: Layout { basis: Basis::Length(1), shrink: 0, ..Default::default() },
                    symbol: "│",
                    style: divider_style,
                    ..
                }
                Row {
                    key: 2u32,
                    layout: Layout { grow: 1, ..Default::default() },
                    ..,
                    { right }
                }
            }
        });
    }

    rsx! {
        Column {
            ref: Some(view.node_ref),
            focusable: true,
            listeners: listeners,
            layout: Layout { grow: 1, fill: Some(theme.normal), ..Default::default() },
            ..,
            { rows }
        }
    }
}
