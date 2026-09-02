//! Two columns showing both versions of a file, paired line by line.

use std::ops::Range;
use std::rc::Rc;

use align::{DiffVersion, LineDecorations};
use file_types::DiffType;
use loom::{
    Basis, Column, ColumnProps, Divider, DividerProps, Layout, Node, Row, RowProps, Scope,
    component, rsx, use_context,
};
use ratatui::style::Style;

use super::code_text::{CodeText, CodeTextProps};
use super::context::Ui;
use super::filler::Filler;
use super::gutter::{Gutter, GutterProps, width_for_line_count};

use crate::hooks::use_diff_viewer_navigation::use_diff_viewer_navigation;
use crate::services::syntax::SyntaxService;

fn row_styles(
    theme: &crate::theme::Theme,
    version: DiffVersion,
    decorations: &LineDecorations,
) -> (Style, Style, Style) {
    let base = theme.normal;
    let role = match version {
        DiffVersion::Original => theme.deleted,
        DiffVersion::Modified => theme.inserted,
    };
    let line = if decorations.line_background {
        base.patch(role)
    } else {
        base
    };
    let gutter = if decorations.gutter_background {
        base.patch(role)
    } else {
        base
    };
    let characters = match version {
        DiffVersion::Original => base.patch(theme.deleted_text),
        DiffVersion::Modified => base.patch(theme.inserted_text),
    };
    (line, characters, gutter.patch(theme.line_number))
}

#[component]
pub fn SideBySide(scope: &mut Scope) -> Node {
    let ctx = use_context::<Ui>(scope);
    let theme = &ctx.theme;
    let syntax = ctx.syntax.as_deref();

    let diff = match ctx.diff.as_deref() {
        Some(pipeline::diff::DiffContent::Diff(diff)) => Some(diff),
        _ => None,
    };
    let view_line_count = diff
        .map(|diff| diff.alignment.view_line_count(DiffType::SideBySide))
        .unwrap_or(0);
    let file_key = ctx
        .file
        .as_ref()
        .map(|file| file.path().as_str().to_string());
    let (view, listeners) = use_diff_viewer_navigation(scope, file_key.as_deref(), view_line_count);
    let Some(diff) = diff else {
        return rsx! { Column { layout: Layout { grow: 1, ..Default::default() }, .. } };
    };

    let alignment = &diff.alignment;
    let original_lines = alignment.lines(DiffVersion::Original).len() as u32;
    let modified_lines = alignment.lines(DiffVersion::Modified).len() as u32;
    let original_gutter = width_for_line_count(original_lines);
    let modified_gutter = width_for_line_count(modified_lines);

    let pairs: Vec<align::ViewLine> = alignment
        .view_lines_from(DiffType::SideBySide, view.view_lines.start)
        .take(view.view_lines.len())
        .collect();

    let divider_style = theme.normal.patch(theme.divider);

    let mut rows: Vec<Node> = Vec::with_capacity(pairs.len());
    for (offset, pair) in pairs.iter().enumerate() {
        let view_line = view.view_lines.start + offset as u32;
        let make_side = |version: DiffVersion, slot: align::Slot, gw: u16| -> Vec<Node> {
            match slot.line() {
                Some(number) => {
                    let decorations = alignment.decorations(version, number);
                    let (unchanged, changed, number_style) =
                        row_styles(theme, version, &decorations);
                    let text = alignment.line(version, number).unwrap_or("");
                    let diff_spans: Vec<Range<u32>> = decorations
                        .characters
                        .iter()
                        .map(|character| character.bytes.clone())
                        .collect();
                    let fill_from = decorations
                        .characters
                        .iter()
                        .filter(|character| character.fill_to_edge)
                        .map(|character| character.bytes.start)
                        .min();
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
                                fill_from: fill_from,
                                empty_markers: Rc::from(decorations.empty_markers.as_slice()),
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
