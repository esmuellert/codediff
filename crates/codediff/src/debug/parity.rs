//! `codediff debug parity` — rendered diff cells as JSONL records.

use std::path::Path;
use std::rc::Rc;

use anyhow::{Context, Result};
use file_types::{DiffVersion, File, Oid, RepoPath, Revs};
use line_index::{ByteOff, CellCol, LineIndex};
use serde::Serialize;

use crate::cli::DiffLayout;

mod inline;
mod side_by_side;

const MIN_WIDTH: u16 = 200;
const PARITY_WIDTH: u16 = 1_600;
const WRAP_WIDTH: u16 = 40;
const MAX_CHARACTER_CELLS: u32 = 10_000;

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "lowercase")]
enum Side {
    Original,
    Modified,
}

impl Side {
    fn role(self) -> Role {
        match self {
            Self::Original => Role::Delete,
            Self::Modified => Role::Insert,
        }
    }
}

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "lowercase")]
enum Role {
    Insert,
    Delete,
}

#[derive(Serialize)]
struct Character {
    start: u32,
    end: Option<u32>,
    fill_to_edge: bool,
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Record {
    Line {
        index: u32,
        original: Option<u32>,
        modified: Option<u32>,
    },
    Highlight {
        side: Side,
        line: u32,
        line_background: Option<Role>,
        gutter_background: Option<Role>,
        characters: Vec<Character>,
        empty_markers: Vec<u32>,
    },
}

pub fn run(
    original_path: &str,
    modified_path: &str,
    layout: DiffLayout,
    ignore_trim_whitespace: bool,
    wrap: bool,
) -> Result<()> {
    match layout {
        DiffLayout::SideBySide => {
            side_by_side::run(original_path, modified_path, ignore_trim_whitespace, wrap)
        }
        DiffLayout::Inline => {
            inline::run(original_path, modified_path, ignore_trim_whitespace, wrap)
        }
    }
}

fn load_diff_content(
    original_path: &str,
    modified_path: &str,
    ignore_trim_whitespace: bool,
) -> Result<Rc<pipeline::diff::DiffContent>> {
    let original_text = read_text(original_path)?;
    let modified_text = read_text(modified_path)?;
    let original = vscode_diff::editor_lines(&original_text);
    let modified = vscode_diff::editor_lines(&modified_text);
    let mut options = vscode_diff::Options::default().with_time_budget_ms(0);
    if ignore_trim_whitespace {
        options = options.ignoring_trim_whitespace();
    }
    let lines_diff = vscode_diff::compute(&original, &modified, &options)?;
    let alignment = pipeline::diff::align(lines_diff, &original, &modified)?;
    let root = std::env::current_dir()?;
    let file = File::unchanged_path(
        RepoPath::new("render.txt", &root),
        Revs::worktree_against(Oid::new("render")),
    );
    Ok(Rc::new(pipeline::diff::DiffContent::Diff(
        pipeline::diff::Diff { file, alignment },
    )))
}

fn leading_indent_cells(text: &str) -> u32 {
    let mut cells = 0u32;
    for character in text.chars() {
        match character {
            ' ' => cells = cells.saturating_add(1),
            '\t' => cells = line_index::tab_advance(CellCol(cells), line_index::DEFAULT_TAB_WIDTH),
            _ => break,
        }
    }
    cells
}

#[allow(clippy::too_many_arguments)]
fn semantic_highlight_record(
    code_width: u32,
    line: u32,
    side: Side,
    alignment: &align::Alignment,
    terminal: &ui::components::TerminalLine,
    version: DiffVersion,
) -> Option<Record> {
    let ui::components::TerminalLine::SourceCode { bytes, .. } = terminal else {
        return None;
    };
    let decorations = alignment.decorations(version, line);
    if !decorations.line_background
        && !decorations.gutter_background
        && decorations.characters.is_empty()
        && decorations.empty_markers.is_empty()
    {
        return None;
    }
    let text = alignment.line(version, line).unwrap_or("");
    let index = LineIndex::new(text, line_index::DEFAULT_TAB_WIDTH);
    let continuation_indent = (bytes.start > 0).then(|| {
        let indent = leading_indent_cells(text);
        if indent.saturating_add(1) > code_width {
            0
        } else {
            indent
        }
    });
    let role = side.role();
    let line_background = decorations.line_background.then_some(role);
    let gutter_background = decorations.gutter_background.then_some(role);
    let fragment_start = index.byte_to_cell(ByteOff(bytes.start)).get();
    let local_cell = |byte| {
        index
            .byte_to_cell(ByteOff(byte))
            .get()
            .saturating_sub(fragment_start)
            .min(code_width)
    };
    let mut characters = Vec::new();
    for decoration in &decorations.characters {
        let start = decoration.bytes.start.max(bytes.start);
        let end = decoration.bytes.end.min(bytes.end);
        if start < end {
            let mut start_cell = local_cell(start);
            let mut end_cell = local_cell(end);
            if let Some(indent) = continuation_indent {
                if decoration.bytes.start >= bytes.start {
                    start_cell = start_cell.saturating_add(indent);
                }
                end_cell = end_cell.saturating_add(indent);
            }
            if start_cell < MAX_CHARACTER_CELLS {
                let fill_to_edge = decoration.fill_to_edge || decoration.bytes.end > bytes.end;
                characters.push(Character {
                    start: start_cell,
                    end: (!fill_to_edge).then(|| end_cell.min(MAX_CHARACTER_CELLS)),
                    fill_to_edge,
                });
            }
        }
        if decoration.fill_to_edge
            && decoration.bytes.start == bytes.end
            && bytes.end == text.len() as u32
        {
            characters.push(Character {
                start: local_cell(bytes.end).saturating_add(continuation_indent.unwrap_or(0)),
                end: None,
                fill_to_edge: true,
            });
        }
    }
    let mut empty_markers = decorations
        .empty_markers
        .into_iter()
        .filter(|marker| {
            *marker >= bytes.start
                && (*marker < bytes.end || (*marker == bytes.end && bytes.end == text.len() as u32))
        })
        .map(|marker| {
            local_cell(marker).saturating_add(
                continuation_indent
                    .filter(|_| marker >= bytes.start)
                    .unwrap_or(0),
            )
        })
        .filter(|marker| *marker < MAX_CHARACTER_CELLS)
        .collect::<Vec<_>>();
    empty_markers.sort_unstable();
    empty_markers.dedup();
    Some(Record::Highlight {
        side,
        line,
        line_background,
        gutter_background,
        characters,
        empty_markers,
    })
}

fn rendered_line_number(line: &ui::components::TerminalLine) -> Option<u32> {
    match line {
        ui::components::TerminalLine::SourceCode { source_line, bytes } if bytes.start == 0 => {
            Some(*source_line)
        }
        _ => None,
    }
}

fn source_line(line: &ui::components::TerminalLine) -> Option<u32> {
    match line {
        ui::components::TerminalLine::SourceCode { source_line, .. } => Some(*source_line),
        ui::components::TerminalLine::Filler => None,
    }
}

fn merge_highlight(highlights: &mut std::collections::BTreeMap<u32, Record>, record: Record) {
    let Record::Highlight {
        side,
        line,
        line_background,
        gutter_background,
        mut characters,
        mut empty_markers,
    } = record
    else {
        return;
    };
    let Some(existing) = highlights.get_mut(&line) else {
        highlights.insert(
            line,
            Record::Highlight {
                side,
                line,
                line_background,
                gutter_background,
                characters,
                empty_markers,
            },
        );
        return;
    };
    let Record::Highlight {
        line_background: existing_line_background,
        gutter_background: existing_gutter_background,
        characters: existing_characters,
        empty_markers: existing_empty_markers,
        ..
    } = existing
    else {
        unreachable!("highlight map contains only highlights");
    };
    if line_background.is_some() {
        *existing_line_background = line_background;
    }
    if gutter_background.is_some() {
        *existing_gutter_background = gutter_background;
    }
    existing_characters.append(&mut characters);
    existing_empty_markers.append(&mut empty_markers);
}

fn gutter_width(line_count: u32) -> u16 {
    let digits = line_count.max(1).ilog10() + 1;
    (digits as u16).max(3) + 1
}

fn read_text(path: &str) -> Result<String> {
    std::fs::read_to_string(Path::new(path)).with_context(|| format!("reading {path}"))
}
