//! One full-width file with no diff decorations.

use std::collections::HashMap;
use std::rc::Rc;

use align::{ViewLine, ViewLineContent, ViewLineType};
use file_types::{DiffType, DiffVersion};
use loom::{
    Basis, Column, ColumnProps, Layout, Node, Row, RowProps, Scope, component, rsx, use_context,
    use_layout_effect, use_measure, use_memo, use_ref,
};

use super::code_text::{CodeText, CodeTextProps, longest_line_cells};
use super::context::Ui;
use super::gutter::{Gutter, GutterProps, width_for_line_count};
use super::wrap::{
    TerminalLine, WrappedViewLine, find_terminal_line_index, longest_terminal_line_cells,
    terminal_line_count, wrap_view_line, wrapped_view_line_range_for_terminal_lines,
};
use crate::hooks::use_diff_viewer_navigation::{HorizontalDimensions, use_diff_viewer_navigation};
use crate::hooks::use_horizontal_scroll::use_horizontal_scroll;
use crate::hooks::use_scroll::use_scroll;
use crate::hooks::use_syntax::use_syntax;
use crate::services::syntax::SyntaxService;

#[derive(Clone, Default)]
struct SingleFileViewState {
    top: Option<TerminalLine>,
    first_cell: u32,
}

#[derive(Default)]
struct SingleFileViewStateHistory {
    entries: HashMap<String, SingleFileViewState>,
}

impl SingleFileViewStateHistory {
    fn load(&self, key: &str) -> SingleFileViewState {
        self.entries.get(key).cloned().unwrap_or_default()
    }

    fn save(&mut self, key: &str, state: SingleFileViewState) {
        self.entries.insert(key.to_owned(), state);
    }
}

#[component]
pub fn SingleFile(scope: &mut Scope, content: Rc<pipeline::diff::DiffContent>, wrap: bool) -> Node {
    let wrap = *wrap;
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
    let (node_ref, size) = use_measure(scope);
    let version = single.side();
    let code_width = size.width.saturating_sub(gutter_width);
    let wrapped_lines = use_memo(scope, (content_id, code_width, wrap), || {
        wrapped_lines(single, version, code_width, wrap)
    });
    let maximum_line_cells = use_memo(scope, (content_id, code_width, wrap), || {
        if wrap {
            longest_terminal_line_cells(&wrapped_lines, version, &single.lines)
        } else {
            longest_line_cells(&single.lines)
        }
    });
    let terminal_line_count = terminal_line_count(&wrapped_lines);
    let initial_top = saved_state
        .top
        .as_ref()
        .and_then(|line| find_terminal_line_index(&wrapped_lines, line))
        .unwrap_or(0);
    let (view, vertical_handle) = use_scroll(scope, terminal_line_count, initial_top, size.height);
    let horizontal_limits = HorizontalDimensions::Single {
        longest_line_cells: *maximum_line_cells,
        gutter_cells: gutter_width,
    }
    .limits(size.width);
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
                top: (view.top > 0)
                    .then(|| {
                        wrapped_lines
                            .iter()
                            .flat_map(WrappedViewLine::terminal_line_pairs)
                            .nth(view.top as usize)
                    })
                    .flatten()
                    .and_then(|(original, modified)| match version {
                        DiffVersion::Original => Some(original.clone()),
                        DiffVersion::Modified => Some(modified.clone()),
                    }),
                first_cell: horizontal.requested_first_cell,
            },
        );
    }
    *previous_identity.current() = Some(identity.clone());
    use_layout_effect(scope, identity, move || {
        vertical_handle.scroll_to(initial_top);
        horizontal_handle.scroll_to(saved_state.first_cell);
    });
    let listeners = use_diff_viewer_navigation(vertical_handle, horizontal_handle);
    let visible_wrapped_lines = &wrapped_lines[wrapped_view_line_range_for_terminal_lines(
        &wrapped_lines,
        view.view_lines.start,
        view.view_lines.end,
    )];
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
        .flat_map(WrappedViewLine::terminal_line_pairs)
        .skip(view.view_lines.start as usize)
        .take(view.view_lines.len())
        .enumerate()
        .filter_map(|(offset, (original, modified))| {
            let line_index = view.view_lines.start + offset as u32;
            let terminal_line = match version {
                DiffVersion::Original => original,
                DiffVersion::Modified => modified,
            };
            let number = match terminal_line {
                TerminalLine::SourceCode { source_line, .. } => *source_line,
                TerminalLine::Filler => return None,
            };
            let gutter_number = terminal_line.gutter_number();
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
                    key: line_index,
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
            ref: Some(node_ref),
            focusable: true,
            listeners: listeners,
            layout: Layout { grow: 1, fill: Some(base), ..Default::default() },
            ..,
            { visible_lines }
        }
    }
}

fn wrapped_lines(
    single: &pipeline::diff::SingleFile,
    version: DiffVersion,
    code_width: u16,
    wrap: bool,
) -> Vec<WrappedViewLine> {
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
            if wrap {
                wrap_view_line(
                    view_line,
                    original,
                    modified,
                    code_width,
                    code_width,
                    DiffType::SideBySide,
                )
            } else {
                WrappedViewLine::from_view_line(view_line, original, modified)
            }
        })
        .collect::<Vec<_>>()
}
