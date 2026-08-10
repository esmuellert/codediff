//! Building a session, and reading the screen back out.
//!
//! Shared by every test that draws. Here rather than in `ui` because
//! making a diff buffer means computing a diff, and `ui` is forbidden
//! from depending on the crate that does — see `cargo xtask lint-arch`. The
//! composition root is the one place allowed to name every layer.
//!
//! An [`Alignment`] owns its two files, so these helpers return a buffer
//! outright and there is no borrow to keep alive at the call site. Before it
//! owned them, every one of these functions would have needed a lifetime, and
//! so would every local holding the result. See D27.
//!
//! [`Alignment`]: align::Alignment

#![allow(dead_code)]

use file_types::{File, Oid, RepoPath, Revs};
use pipeline::file::DiffContent;
use ui::crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use ui::ratatui::Terminal;
use ui::ratatui::backend::TestBackend;
use ui::ratatui::buffer::Buffer as Cells;
use ui::testing::TestSession;
use ui::{Buffer, Session, Theme};

/// The ordinary comparison, since these tests never open a repository.
pub fn revs() -> Revs {
    Revs::worktree_against(Oid::new("b87b24c"))
}

/// A file under a fixed root, since these tests never touch a disk.
pub fn file(path: &str) -> File {
    File::unchanged_path(RepoPath::new(path, std::path::Path::new("/repo")), revs())
}

/// A side-by-side buffer over two texts.
pub fn diff(label: &str, before: &str, after: &str) -> Buffer {
    let original = vscode_diff::lines(before);
    let modified = vscode_diff::lines(after);
    let computed = vscode_diff::compute(&original, &modified, &vscode_diff::Options::default())
        .expect("the engine runs");
    with_diff(label, before, after, computed)
}

/// A side-by-side buffer over a diff the caller made, for the cases a real
/// engine run cannot produce — an abandoned diff, say.
pub fn with_diff(
    label: &str,
    before: &str,
    after: &str,
    computed: vscode_diff::LinesDiff,
) -> Buffer {
    let original = vscode_diff::lines(before);
    let modified = vscode_diff::lines(after);
    let alignment = align::Alignment::new(computed, &original, &modified);
    Buffer::diff(DiffContent::Diff(pipeline::file::Diff {
        file: file(label),
        alignment,
    }))
}

/// A buffer for a file present on both sides, shown alone.
pub fn single(label: &str, contents: &str) -> Buffer {
    Buffer::diff(lone(file(label), contents))
}

/// A buffer for a file that exists only on the modified side.
///
/// The `(added)` note is derived from that, never passed in — which is the
/// point of `File` and the reason a test cannot fake it with a label.
pub fn added(label: &str, contents: &str) -> Buffer {
    let file = File::added(RepoPath::new(label, std::path::Path::new("/repo")), revs());
    Buffer::diff(lone(file, contents))
}

/// One version of a file, as the pipeline would hand it over.
fn lone(file: File, contents: &str) -> DiffContent {
    DiffContent::SingleFile(pipeline::file::SingleFile {
        file,
        lines: std::sync::Arc::new(
            vscode_diff::lines(contents)
                .iter()
                .map(|line| (*line).to_owned())
                .collect(),
        ),
    })
}

/// A session over a side-by-side buffer, in the default dark theme.
pub fn session(label: &str, before: &str, after: &str) -> TestSession {
    TestSession::new(diff(label, before, after), Theme::DARK)
}

/// The screen as a grid of cells, for asserting colours.
pub fn cells(session: &mut Session, width: u16, height: u16) -> Cells {
    let mut terminal = Terminal::new(TestBackend::new(width, height)).expect("in-memory terminal");
    session.draw(&mut terminal).expect("draws");
    terminal.backend().buffer().clone()
}

/// The screen as text, one line per line.
pub fn screen(session: &mut Session, width: u16, height: u16) -> String {
    let cells = cells(session, width, height);
    (0..height)
        .map(|y| {
            (0..width)
                .map(|x| cells[(x, y)].symbol())
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub fn key(code: KeyCode) -> Event {
    Event::Key(KeyEvent::new(code, KeyModifiers::NONE))
}

/// Types a line of keys. `\x1b` is escape; everything else is that character.
pub fn type_keys(session: &mut TestSession, keys: &str) -> ui::Flow {
    let mut flow = ui::Flow::Continue;
    for c in keys.chars() {
        let code = match c {
            '\u{1b}' => KeyCode::Esc,
            other => KeyCode::Char(other),
        };
        // Shift is how a terminal reports a capital, and the table is written
        // in that form; without it `G` would never match.
        let modifiers = if c.is_ascii_uppercase() {
            KeyModifiers::SHIFT
        } else {
            KeyModifiers::NONE
        };
        flow = session.handle_event(&Event::Key(KeyEvent::new(code, modifiers)));
    }
    flow
}

/// Gives the session a height, which page motions and scrolling need.
///
/// A viewport learns its height from a frame rather than from an event, so a
/// test that never drew would page by nothing at all.
pub fn measure(session: &mut TestSession) {
    let mut terminal = Terminal::new(TestBackend::new(60, 12)).expect("in-memory terminal");
    session.draw(&mut terminal).expect("draws");
}

/// The grid column a character sits in, rather than its byte offset.
///
/// `╱` and `│` are three bytes each, so byte offsets would be wrong in exactly
/// the lines these tests care about.
pub fn column_of(line: &str, needle: char) -> Option<usize> {
    line.chars().position(|c| c == needle)
}
