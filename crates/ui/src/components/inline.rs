//! One full-width column showing original and modified lines in sequence.

use std::ops::Range;
use std::rc::Rc;

use align::DiffVersion;
use file_types::DiffType;
use loom::{
    Basis, Column, ColumnProps, Layout, Node, Row, RowProps, Scope, component, rsx, use_context,
    use_measure,
};

use super::code_text::{self, CodeText, CodeTextProps};
use super::context::Ui;
use super::gutter::{self, Gutter, GutterProps, width_for_line_count};
use crate::hooks::use_syntax::use_syntax;
use crate::services::syntax::SyntaxService;

#[component]
pub fn Inline(scope: &mut Scope, content: Rc<pipeline::diff::DiffContent>) -> Node {
    let ctx = use_context::<Ui>(scope);
    let theme = &ctx.theme;
    let pipeline::diff::DiffContent::Diff(diff) = content.as_ref() else {
        unreachable!("DiffViewer sends diffs to Inline")
    };
    let alignment = &diff.alignment;
    let original_line_count = alignment.lines(DiffVersion::Original).len() as u32;
    let modified_line_count = alignment.lines(DiffVersion::Modified).len() as u32;
    let original_gutter_width = width_for_line_count(original_line_count);
    let modified_gutter_width = width_for_line_count(modified_line_count);
    let view_line_count = alignment.view_line_count(DiffType::Inline);
    let (node_ref, size) = use_measure(scope);
    let visible_lines = 0..u32::from(size.height).min(view_line_count);
    let lines: Vec<align::ViewLine> = alignment
        .view_lines_from(DiffType::Inline, visible_lines.start)
        .take(visible_lines.len())
        .collect();
    let syntax = use_syntax(
        scope,
        ctx.syntax_service.as_ref().map(Rc::clone),
        Rc::clone(content),
        DiffType::Inline,
        visible_lines,
    );
    let syntax = syntax.as_deref();

    let mut rows = Vec::with_capacity(lines.len());
    for (view_line, line) in lines.iter().enumerate() {
        let (version, number) = match (line.modified.line(), line.original.line()) {
            (Some(number), _) => (DiffVersion::Modified, number),
            (None, Some(number)) => (DiffVersion::Original, number),
            (None, None) => continue,
        };
        let decorations = alignment.decorations(version, number);
        let code_styles = code_text::diff_styles(theme, version, decorations.line_background);
        let number_style = gutter::diff_style(theme, version, decorations.gutter_background);
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

        rows.push(rsx! {
            Row {
                key: view_line as u32,
                layout: Layout { basis: Basis::Length(1), shrink: 0, ..Default::default() },
                ..,
                Gutter {
                    key: 0u32,
                    number: line.original.line(),
                    style: number_style,
                    blank: number_style,
                    width: original_gutter_width,
                }
                Gutter {
                    key: 1u32,
                    number: line.modified.line(),
                    style: number_style,
                    blank: number_style,
                    width: modified_gutter_width,
                }
                CodeText {
                    key: 2u32,
                    text: Rc::from(text),
                    first_cell: 0,
                    diff: Rc::from(diff_spans.as_slice()),
                    fill_from: fill_from,
                    empty_markers: Rc::from(decorations.empty_markers.as_slice()),
                    syntax: Rc::from(
                        syntax
                            .map(|store| SyntaxService::line_spans(store, &diff.file, version, number))
                            .unwrap_or_default()
                            .as_slice()
                    ),
                    unchanged_style: code_styles.base,
                    changed_style: code_styles.changed,
                    selection: None,
                }
            }
        });
    }

    rsx! {
        Column {
            ref: Some(node_ref),
            layout: Layout { grow: 1, fill: Some(theme.normal), ..Default::default() },
            ..,
            { rows }
        }
    }
}
