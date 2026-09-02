//! `codediff debug parity` — final SideBySide cells as JSONL records.

use std::collections::BTreeMap;
use std::path::Path;
use std::rc::Rc;

use anyhow::{Context, Result};
use file_types::{DiffType, File, Oid, RepoPath, Revs};
use loom::testing::Harness;
use serde::Serialize;
use ui::Theme;
use ui::components::side_by_side::{SideBySide, SideBySideProps};
use ui::components::{Context as UiContext, Ui};

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

pub fn run(original_path: &str, modified_path: &str, ignore_trim_whitespace: bool) -> Result<()> {
    let original_text = read(original_path)?;
    let modified_text = read(modified_path)?;
    let original = vscode_diff::editor_lines(&original_text);
    let modified = vscode_diff::editor_lines(&modified_text);
    let mut options = vscode_diff::Options::default().with_time_budget_ms(0);
    if ignore_trim_whitespace {
        options = options.ignoring_trim_whitespace();
    }
    let diff = vscode_diff::compute(&original, &modified, &options)?;
    let alignment = pipeline::diff::align(diff, &original, &modified)?;
    let height = u16::try_from(alignment.view_line_count(DiffType::SideBySide).max(1))?;
    let root = std::env::current_dir()?;
    let file = File::unchanged_path(
        RepoPath::new("parity.txt", &root),
        Revs::worktree_against(Oid::new("parity")),
    );
    let content = pipeline::diff::DiffContent::Diff(pipeline::diff::Diff {
        file: file.clone(),
        alignment,
    });
    let theme = Theme::DARK;
    let width = parity_width(&original, &modified)?;
    let mut harness =
        Harness::new::<SideBySide>(SideBySideProps {}, width, height).provide::<Ui>(UiContext {
            theme: Rc::new(theme),
            file: Some(Rc::new(file)),
            diff: Some(Rc::new(content)),
            ..UiContext::default()
        });
    for _ in 0..4 {
        harness.force_draw();
    }
    records(
        &mut harness,
        original.len() as u32,
        modified.len() as u32,
        theme,
        width,
        height,
    )
}

fn records(
    harness: &mut Harness,
    original_lines: u32,
    modified_lines: u32,
    theme: Theme,
    width: u16,
    height: u16,
) -> Result<()> {
    let original_gutter = gutter_width(original_lines);
    let modified_gutter = gutter_width(modified_lines);
    let mut original = BTreeMap::new();
    let mut modified = BTreeMap::new();
    let mut rows = Vec::new();
    let cells = harness.cells();

    for y in 0..height {
        let divider = divider_at(cells, width, y).expect("SideBySide has a divider");
        let original_line = number(cells, 0, original_gutter, y);
        let modified_start = divider + 1;
        let modified_line = number(cells, modified_start, modified_gutter, y);
        rows.push(Record::Row {
            index: u32::from(y),
            original: original_line,
            modified: modified_line,
        });
        if let Some(line) = original_line
            && let Some(record) = highlight(
                cells,
                y,
                0,
                original_gutter,
                divider,
                line,
                Side::Original,
                theme.normal.patch(theme.deleted).bg,
                theme.normal.patch(theme.deleted_text).bg,
            )
        {
            original.insert(line, record);
        }
        if let Some(line) = modified_line
            && let Some(record) = highlight(
                cells,
                y,
                modified_start,
                modified_gutter,
                width,
                line,
                Side::Modified,
                theme.normal.patch(theme.inserted).bg,
                theme.normal.patch(theme.inserted_text).bg,
            )
        {
            modified.insert(line, record);
        }
    }

    for record in rows
        .into_iter()
        .chain(original.into_values())
        .chain(modified.into_values())
    {
        println!("{}", serde_json::to_string(&record)?);
    }
    Ok(())
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

fn divider_at(cells: &ui::ratatui::buffer::Buffer, width: u16, y: u16) -> Option<u16> {
    (0..width)
        .filter(|&x| cells.cell((x, y)).is_some_and(|cell| cell.symbol() == "│"))
        .min_by_key(|&x| x.abs_diff(width / 2))
}

fn gutter_width(lines: u32) -> u16 {
    let digits = lines.max(1).ilog10() + 1;
    (digits as u16).max(3) + 1
}

fn parity_width(original: &[&str], modified: &[&str]) -> Result<u16> {
    let content = original
        .iter()
        .chain(modified)
        .map(|line| {
            line_index::LineIndex::new(line, line_index::DEFAULT_TAB_WIDTH)
                .width()
                .get()
        })
        .max()
        .unwrap_or(0);
    let gutters = gutter_width(original.len() as u32).max(gutter_width(modified.len() as u32));
    let width = (content + u32::from(gutters) + 1)
        .checked_mul(2)
        .and_then(|width| width.checked_add(1))
        .context("parity render width overflowed")?;
    Ok(u16::try_from(width)?.max(MIN_WIDTH))
}

fn read(path: &str) -> Result<String> {
    std::fs::read_to_string(Path::new(path)).with_context(|| format!("reading {path}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ui::ratatui::buffer::Buffer;
    use ui::ratatui::layout::Rect;

    #[test]
    fn a_vertical_bar_in_code_is_not_mistaken_for_the_pane_divider() {
        let mut cells = Buffer::empty(Rect::new(0, 0, 200, 1));
        cells[(20, 0)].set_symbol("│");
        cells[(99, 0)].set_symbol("│");

        assert_eq!(divider_at(&cells, 200, 0), Some(99));
    }

    #[test]
    fn parity_width_contains_the_longest_line_on_both_sides() {
        let original = ["short"];
        let modified = [
            "a line which is longer than one hundred terminal cells ....................................................................",
        ];

        assert!(parity_width(&original, &modified).unwrap() > 200);
    }
}
