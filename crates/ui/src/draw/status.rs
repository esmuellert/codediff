//! The bottom row.
//!
//! What a reviewer needs to know without asking: which file, where in it, and
//! how much is left to look at. Kept to one row, because every row it takes is
//! a row of diff it hides.

use file_types::File;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;

use crate::render::cells;
use crate::theme::Theme;
use crate::view::Direction;

/// What the status line says.
pub struct Status<'a> {
    /// Which file, as structure rather than a formatted name.
    ///
    /// The whole reason this is a [`File`] and not a `&str`: the directory is
    /// dimmed while the name is not, and the directory is dropped first when
    /// the row is too narrow. A string could support neither. It used to be
    /// one — `"old.rs → new.rs   (added)"` — and the `(added)` was rendered
    /// bold, as though it were part of the path. See D28.
    pub file: &'a File,
    /// Cursor position and document height, in view lines. Both 0-based
    /// internally, shown 1-based.
    pub view_line: u32,
    pub view_lines: u32,
    /// Runs of changed view lines in the whole file.
    pub changes: usize,
    /// Which of them the cursor is in, if any, 0-based.
    pub change: Option<usize>,
    /// The engine gave up before finishing; what is shown is incomplete.
    pub timed_out: bool,
    /// A change-navigation key that had nowhere to go, if the last one did.
    ///
    /// Shown instead of the change counter, since it answers the key that was
    /// just pressed and the counter would only repeat what it already said.
    pub exhausted: Option<Direction>,
}

pub fn draw(buf: &mut Buffer, area: Rect, status: &Status<'_>, theme: &Theme) {
    let base = theme.status;
    cells::fill(buf, area, base);

    let right = summary(status);
    // The room the name has, once the position on the right is accounted for.
    let room = area.width.saturating_sub(right.chars().count() as u16 + 3);
    let mut x = name(buf, area, status.file, room, base, theme);

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

    let width = right.chars().count() as u16;
    let at = area.width.saturating_sub(width + 1);
    if at > x + 1 {
        cells::write(buf, area, at, &right, base);
    }
}

/// Writes which file this is, in as much detail as the width allows.
///
/// Three independent parts, dropped in order of what a reviewer can most
/// afford to lose:
///
/// 1. the directory, dimmed — recoverable from the file name plus keymap_type
/// 2. `from → ` for a rename — useful, rarely essential
/// 3. the file name and any `(added)`/`(deleted)` note — never dropped
///
/// This is what a pre-formatted string cannot do, and why [`File`] carries the
/// facts rather than a label.
fn name(
    buf: &mut Buffer,
    area: Rect,
    file: &File,
    room: u16,
    base: ratatui::style::Style,
    theme: &Theme,
) -> u16 {
    let path = file.path();
    let note = match file.only() {
        Some(file_types::DiffVersion::Modified) => "   (added)",
        Some(file_types::DiffVersion::Original) => "   (deleted)",
        None => "",
    };

    let essential = path.file_name().chars().count() + note.chars().count();
    let mut x = 1;

    // Widest form first, narrowing until it fits.
    let previous = file.previous_path().map(|p| format!("{p} → "));
    let directory = path.directory();
    let with_directory = !directory.is_empty()
        && essential
            + directory.chars().count()
            + 1
            + previous.as_ref().map_or(0, |p| p.chars().count())
            <= room as usize;

    if let Some(previous) = previous.filter(|p| essential + p.chars().count() <= room as usize) {
        x = cells::write(buf, area, x, &previous, base.patch(theme.status_path));
    }
    if with_directory {
        x = cells::write(buf, area, x, directory, base.patch(theme.divider));
        x = cells::write(buf, area, x, "/", base.patch(theme.divider));
    }
    x = cells::write(
        buf,
        area,
        x,
        path.file_name(),
        base.patch(theme.status_path),
    );
    if !note.is_empty() {
        x = cells::write(buf, area, x, note, base);
    }
    x
}

fn summary(status: &Status<'_>) -> String {
    let position = format!("{}/{}", status.view_line + 1, status.view_lines.max(1));
    if let Some(direction) = status.exhausted {
        let which = match direction {
            Direction::Next => "next",
            Direction::Previous => "previous",
        };
        return format!("no {which} change   {position}");
    }
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
    use file_types::RepoPath;
    use std::path::Path;
    use std::sync::LazyLock;

    fn at(relative: &str) -> RepoPath {
        RepoPath::new(relative, Path::new("/repo"))
    }

    static MAIN: LazyLock<File> = LazyLock::new(|| File::unchanged_path(at("src/main.rs")));

    fn status() -> Status<'static> {
        Status {
            file: &MAIN,
            view_line: 0,
            view_lines: 100,
            changes: 3,
            change: None,
            timed_out: false,
            exhausted: None,
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
    fn a_change_key_with_nowhere_to_go_says_so_instead_of_counting() {
        // The counter would only repeat what it already said, leaving the
        // reader unable to tell a key that did nothing from one that is not
        // bound at all.
        for (direction, expected) in [
            (Direction::Next, "no next change"),
            (Direction::Previous, "no previous change"),
        ] {
            let line = render(
                &Status {
                    change: Some(2),
                    exhausted: Some(direction),
                    ..status()
                },
                60,
            );
            assert!(line.contains(expected), "{line:?}");
            assert!(!line.contains("change 3/3"), "{line:?}");
            assert!(line.contains("1/100"), "the position stays: {line:?}");
        }
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

    #[test]
    fn the_directory_goes_before_the_file_name_does() {
        // The whole point of holding a `File` rather than a formatted string.
        // A reviewer who loses the directory can still tell which file this
        // is; one who loses the name cannot.
        let deep = File::unchanged_path(at("crates/ui/src/render/status.rs"));
        let wide = render(
            &Status {
                file: &deep,
                ..status()
            },
            70,
        );
        assert!(wide.contains("crates/ui/src/render"), "{wide:?}");
        assert!(wide.contains("status.rs"), "{wide:?}");

        let narrow = render(
            &Status {
                file: &deep,
                ..status()
            },
            30,
        );
        assert!(
            narrow.contains("status.rs"),
            "the name survives: {narrow:?}"
        );
        assert!(
            !narrow.contains("crates/ui/src/render"),
            "the directory was dropped: {narrow:?}"
        );
    }

    #[test]
    fn a_one_sided_file_says_which_it_is() {
        let added = File::added(at("new.rs"));
        let line = render(
            &Status {
                file: &added,
                ..status()
            },
            60,
        );
        assert!(line.contains("new.rs"), "{line:?}");
        assert!(line.contains("(added)"), "{line:?}");

        let gone = File::deleted(at("old.rs"));
        let line = render(
            &Status {
                file: &gone,
                ..status()
            },
            60,
        );
        assert!(line.contains("(deleted)"), "{line:?}");
    }

    #[test]
    fn a_rename_shows_both_names_when_there_is_room() {
        let moved = File::renamed(at("old.rs"), at("new.rs"));
        let wide = render(
            &Status {
                file: &moved,
                ..status()
            },
            70,
        );
        assert!(wide.contains("old.rs"), "{wide:?}");
        assert!(wide.contains("→"), "{wide:?}");
        assert!(wide.contains("new.rs"), "{wide:?}");

        // Too narrow for both: the name it has *now* is the one that stays.
        let narrow = render(
            &Status {
                file: &moved,
                ..status()
            },
            24,
        );
        assert!(narrow.contains("new.rs"), "{narrow:?}");
        assert!(!narrow.contains("→"), "{narrow:?}");
    }
}
