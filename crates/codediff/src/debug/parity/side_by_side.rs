//! Records the production SideBySide renderer for the VS Code oracle.

use std::collections::BTreeMap;
use std::rc::Rc;

use anyhow::{Context, Result};
use file_types::{DiffType, DiffVersion};
use loom::testing::Harness;
use loom::{Node, Scope, component, rsx, use_ref};
use ui::Theme;
use ui::components::diff_viewer::ViewState;
use ui::components::side_by_side::{SideBySide, SideBySideProps};
use ui::components::{Context as UiContext, TerminalLine, Ui, terminal_line_pairs};

use super::{
    MIN_WIDTH, PARITY_WIDTH, Record, Side, WRAP_WIDTH, gutter_width, load_diff_content,
    rendered_line_number, source_line,
};

#[component]
fn SideBySideHost(scope: &mut Scope, content: Rc<pipeline::diff::DiffContent>, wrap: bool) -> Node {
    let view_state = use_ref(scope, ViewState::default);
    let content_id = Rc::as_ptr(content) as usize;
    rsx! {
        SideBySide {
            key: content_id,
            content: Rc::clone(content),
            view_state: view_state,
            wrap: *wrap,
            compact: false,
            auto_focus: false,
        }
    }
}

pub(super) fn run(
    original_path: &str,
    modified_path: &str,
    ignore_trim_whitespace: bool,
    wrap: bool,
) -> Result<()> {
    let content = load_diff_content(original_path, modified_path, ignore_trim_whitespace)?;
    let pipeline::diff::DiffContent::Diff(diff) = content.as_ref() else {
        unreachable!()
    };
    let original = diff.alignment.lines(DiffVersion::Original);
    let modified = diff.alignment.lines(DiffVersion::Modified);
    let width = render_width(original, modified, wrap)?;
    let original_gutter = gutter_width(original.len() as u32);
    let modified_gutter = gutter_width(modified.len() as u32);
    let (original_width, modified_width) = pane_widths(width, original_gutter, modified_gutter);
    let rows = terminal_line_pairs(
        &diff.alignment,
        DiffType::SideBySide,
        original_width,
        modified_width,
        wrap,
    );
    let height =
        u16::try_from(rows.len().max(1)).context("side-by-side parity height overflowed")?;
    let theme = Theme::DARK;
    let mut harness = Harness::new::<SideBySideHost>(
        SideBySideHostProps {
            content: Rc::clone(&content),
            wrap,
        },
        width,
        height,
    )
    .provide::<Ui>(UiContext {
        theme: Rc::new(theme),
        file: Some(Rc::new(diff.file.clone())),
        ..UiContext::default()
    });
    for _ in 0..4 {
        harness.force_draw();
    }
    print_records(
        &mut harness,
        width,
        height,
        original.len() as u32,
        modified.len() as u32,
        &rows,
        &diff.alignment,
        wrap,
    )
}

#[allow(clippy::too_many_arguments)]
fn print_records(
    harness: &mut Harness,
    width: u16,
    height: u16,
    original_line_count: u32,
    modified_line_count: u32,
    rows: &[(TerminalLine, TerminalLine)],
    alignment: &align::Alignment,
    wrap: bool,
) -> Result<()> {
    let cells = harness.cells();
    let original_gutter = gutter_width(original_line_count);
    let modified_gutter = gutter_width(modified_line_count);
    let mut row_records = Vec::new();
    let mut original_highlights = BTreeMap::new();
    let mut modified_highlights = BTreeMap::new();

    for row in 0..height {
        let Some((original, modified)) = rows.get(usize::from(row)) else {
            row_records.push(Record::Row {
                index: u32::from(row),
                original: None,
                modified: None,
            });
            continue;
        };
        let divider = divider_at(cells, width, row).context("side-by-side divider missing")?;
        let modified_start = divider + 1;
        row_records.push(Record::Row {
            index: u32::from(row),
            original: rendered_line_number(original),
            modified: rendered_line_number(modified),
        });
        if let Some(line) = source_line(original)
            && let Some(record) = super::semantic_highlight_record(
                if wrap {
                    u32::from(divider.saturating_sub(original_gutter))
                } else {
                    u32::MAX
                },
                line,
                Side::Original,
                alignment,
                original,
                DiffVersion::Original,
            )
        {
            super::merge_highlight(&mut original_highlights, record);
        }
        if let Some(line) = source_line(modified)
            && let Some(record) = super::semantic_highlight_record(
                if wrap {
                    u32::from(width.saturating_sub(modified_start + modified_gutter))
                } else {
                    u32::MAX
                },
                line,
                Side::Modified,
                alignment,
                modified,
                DiffVersion::Modified,
            )
        {
            super::merge_highlight(&mut modified_highlights, record);
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

fn divider_at(cells: &ui::ratatui::buffer::Buffer, width: u16, row: u16) -> Option<u16> {
    (0..width)
        .filter(|&column| {
            cells
                .cell((column, row))
                .is_some_and(|cell| cell.symbol() == "│")
        })
        .min_by_key(|&column| column.abs_diff(width / 2))
}

fn render_width<T: AsRef<str>>(original: &[T], modified: &[T], wrap: bool) -> Result<u16> {
    let original_gutter = gutter_width(original.len() as u32);
    let modified_gutter = gutter_width(modified.len() as u32);
    if wrap {
        let width = u32::from(original_gutter)
            .checked_add(u32::from(modified_gutter))
            .and_then(|width| width.checked_add(u32::from(WRAP_WIDTH) * 2))
            .and_then(|width| width.checked_add(1))
            .context("side-by-side wrapped parity width overflowed")?;
        return Ok(u16::try_from(width)?);
    }
    let longest = original
        .iter()
        .chain(modified)
        .map(|line| {
            line_index::LineIndex::new(line.as_ref(), line_index::DEFAULT_TAB_WIDTH)
                .width()
                .get()
        })
        .max()
        .unwrap_or(0);
    let maximum_gutter = original_gutter.max(modified_gutter);
    let width = longest
        .checked_add(u32::from(maximum_gutter))
        .and_then(|width| width.checked_add(1))
        .and_then(|width| width.checked_mul(2))
        .and_then(|width| width.checked_add(1))
        .context("side-by-side parity width overflowed")?;
    Ok(u16::try_from(width.min(u32::from(PARITY_WIDTH)))?.max(MIN_WIDTH))
}

fn pane_widths(width: u16, original_gutter: u16, modified_gutter: u16) -> (u16, u16) {
    let text = u32::from(width)
        .saturating_sub(u32::from(original_gutter))
        .saturating_sub(u32::from(modified_gutter))
        .saturating_sub(1);
    (
        u16::try_from(text.div_ceil(2)).unwrap_or(u16::MAX),
        u16::try_from(text / 2).unwrap_or(u16::MAX),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use ui::ratatui::buffer::Buffer;
    use ui::ratatui::layout::Rect;

    #[test]
    fn a_code_bar_is_not_mistaken_for_the_divider() {
        let mut cells = Buffer::empty(Rect::new(0, 0, 200, 1));
        cells[(20, 0)].set_symbol("│");
        cells[(99, 0)].set_symbol("│");

        assert_eq!(divider_at(&cells, 200, 0), Some(99));
    }

    #[test]
    fn wrapped_panes_have_the_requested_code_width() {
        let width = render_width(&["long"], &["long"], true).unwrap();
        assert_eq!(pane_widths(width, 4, 4), (40, 40));
    }
}
