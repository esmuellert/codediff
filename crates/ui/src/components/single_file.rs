//! One full-width file with no diff decorations.

use std::collections::HashMap;
use std::rc::Rc;

use align::{ViewLine, ViewLineContent, ViewLineType};
use file_types::{DiffType, DiffVersion};
use loom::{
    Basis, Column, ColumnProps, Layout, Node, Row, RowProps, Scope, Scroll, ScrollOffset,
    ScrollProps, component, rsx, use_context, use_layout_effect, use_measure, use_memo, use_ref,
    use_scroll,
};

use super::code_text::{CodeText, CodeTextProps, longest_line_cells};
use super::context::Ui;
use super::gutter::{Gutter, GutterProps, width_for_line_count};
use crate::hooks::use_diff_viewer_navigation::use_diff_viewer_navigation;
use crate::hooks::use_syntax::use_syntax;
use crate::services::syntax::SyntaxService;
use crate::view::terminal_lines::{
    TerminalLine, WrappedViewLine, find_terminal_line_index, wrap_view_line,
};

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
pub fn SingleFile(
    scope: &mut Scope,
    content: Rc<pipeline::diff::DiffContent>,
    wrap: bool,
    compact: bool,
) -> Node {
    let wrap = *wrap;
    let _ = compact;
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
            u32::from(code_width)
        } else {
            longest_line_cells(&single.lines)
        }
    });
    let initial_top = saved_state
        .top
        .as_ref()
        .and_then(|line| find_terminal_line_index(&wrapped_lines, line))
        .unwrap_or(0);
    let (vertical_view, vertical_handle) = use_scroll(scope, || ScrollOffset {
        x: 0,
        y: initial_top,
    });
    let (horizontal_view, horizontal_handle) = use_scroll(scope, || ScrollOffset {
        x: saved_state.first_cell,
        y: 0,
    });
    let total_terminal_lines = wrapped_lines
        .iter()
        .flat_map(WrappedViewLine::terminal_line_pairs)
        .count() as u32;
    let visible_start = vertical_view
        .clamped_offset()
        .y
        .min(total_terminal_lines.saturating_sub(1));
    let visible_count = u32::from(size.height).saturating_add(4);
    let vertical_position = vertical_view.requested_offset().y;
    let horizontal_position = horizontal_view.requested_offset().x;
    if !content_changed {
        view_states.current().save(
            &file_key,
            SingleFileViewState {
                top: (vertical_position > 0)
                    .then(|| {
                        wrapped_lines
                            .iter()
                            .flat_map(WrappedViewLine::terminal_line_pairs)
                            .nth(vertical_position as usize)
                    })
                    .flatten()
                    .map(|(original, modified)| match version {
                        DiffVersion::Original => original.clone(),
                        DiffVersion::Modified => modified.clone(),
                    }),
                first_cell: horizontal_position,
            },
        );
    }
    *previous_identity.current() = Some(identity.clone());
    let restore_vertical = vertical_handle.clone();
    let restore_horizontal = horizontal_handle.clone();
    use_layout_effect(scope, (identity, code_width, wrap), move || {
        restore_vertical.scroll_to(ScrollOffset {
            x: 0,
            y: initial_top,
        });
        restore_horizontal.scroll_to(ScrollOffset {
            x: saved_state.first_cell,
            y: 0,
        });
    });
    let listeners = use_diff_viewer_navigation(
        scope,
        vertical_handle.clone(),
        horizontal_handle.clone(),
        None,
    );
    let syntax = use_syntax(
        scope,
        ctx.syntax_service.as_ref().map(Rc::clone),
        Rc::clone(content),
        DiffType::Single,
        &wrapped_lines,
    );
    let syntax = syntax.as_deref();

    let base = ctx.theme.normal;
    let number_style = base.patch(ctx.theme.line_number);
    let mut gutter_rows = Vec::with_capacity(visible_count as usize);
    let mut code_rows = Vec::with_capacity(visible_count as usize);
    for (offset, (original, modified)) in wrapped_lines
        .iter()
        .flat_map(WrappedViewLine::terminal_line_pairs)
        .skip(visible_start as usize)
        .take(visible_count as usize)
        .enumerate()
    {
        let line_index = visible_start + offset as u32;
        let terminal_line = match version {
            DiffVersion::Original => original,
            DiffVersion::Modified => modified,
        };
        let TerminalLine::SourceCode { source_line, .. } = terminal_line else {
            continue;
        };
        let number = *source_line;
        let Some(text) = single.lines.get(number.saturating_sub(1) as usize) else {
            continue;
        };
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
        gutter_rows.push(rsx! {
            Row {
                key: line_index,
                layout: Layout { basis: Basis::Length(1), shrink: 0, ..Default::default() },
                ..,
                Gutter {
                    key: 0u32,
                    number: terminal_line.gutter_number(),
                    style: number_style,
                    blank: base,
                    width: gutter_width,
                }
            }
        });
        code_rows.push(rsx! {
            Row {
                key: line_index,
                layout: Layout { basis: Basis::Length(1), shrink: 0, ..Default::default() },
                ..,
                CodeText {
                    key: 0u32,
                    text: text,
                    first_cell: 0,
                    diff: diff,
                    fill_from: fill_from,
                    empty_markers: empty_markers,
                    syntax: syntax_spans,
                    unchanged_style: base,
                    changed_style: base,
                    selection: None,
                }
            }
        });
    }

    rsx! {
        Column {
            ref: Some(node_ref),
            focusable: true,
            listeners: listeners,
            layout: Layout { grow: 1, fill: Some(base), ..Default::default() },
            ..,
            Scroll {
                view: vertical_view,
                handle: Some(vertical_handle),
                horizontal: false,
                vertical: true,
                wheel_step: 3,
                content_height: Some(total_terminal_lines),
                content_offset: ScrollOffset { x: 0, y: visible_start },
                layout: Layout { grow: 1, fill: Some(base), ..Default::default() },
                ..,
                Row {
                    layout: Layout { grow: 1, ..Default::default() },
                    ..,
                    Column {
                        layout: Layout { basis: Basis::Length(gutter_width), shrink: 0, ..Default::default() },
                        ..,
                        { gutter_rows }
                    }
                    Scroll {
                        view: horizontal_view,
                        handle: Some(horizontal_handle),
                        horizontal: true,
                        vertical: false,
                        wheel_step: 3,
                        content_width: Some(*maximum_line_cells),
                        layout: Layout { grow: 1, shrink: 0, fill: Some(base), ..Default::default() },
                        ..,
                        Column {
                            layout: Layout { grow: 1, fill: Some(base), ..Default::default() },
                            ..,
                            { code_rows }
                        }
                    }
                }
            }
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
