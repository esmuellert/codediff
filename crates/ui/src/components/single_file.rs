//! One full-width file with no diff decorations.

use std::rc::Rc;

use file_types::DiffType;
use loom::{
    Basis, Column, ColumnProps, Layout, Node, Row, RowProps, Scope, component, rsx, use_context,
    use_memo,
};

use super::code_text::{CodeText, CodeTextProps, longest_line_cells};
use super::context::Ui;
use super::gutter::{Gutter, GutterProps, width_for_line_count};
use crate::hooks::use_diff_viewer_navigation::use_diff_viewer_navigation;
use crate::hooks::use_horizontal_scroll::HorizontalDimensions;
use crate::hooks::use_syntax::use_syntax;
use crate::services::syntax::SyntaxService;

#[component]
pub fn SingleFile(scope: &mut Scope, content: Rc<pipeline::diff::DiffContent>) -> Node {
    let ctx = use_context::<Ui>(scope);
    let pipeline::diff::DiffContent::SingleFile(single) = content.as_ref() else {
        unreachable!("DiffViewer sends one-sided files to SingleFile")
    };
    let line_count = single.lines.len() as u32;
    let file_key = single.file.path().as_str().to_string();
    let gutter_width = width_for_line_count(line_count);
    let content_id = Rc::as_ptr(content) as usize;
    let maximum_line_cells = use_memo(scope, content_id, || longest_line_cells(&single.lines));
    let (view, horizontal, listeners) = use_diff_viewer_navigation(
        scope,
        Some(&file_key),
        line_count,
        HorizontalDimensions::Single {
            longest_line_cells: *maximum_line_cells,
            gutter_cells: gutter_width,
        },
    );
    let syntax = use_syntax(
        scope,
        ctx.syntax_service.as_ref().map(Rc::clone),
        Rc::clone(content),
        DiffType::Single,
        view.view_lines.clone(),
    );
    let version = single.side();
    let syntax = syntax.as_deref();

    let base = ctx.theme.normal;
    let number_style = base.patch(ctx.theme.line_number);
    let visible_lines: Vec<Node> = single
        .lines
        .iter()
        .enumerate()
        .skip(view.view_lines.start as usize)
        .take(view.view_lines.len())
        .map(|(index, text)| {
            let number = index as u32 + 1;
            rsx! {
                Row {
                    key: number,
                    layout: Layout { basis: Basis::Length(1), shrink: 0, ..Default::default() },
                    ..,
                    Gutter {
                        key: 0u32,
                        number: Some(number),
                        style: number_style,
                        blank: base,
                        width: gutter_width,
                    }
                    CodeText {
                        key: 1u32,
                        text: Rc::from(text.as_str()),
                        first_cell: horizontal.first_cell(version),
                        diff: Rc::from([]),
                        fill_from: None,
                        empty_markers: Rc::from([]),
                        syntax: Rc::from(
                            syntax
                                .map(|store| SyntaxService::line_spans(store, &single.file, version, number))
                                .unwrap_or_default()
                                .as_slice()
                        ),
                        unchanged_style: base,
                        changed_style: base,
                        selection: None,
                    }
                }
            }
        })
        .collect();

    rsx! {
        Column {
            ref: Some(view.node_ref),
            focusable: true,
            listeners: listeners,
            layout: Layout { grow: 1, fill: Some(base), ..Default::default() },
            ..,
            { visible_lines }
        }
    }
}
