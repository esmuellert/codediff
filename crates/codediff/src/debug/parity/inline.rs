//! Rendering records from the production Inline component.

use std::collections::BTreeMap;
use std::rc::Rc;

use anyhow::{Context, Result};
use file_types::{DiffType, DiffVersion};
use loom::testing::Harness;
use ui::Theme;
use ui::components::inline::{Inline, InlineProps};
use ui::components::{Context as UiContext, Ui};

use super::{MIN_WIDTH, Record, Side, gutter_width, highlight, number, render_content};

pub(super) fn run(
    original_path: &str,
    modified_path: &str,
    ignore_trim_whitespace: bool,
) -> Result<()> {
    let content = render_content(original_path, modified_path, ignore_trim_whitespace)?;
    let pipeline::diff::DiffContent::Diff(diff) = content.as_ref() else {
        unreachable!()
    };
    let height = u16::try_from(diff.alignment.view_line_count(DiffType::Inline).max(1))?;
    let width = render_width(
        diff.alignment.lines(DiffVersion::Original),
        diff.alignment.lines(DiffVersion::Modified),
    )?;
    let file = diff.file.clone();
    let original_lines = diff.alignment.lines(DiffVersion::Original).len() as u32;
    let modified_lines = diff.alignment.lines(DiffVersion::Modified).len() as u32;
    let theme = Theme::DARK;
    let mut harness = Harness::new::<Inline>(
        InlineProps {
            content: Rc::clone(&content),
        },
        width,
        height,
    )
    .provide::<Ui>(UiContext {
        theme: Rc::new(theme),
        file: Some(Rc::new(file)),
        ..UiContext::default()
    });
    for _ in 0..4 {
        harness.force_draw();
    }
    records(
        &mut harness,
        original_lines,
        modified_lines,
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
    let code_start = original_gutter + modified_gutter;
    let mut original = BTreeMap::new();
    let mut modified = BTreeMap::new();
    let mut rows = Vec::new();
    let cells = harness.cells();

    for y in 0..height {
        let original_line = number(cells, 0, original_gutter, y);
        let modified_line = number(cells, original_gutter, modified_gutter, y);
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
                code_start,
                width,
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
                0,
                code_start,
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

fn render_width<T: AsRef<str>>(original: &[T], modified: &[T]) -> Result<u16> {
    let content = original
        .iter()
        .chain(modified)
        .map(|line| {
            line_index::LineIndex::new(line.as_ref(), line_index::DEFAULT_TAB_WIDTH)
                .width()
                .get()
        })
        .max()
        .unwrap_or(0);
    let gutters = gutter_width(original.len() as u32) + gutter_width(modified.len() as u32);
    let width = content
        .checked_add(u32::from(gutters))
        .and_then(|width| width.checked_add(1))
        .context("inline render width overflowed")?;
    Ok(u16::try_from(width)?.max(MIN_WIDTH))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn width_contains_the_longest_line_after_both_gutters() {
        let long = "x".repeat(250);
        let original = ["short"];
        let modified = [long.as_str()];

        assert_eq!(render_width(&original, &modified).unwrap(), 259);
    }
}
