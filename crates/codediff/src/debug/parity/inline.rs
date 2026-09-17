//! Records the production Inline renderer for the VS Code oracle.

use std::collections::BTreeMap;
use std::rc::Rc;

use anyhow::{Context, Result};
use file_types::{DiffType, DiffVersion};
use loom::testing::Harness;
use loom::{Node, Scope, component, rsx, use_ref};
use ui::Theme;
use ui::components::diff_viewer::ViewState;
use ui::components::inline::{Inline, InlineProps};
use ui::components::{Context as UiContext, TerminalLine, Ui, terminal_line_pairs};

use super::{
    MIN_WIDTH, PARITY_WIDTH, Record, Side, WRAP_WIDTH, gutter_width, load_diff_content,
    rendered_line_number, source_line,
};

#[component]
fn InlineHost(scope: &mut Scope, content: Rc<pipeline::diff::DiffContent>, wrap: bool) -> Node {
    let view_state = use_ref(scope, ViewState::default);
    let content_id = Rc::as_ptr(content) as usize;
    rsx! {
        Inline {
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
    let gutters = gutter_width(original.len() as u32) + gutter_width(modified.len() as u32);
    let rows = terminal_line_pairs(
        &diff.alignment,
        DiffType::Inline,
        width.saturating_sub(gutters),
        width.saturating_sub(gutters),
        wrap,
    );
    let height = u16::try_from(rows.len().max(1)).context("inline parity height overflowed")?;
    let theme = Theme::DARK;
    let mut harness = Harness::new::<InlineHost>(
        InlineHostProps {
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
        width,
        height,
        original.len() as u32,
        modified.len() as u32,
        &rows,
        &diff.alignment,
        wrap,
    )
}

fn print_records(
    width: u16,
    height: u16,
    original_line_count: u32,
    modified_line_count: u32,
    rows: &[(TerminalLine, TerminalLine)],
    alignment: &align::Alignment,
    wrap: bool,
) -> Result<()> {
    let mut row_records = Vec::new();
    let mut original_highlights = BTreeMap::new();
    let mut modified_highlights = BTreeMap::new();
    let code_start = gutter_width(original_line_count) + gutter_width(modified_line_count);

    for (index, text) in alignment.lines(DiffVersion::Original).iter().enumerate() {
        let line = index as u32 + 1;
        let terminal = TerminalLine::SourceCode {
            source_line: line,
            bytes: 0..text.len() as u32,
        };
        if let Some(record) = super::semantic_highlight_record(
            u32::MAX,
            line,
            Side::Original,
            alignment,
            &terminal,
            DiffVersion::Original,
        ) {
            super::merge_highlight(&mut original_highlights, record);
        }
    }

    for row in 0..height {
        let Some((original, modified)) = rows.get(usize::from(row)) else {
            row_records.push(Record::Row {
                index: u32::from(row),
                original: None,
                modified: None,
            });
            continue;
        };
        row_records.push(Record::Row {
            index: u32::from(row),
            original: rendered_line_number(original),
            modified: rendered_line_number(modified),
        });

        let Some(line) = source_line(modified) else {
            continue;
        };
        if let Some(record) = super::semantic_highlight_record(
            if wrap {
                u32::from(width.saturating_sub(code_start))
            } else {
                u32::MAX
            },
            line,
            Side::Modified,
            alignment,
            modified,
            DiffVersion::Modified,
        ) {
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

fn render_width<T: AsRef<str>>(original: &[T], modified: &[T], wrap: bool) -> Result<u16> {
    let gutters = gutter_width(original.len() as u32) + gutter_width(modified.len() as u32);
    if wrap {
        return gutters
            .checked_add(WRAP_WIDTH)
            .context("inline wrapped parity width overflowed");
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
    let width = longest
        .checked_add(u32::from(gutters))
        .and_then(|width| width.checked_add(1))
        .context("inline parity width overflowed")?;
    Ok(u16::try_from(width.min(u32::from(PARITY_WIDTH)))?.max(MIN_WIDTH))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrapped_width_is_the_code_column_plus_both_gutters() {
        assert_eq!(render_width(&["long"], &["long"], true).unwrap(), 48);
    }

    #[test]
    fn unwrapped_width_keeps_a_long_line_visible() {
        let line = "x".repeat(250);
        assert_eq!(render_width(&[line.as_str()], &["x"], false).unwrap(), 259);
    }
}
