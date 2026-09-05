//! Rendering records from the production Inline component.

use std::collections::BTreeMap;
use std::rc::Rc;

use anyhow::{Context, Result};
use file_types::{DiffType, DiffVersion};
use loom::testing::Harness;
use ui::Theme;
use ui::components::inline::{Inline, InlineProps};
use ui::components::{Context as UiContext, Ui};

use super::{
    MIN_WIDTH, Record, Side, gutter_width, highlight_record, line_number, load_diff_content,
};

pub(super) fn run(
    original_path: &str,
    modified_path: &str,
    ignore_trim_whitespace: bool,
) -> Result<()> {
    let content = load_diff_content(original_path, modified_path, ignore_trim_whitespace)?;
    let pipeline::diff::DiffContent::Diff(diff) = content.as_ref() else {
        unreachable!()
    };
    let height = u16::try_from(diff.alignment.view_line_count(DiffType::Inline).max(1))?;
    let width = render_width(
        diff.alignment.lines(DiffVersion::Original),
        diff.alignment.lines(DiffVersion::Modified),
    )?;
    let file = diff.file.clone();
    let original_line_count = diff.alignment.lines(DiffVersion::Original).len() as u32;
    let modified_line_count = diff.alignment.lines(DiffVersion::Modified).len() as u32;
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
    print_records(
        &mut harness,
        original_line_count,
        modified_line_count,
        theme,
        width,
        height,
    )
}

fn print_records(
    harness: &mut Harness,
    original_line_count: u32,
    modified_line_count: u32,
    theme: Theme,
    width: u16,
    height: u16,
) -> Result<()> {
    let original_gutter_width = gutter_width(original_line_count);
    let modified_gutter_width = gutter_width(modified_line_count);
    let code_start = original_gutter_width + modified_gutter_width;
    let mut original_highlights = BTreeMap::new();
    let mut modified_highlights = BTreeMap::new();
    let mut row_records = Vec::new();
    let cells = harness.cells();

    for row in 0..height {
        let original_line_number = line_number(cells, 0, original_gutter_width, row);
        let modified_line_number =
            line_number(cells, original_gutter_width, modified_gutter_width, row);
        row_records.push(Record::Row {
            index: u32::from(row),
            original: original_line_number,
            modified: modified_line_number,
        });
        if let Some(line_number) = original_line_number
            && let Some(record) = highlight_record(
                cells,
                row,
                0,
                code_start,
                width,
                line_number,
                Side::Original,
                theme.normal.patch(theme.deleted).bg,
                theme.normal.patch(theme.deleted_text).bg,
            )
        {
            original_highlights.insert(line_number, record);
        }
        if let Some(line_number) = modified_line_number
            && let Some(record) = highlight_record(
                cells,
                row,
                0,
                code_start,
                width,
                line_number,
                Side::Modified,
                theme.normal.patch(theme.inserted).bg,
                theme.normal.patch(theme.inserted_text).bg,
            )
        {
            modified_highlights.insert(line_number, record);
        }
    }

    for record in row_records
        .into_iter()
        .chain(original_highlights.into_values())
        .chain(modified_highlights.into_values())
    {
        println!("{}", serde_json::to_string(&record)?);
    }
    Ok(())
}

fn render_width<T: AsRef<str>>(original: &[T], modified: &[T]) -> Result<u16> {
    let longest_line_width = original
        .iter()
        .chain(modified)
        .map(|line| {
            line_index::LineIndex::new(line.as_ref(), line_index::DEFAULT_TAB_WIDTH)
                .width()
                .get()
        })
        .max()
        .unwrap_or(0);
    let gutter_cells = gutter_width(original.len() as u32) + gutter_width(modified.len() as u32);
    let width = longest_line_width
        .checked_add(u32::from(gutter_cells))
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
