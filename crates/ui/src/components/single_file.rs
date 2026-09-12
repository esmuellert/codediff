//! One full-width file with no diff decorations.

use std::collections::HashMap;
use std::rc::Rc;

use align::{ViewLine, ViewLineContent, ViewLineType};
use file_types::{DiffType, DiffVersion};
use loom::{
    Basis, Column, ColumnProps, Layout, Node, Row, RowProps, Scope, component, rsx, use_context,
    use_layout_effect, use_memo, use_ref,
};

use super::code_text::{CodeText, CodeTextProps, longest_line_cells};
use super::context::Ui;
use super::gutter::{Gutter, GutterProps, width_for_line_count};
use super::wrap::{TerminalLine, WrappedViewLine};
use crate::hooks::use_diff_viewer_navigation::{HorizontalDimensions, use_diff_viewer_navigation};
use crate::hooks::use_horizontal_scroll::use_horizontal_scroll;
use crate::hooks::use_scroll::use_scroll;
use crate::hooks::use_syntax::use_syntax;
use crate::services::syntax::SyntaxService;

#[derive(Clone, Copy, Default)]
struct SingleFileViewState {
    top: u32,
    first_cell: u32,
}

#[derive(Default)]
struct SingleFileViewStateHistory {
    entries: HashMap<String, SingleFileViewState>,
}

impl SingleFileViewStateHistory {
    fn load(&self, key: &str) -> SingleFileViewState {
        self.entries.get(key).copied().unwrap_or_default()
    }

    fn save(&mut self, key: &str, state: SingleFileViewState) {
        self.entries.insert(key.to_owned(), state);
    }
}

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
    let view_states = use_ref(scope, SingleFileViewStateHistory::default);
    let saved_state = view_states.current().load(&file_key);
    let previous_identity = use_ref(scope, || None::<(String, usize)>);
    let identity = (file_key.clone(), content_id);
    let content_changed = previous_identity.current().as_ref() != Some(&identity);
    let version = single.side();
    let wrapped_lines = use_memo(scope, content_id, || {
        single
            .lines
            .iter()
            .enumerate()
            .map(|(index, text)| {
                let source_line = index as u32 + 1;
                let view_line = match version {
                    DiffVersion::Original => ViewLine {
                        original: ViewLineContent::SourceLine(source_line),
                        modified: ViewLineContent::Filler,
                        kind: ViewLineType::Deleted,
                    },
                    DiffVersion::Modified => ViewLine {
                        original: ViewLineContent::Filler,
                        modified: ViewLineContent::SourceLine(source_line),
                        kind: ViewLineType::Inserted,
                    },
                };
                let (original, modified) = match version {
                    DiffVersion::Original => (Some(text.as_str()), None),
                    DiffVersion::Modified => (None, Some(text.as_str())),
                };
                WrappedViewLine::from_view_line(view_line, original, modified)
            })
            .collect::<Vec<_>>()
    });
    let maximum_line_cells = use_memo(scope, content_id, || longest_line_cells(&single.lines));
    let (view, vertical_handle) = use_scroll(
        scope,
        wrapped_lines
            .iter()
            .map(|line| line.original.len() as u32)
            .sum(),
        saved_state.top,
    );
    let horizontal_limits = HorizontalDimensions::Single {
        longest_line_cells: *maximum_line_cells,
        gutter_cells: gutter_width,
    }
    .limits(view.width);
    let (horizontal_view, horizontal_handle) = use_horizontal_scroll(
        scope,
        horizontal_limits.maximum_first_cell(),
        saved_state.first_cell,
    );
    let horizontal = horizontal_limits.view(horizontal_view.first_cell);
    if !content_changed {
        view_states.current().save(
            &file_key,
            SingleFileViewState {
                top: view.top,
                first_cell: horizontal.requested_first_cell,
            },
        );
    }
    *previous_identity.current() = Some(identity.clone());
    use_layout_effect(scope, identity, move || {
        vertical_handle.scroll_to(saved_state.top);
        horizontal_handle.scroll_to(saved_state.first_cell);
    });
    let listeners = use_diff_viewer_navigation(vertical_handle, horizontal_handle);
    let visible_wrapped_lines =
        &wrapped_lines[view.view_lines.start as usize..view.view_lines.end as usize];
    let syntax = use_syntax(
        scope,
        ctx.syntax_service.as_ref().map(Rc::clone),
        Rc::clone(content),
        DiffType::Single,
        visible_wrapped_lines,
    );
    let syntax = syntax.as_deref();

    let base = ctx.theme.normal;
    let number_style = base.patch(ctx.theme.line_number);
    let visible_lines: Vec<Node> = wrapped_lines
        .iter()
        .flat_map(|wrapped_line| wrapped_line.original.iter().zip(&wrapped_line.modified))
        .skip(view.view_lines.start as usize)
        .take(view.view_lines.len())
        .filter_map(|(original, modified)| {
            let terminal_line = match version {
                DiffVersion::Original => original,
                DiffVersion::Modified => modified,
            };
            let number = match terminal_line {
                TerminalLine::SourceCode { source_line, .. } => *source_line,
                TerminalLine::Filler => return None,
            };
            let gutter_number = match terminal_line {
                TerminalLine::SourceCode { source_line, bytes } if bytes.start == 0 => {
                    Some(*source_line)
                }
                _ => None,
            };
            let text = single.lines.get(number.saturating_sub(1) as usize)?;
            let syntax_spans = syntax
                .map(|store| SyntaxService::line_spans(store, &single.file, version, number))
                .unwrap_or_default();
            let (text, diff, fill_from, empty_markers, syntax_spans) =
                super::code_text::prepare_code_text_inputs(
                    text,
                    terminal_line,
                    &[],
                    None,
                    &[],
                    &syntax_spans,
                );
            let row = rsx! {
                Row {
                    key: number,
                    layout: Layout { basis: Basis::Length(1), shrink: 0, ..Default::default() },
                    ..,
                    Gutter {
                        key: 0u32,
                        number: gutter_number,
                        style: number_style,
                        blank: base,
                        width: gutter_width,
                    }
                    CodeText {
                        key: 1u32,
                        text: text,
                        first_cell: horizontal.first_cell(version),
                        diff: diff,
                        fill_from: fill_from,
                        empty_markers: empty_markers,
                        syntax: syntax_spans,
                        unchanged_style: base,
                        changed_style: base,
                        selection: None,
                    }
                }
            };
            Some(row)
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
