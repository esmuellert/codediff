//! `codediff debug parity` — rendered diff cells as JSONL records.

use std::path::Path;
use std::rc::Rc;

use anyhow::{Context, Result};
use file_types::{File, Oid, RepoPath, Revs};
use serde::Serialize;

use crate::cli::DiffLayout;

mod inline;
mod side_by_side;

const MIN_WIDTH: u16 = 200;

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
    Row {
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
) -> Result<()> {
    match layout {
        DiffLayout::SideBySide => {
            side_by_side::run(original_path, modified_path, ignore_trim_whitespace)
        }
        DiffLayout::Inline => inline::run(original_path, modified_path, ignore_trim_whitespace),
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

#[allow(clippy::too_many_arguments)]
fn highlight_record(
    cells: &ui::ratatui::buffer::Buffer,
    row: u16,
    row_start: u16,
    gutter_width: u16,
    row_end: u16,
    line_number: u32,
    side: Side,
    line_background_colour: Option<ui::ratatui::style::Color>,
    changed_background_colour: Option<ui::ratatui::style::Color>,
) -> Option<Record> {
    let line_background = (cells
        .cell((row_start, row))
        .and_then(|cell| cell.style().bg)
        == line_background_colour)
        .then(|| side.role());
    let gutter_background = line_background;
    let code_start = row_start + gutter_width;
    let empty_markers = (code_start..row_end)
        .filter(|&column| {
            cells
                .cell((column, row))
                .is_some_and(|cell| cell.style().underline_color == changed_background_colour)
        })
        .map(|column| u32::from(column - code_start))
        .collect::<Vec<_>>();
    let mut characters = Vec::new();
    let mut column = code_start;
    while column < row_end {
        if cells.cell((column, row)).and_then(|cell| cell.style().bg) != changed_background_colour {
            column += 1;
            continue;
        }
        let range_start = column;
        while column < row_end
            && cells.cell((column, row)).and_then(|cell| cell.style().bg)
                == changed_background_colour
        {
            column += 1;
        }
        characters.push(Character {
            start: u32::from(range_start - code_start),
            end: (column < row_end).then(|| u32::from(column - code_start)),
            fill_to_edge: column == row_end,
        });
    }
    if line_background.is_none() && characters.is_empty() && empty_markers.is_empty() {
        return None;
    }
    Some(Record::Highlight {
        side,
        line: line_number,
        line_background,
        gutter_background,
        characters,
        empty_markers,
    })
}

fn line_number(
    cells: &ui::ratatui::buffer::Buffer,
    gutter_start: u16,
    gutter_width: u16,
    row: u16,
) -> Option<u32> {
    let text: String = (gutter_start..gutter_start + gutter_width)
        .filter_map(|column| cells.cell((column, row)))
        .map(|cell| cell.symbol())
        .collect();
    text.trim().parse().ok()
}

fn gutter_width(line_count: u32) -> u16 {
    let digits = line_count.max(1).ilog10() + 1;
    (digits as u16).max(3) + 1
}

fn read_text(path: &str) -> Result<String> {
    std::fs::read_to_string(Path::new(path)).with_context(|| format!("reading {path}"))
}
