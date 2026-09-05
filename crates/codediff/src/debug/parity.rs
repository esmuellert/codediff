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

fn render_content(
    original_path: &str,
    modified_path: &str,
    ignore_trim_whitespace: bool,
) -> Result<Rc<pipeline::diff::DiffContent>> {
    let original_text = read(original_path)?;
    let modified_text = read(modified_path)?;
    let original = vscode_diff::editor_lines(&original_text);
    let modified = vscode_diff::editor_lines(&modified_text);
    let mut options = vscode_diff::Options::default().with_time_budget_ms(0);
    if ignore_trim_whitespace {
        options = options.ignoring_trim_whitespace();
    }
    let computed = vscode_diff::compute(&original, &modified, &options)?;
    let alignment = pipeline::diff::align(computed, &original, &modified)?;
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
fn highlight(
    cells: &ui::ratatui::buffer::Buffer,
    y: u16,
    start: u16,
    gutter: u16,
    end: u16,
    line: u32,
    side: Side,
    line_bg: Option<ui::ratatui::style::Color>,
    char_bg: Option<ui::ratatui::style::Color>,
) -> Option<Record> {
    let line_background =
        (cells.cell((start, y)).and_then(|cell| cell.style().bg) == line_bg).then(|| side.role());
    let gutter_background = line_background;
    let code_start = start + gutter;
    let empty_markers = (code_start..end)
        .filter(|&x| {
            cells
                .cell((x, y))
                .is_some_and(|cell| cell.style().underline_color == char_bg)
        })
        .map(|x| u32::from(x - code_start))
        .collect::<Vec<_>>();
    let mut characters = Vec::new();
    let mut x = code_start;
    while x < end {
        if cells.cell((x, y)).and_then(|cell| cell.style().bg) != char_bg {
            x += 1;
            continue;
        }
        let first = x;
        while x < end && cells.cell((x, y)).and_then(|cell| cell.style().bg) == char_bg {
            x += 1;
        }
        characters.push(Character {
            start: u32::from(first - code_start),
            end: (x < end).then(|| u32::from(x - code_start)),
            fill_to_edge: x == end,
        });
    }
    if line_background.is_none() && characters.is_empty() && empty_markers.is_empty() {
        return None;
    }
    Some(Record::Highlight {
        side,
        line,
        line_background,
        gutter_background,
        characters,
        empty_markers,
    })
}

fn number(cells: &ui::ratatui::buffer::Buffer, start: u16, width: u16, y: u16) -> Option<u32> {
    let text: String = (start..start + width)
        .filter_map(|x| cells.cell((x, y)))
        .map(|cell| cell.symbol())
        .collect();
    text.trim().parse().ok()
}

fn gutter_width(lines: u32) -> u16 {
    let digits = lines.max(1).ilog10() + 1;
    (digits as u16).max(3) + 1
}

fn read(path: &str) -> Result<String> {
    std::fs::read_to_string(Path::new(path)).with_context(|| format!("reading {path}"))
}
