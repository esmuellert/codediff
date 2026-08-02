//! The bottom row.
//!
//! What a reviewer needs to know without asking: which file, where in it, and
//! how much is left to look at. Kept to one row, because every row it takes is
//! a row of diff it hides.

use crate::render::cells;
use crate::theme::Theme;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

/// What the status line says.
pub struct Status<'a> {
    pub path: &'a str,
    /// Cursor row and total rows, both 0-based internally, shown 1-based.
    pub row: u32,
    pub rows: u32,
    /// Runs of changed rows in the whole file.
    pub changes: usize,
    /// Which of them the cursor is in, if any, 0-based.
    pub change: Option<usize>,
    /// The engine gave up before finishing; what is shown is incomplete.
    pub timed_out: bool,
}

pub fn draw(buf: &mut Buffer, area: Rect, status: &Status<'_>, theme: &Theme) {
    let base = theme.status;
    cells::fill(buf, area, base);

    let mut x = cells::write(buf, area, 1, status.path, base.patch(theme.status_path));

    if status.timed_out {
        // Deliberately loud. A diff the engine abandoned is not a diff, and a
        // reviewer who mistakes one for a complete one will approve code they
        // have not seen.
        x = cells::write(
            buf,
            area,
            x + 2,
            "PARTIAL — diff timed out",
            base.patch(theme.warning),
        );
    }

    let right = summary(status);
    let width = right.chars().count() as u16;
    let at = area.width.saturating_sub(width + 1);
    if at > x + 1 {
        cells::write(buf, area, at, &right, base);
    }
}

fn summary(status: &Status<'_>) -> String {
    let position = format!("{}/{}", status.row + 1, status.rows.max(1));
    match (status.change, status.changes) {
        (_, 0) => format!("no changes   {position}"),
        (Some(i), n) => format!("change {}/{n}   {position}", i + 1),
        (None, 1) => format!("1 change   {position}"),
        (None, n) => format!("{n} changes   {position}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn status() -> Status<'static> {
        Status {
            path: "src/main.rs",
            row: 0,
            rows: 100,
            changes: 3,
            change: None,
            timed_out: false,
        }
    }

    fn render(status: &Status<'_>, width: u16) -> String {
        let area = Rect::new(0, 0, width, 1);
        let mut buf = Buffer::empty(area);
        draw(&mut buf, area, status, &Theme::DARK);
        (0..width).map(|x| buf[(x, 0)].symbol()).collect()
    }

    #[test]
    fn it_names_the_file_and_says_where_we_are() {
        let line = render(&status(), 60);
        assert!(line.contains("src/main.rs"));
        assert!(line.contains("3 changes"));
        assert!(line.contains("1/100"));
    }

    #[test]
    fn inside_a_hunk_it_counts_that_hunk_instead() {
        let line = render(
            &Status {
                change: Some(1),
                ..status()
            },
            60,
        );
        assert!(line.contains("change 2/3"), "{line:?}");
    }

    #[test]
    fn an_identical_file_says_so_rather_than_showing_a_zero() {
        let line = render(
            &Status {
                changes: 0,
                ..status()
            },
            60,
        );
        assert!(line.contains("no changes"), "{line:?}");
    }

    #[test]
    fn an_abandoned_diff_is_announced() {
        let line = render(
            &Status {
                timed_out: true,
                ..status()
            },
            80,
        );
        assert!(line.contains("PARTIAL"), "{line:?}");
    }

    #[test]
    fn a_narrow_terminal_drops_the_summary_rather_than_overlapping_the_path() {
        let line = render(&status(), 16);
        assert_eq!(line.chars().count(), 16);
        assert!(!line.contains("changes"), "{line:?}");
    }
}
