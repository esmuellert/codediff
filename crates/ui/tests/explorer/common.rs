//! What every explorer test needs: a repository's worth of entries, and a
//! way to draw a session and read the screen back.
//!
//! Shared by the three files beside it rather than by one of them, so that
//! none of the three is the one that happens to own the fixtures.

pub use file_types::{ChangeType, DiffType, File, Oid, RepoPath, Rev, Revs, Stats};
pub use pipeline::file::{DiffContent, Files};
pub use ratatui::buffer::Buffer as Cells;
pub use ratatui::layout::Rect;
pub use ratatui::style::Color;
pub use std::path::Path;
pub use ui::{Buffer, Session, Theme};

/// The two comparisons `codediff` with no arguments produces.
///
/// A group *is* a revision pair, so these are what put a file in one group
/// rather than the other. Nothing stamps a category on a file: there is one
/// answer, and it is the one the file already carries.
pub fn revs() -> Revs {
    Revs::new(Rev::Index, Rev::Worktree)
}

pub fn staged_revs() -> Revs {
    Revs::new(Rev::Commit(Oid::new("b87b24c")), Rev::Index)
}

pub fn at(relative: &str) -> RepoPath {
    RepoPath::new(relative, Path::new("/repo"))
}

pub fn modified(path: &str) -> File {
    File::unchanged_path(at(path), revs())
}

/// The same file, in the staged comparison instead of the unstaged one.
pub fn staged(path: &str) -> File {
    File::unchanged_path(at(path), staged_revs())
}

pub fn untracked(path: &str) -> File {
    File::added(at(path), revs()).set_change_type(ChangeType::Untracked)
}

/// One group's worth, as a comparison of two revisions produces.
pub fn only(files: Vec<File>) -> Vec<File> {
    files
}

pub fn entries() -> Vec<File> {
    vec![
        modified("src/app.rs").set_stats(Stats::new(12, 3)),
        modified("src/view/tab.rs").set_stats(Stats::new(4, 0)),
        untracked("notes.txt"),
        staged("README.md").set_stats(Stats::new(1, 1)),
    ]
}

/// Draws a session and returns the screen as text, one string per row.
pub fn screen(session: &mut Session, width: u16, height: u16) -> Vec<String> {
    let mut cells = Cells::empty(Rect::new(0, 0, width, height));
    session.draw_into(&mut cells, Rect::new(0, 0, width, height));
    (0..height)
        .map(|y| {
            (0..width)
                .map(|x| cells[(x, y)].symbol())
                .collect::<String>()
                .trim_end()
                .to_owned()
        })
        .collect()
}

/// The file a [`modified`] row is built for, so its comparison can be
/// scripted with the same revisions the row carries.
pub fn unchanged(path: &str) -> File {
    File::unchanged_path(at(path), revs())
}

/// One version of a file, as the worker would answer with it.
///
/// What a test hands to [`Files::canned`], in the order it expects rows to be
/// opened. No repository is touched and no engine is run — what these check is
/// what a pane shows, not how its contents were obtained.
pub fn single_file(file: File, text: &str) -> Result<DiffContent, String> {
    let lines = text.lines().map(str::to_owned).collect();
    Ok(DiffContent::SingleFile(pipeline::file::SingleFile {
        file,
        lines: std::sync::Arc::new(lines),
    }))
}

/// A two-sided comparison, which is what a layout key has anything to say
/// about.
///
/// A file against itself: no engine is run, because `ui` may not name one, and
/// none is needed — what this carries is a layout, not a pairing.
pub fn diff(file: File, text: &str) -> Result<DiffContent, String> {
    let lines: Vec<&str> = text.lines().collect();
    let alignment = align::Alignment::new(
        diff_types::LinesDiff {
            changes: Vec::new(),
            moves: Vec::new(),
            hit_timeout: false,
        },
        &lines,
        &lines,
    );
    Ok(DiffContent::Diff(pipeline::file::Diff { file, alignment }))
}

/// A session over `groups`, whose worker answers from a script.
///
/// The theme stays at the call site: which one a colour test uses is part of
/// what it checks — in `basic-dark` the comment, the line number and the
/// indent guide are all `DarkGray`, so a test written against it can match the
/// wrong thing and never fail.
pub fn scripted(
    files: Vec<File>,
    theme: Theme,
    script: Vec<Result<DiffContent, String>>,
) -> Session {
    Session::with_files(Buffer::explorer(files), theme, Files::canned(script))
}

/// Opens the selected row and waits for its comparison.
///
/// The two calls the loop makes, without a terminal between them. The wait is
/// what a test may do and the interface may not: the comparison is on a thread
/// of its own, and an assertion about a pane has to know when to look.
pub fn open_selected(session: &mut Session) {
    session.open();
    assert!(session.opened(), "nothing was installed");
}

/// The column `needle` starts at, which is not where `str::find` puts it.
///
/// `find` answers in bytes, and the divider between the panes is three of
/// them. Reading a cell at a byte offset lands one column left of the text and
/// picks up the colour of whatever is there.
pub fn column_of(row: &str, needle: &str) -> u16 {
    let byte = row
        .find(needle)
        .unwrap_or_else(|| panic!("no {needle:?} in {row:?}"));
    row[..byte].chars().count() as u16
}
