//! Colouring the file a pane is showing.
//!
//! ---
//!
//! The `syntax` crate reads a file and reports which of the caller's pens each
//! run of text wears; [`theme::code`](crate::theme::code) says which scope
//! earns which pen, and which colour that is. This module is the join: it owns
//! the two process-wide tables and lends them to whoever holds the lines.
//!
//! **Why the tables are global.** The grammars are twelve megabytes of
//! immutable data that take a millisecond to unpack, and the scope table is a
//! constant — neither depends on the theme, because a span names a pen rather
//! than a colour. Passing them down through the view would put a lifetime on
//! every buffer to say something that is true for the whole process, and a
//! second copy per file in an explorer would be twelve megabytes each. They
//! are built on first use and never change, which is the only kind of global
//! this program has.
//!
//! **Nothing here is cached against a theme.** Change theme and every span
//! stays valid; only [`Code::pen`](crate::theme::Code::pen) answers
//! differently. That is the whole payoff of a pen being a number.

use std::sync::OnceLock;

use align::DiffVersion;
use file_types::File;
use syntax::{Clues, Engine, Highlighted, Palette, Span};

/// Every grammar, unpacked once.
fn engine() -> &'static Engine {
    static ENGINE: OnceLock<Engine> = OnceLock::new();
    ENGINE.get_or_init(Engine::new)
}

/// The scope table, compiled once.
fn palette() -> &'static Palette {
    static PALETTE: OnceLock<Palette> = OnceLock::new();
    PALETTE.get_or_init(|| Palette::new(&crate::theme::scopes::rules()))
}

/// Begins colouring one version of one file.
///
/// A file whose language nothing claims — a `.lock`, a log, an unknown
/// extension — gets [`Highlighted::none`], which answers "no spans" for every
/// line. Not an error: every caller already copes with a line that has not
/// been read yet, and this is the same answer for a different reason.
pub fn begin(file: &File, version: DiffVersion, lines: &[String]) -> Highlighted {
    // The path on *this* side, so a rename is read as what it became rather
    // than as what it was: `main.py` renamed to `main.rs` is Python on the
    // left and Rust on the right, and showing either as the other would be a
    // lie the reader could see.
    let Some(path) = file.on(version) else {
        return Highlighted::none();
    };
    let clues = Clues::new(path.as_str(), lines.first().map(String::as_str));
    match engine().find(clues) {
        Some(grammar) => Highlighted::new(engine(), grammar, palette(), lines),
        None => Highlighted::none(),
    }
}

/// Colours up to and including line `number`, if it is not coloured already.
///
/// What a frame calls before drawing, with the last line it is about to show.
///
/// **Numbered from 1**, like [`Alignment::line`] and like the gutter, and
/// unlike `syntax`, which indexes from 0. The two conventions meet here and
/// nowhere else, which is the point of this module having the conversion in
/// it: written at each call site instead, it was wrong at one of them, and a
/// whole file coloured one line out still looks coloured.
///
/// [`Alignment::line`]: align::Alignment::line
pub fn reach(read: &mut Highlighted, number: u32, lines: &[String]) {
    let Some(index) = number.checked_sub(1) else {
        return;
    };
    read.reach(engine(), palette(), index, lines);
}

/// Whether line `number` has been coloured yet, numbered from 1.
pub fn caught_up(read: &Highlighted, number: u32) -> bool {
    match number.checked_sub(1) {
        Some(index) => read.caught_up(index),
        None => true,
    }
}

/// Colours a little more, and says whether there was anything to do.
///
/// What an idle moment calls.
pub fn read_more(read: &mut Highlighted, lines: &[String]) -> bool {
    read.read_more(engine(), palette(), lines)
}

/// The colouring of the versions a pane is drawing, for the frame.
///
/// Borrowed rather than owned so a renderer can be handed it without learning
/// what a diff is. `Off` is what the toggle produces and what a buffer with
/// nothing to colour reports; both draw plainly, and neither is a special case
/// anywhere below.
#[derive(Clone, Copy, Default)]
pub enum Spans<'a> {
    #[default]
    Off,
    /// One version, which is what a lone file has.
    One(&'a Highlighted),
    /// Both, which is what a diff has.
    Both {
        original: &'a Highlighted,
        modified: &'a Highlighted,
    },
}

impl<'a> Spans<'a> {
    /// How line `number` of one version is coloured.
    ///
    /// **Numbered from 1** — see [`reach`].
    pub fn line(&self, version: DiffVersion, number: u32) -> &'a [Span] {
        let Some(index) = number.checked_sub(1) else {
            return &[];
        };
        match self {
            Spans::Off => &[],
            Spans::One(read) => read.line(index),
            Spans::Both { original, modified } => match version {
                DiffVersion::Original => original.line(index),
                DiffVersion::Modified => modified.line(index),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use file_types::RepoPath;
    use std::path::Path;

    fn file(name: &str) -> File {
        File::unchanged_path(RepoPath::new(name, Path::new("/repo")))
    }

    fn lines(source: &str) -> Vec<String> {
        source.lines().map(str::to_owned).collect()
    }

    #[test]
    fn a_file_whose_language_we_know_is_coloured() {
        let lines = lines("fn main() {\n    let x = 1;\n}\n");
        let mut read = begin(&file("src/main.rs"), DiffVersion::Modified, &lines);
        reach(&mut read, 1, &lines);
        assert!(!read.line(0).is_empty(), "`fn` is a keyword");
    }

    #[test]
    fn line_one_is_the_first_line_and_not_the_second() {
        // The conversion this module exists for. `align` numbers lines from
        // one and `syntax` indexes from zero, and getting it wrong colours the
        // whole file one line out — which still *looks* coloured, so only an
        // assertion about which line can catch it.
        let lines = lines("fn one() {}\n\"two\"\n");
        let mut read = begin(&file("src/main.rs"), DiffVersion::Modified, &lines);
        reach(&mut read, 2, &lines);
        let spans = Spans::One(&read);

        let first = spans.line(DiffVersion::Modified, 1);
        let second = spans.line(DiffVersion::Modified, 2);
        assert!(!first.is_empty() && !second.is_empty(), "both were read");
        assert_ne!(first, second, "and they are different lines");
        assert!(
            spans.line(DiffVersion::Modified, 0).is_empty(),
            "there is no line zero"
        );
    }

    #[test]
    fn a_file_whose_language_we_do_not_know_is_left_plain() {
        let lines = lines("some prose\nand more of it\n");
        let mut read = begin(&file("notes.qqzz"), DiffVersion::Modified, &lines);
        reach(&mut read, 2, &lines);
        assert!(read.finished(), "nothing to do");
        assert!(read.line(0).is_empty());
    }

    #[test]
    fn a_rename_is_read_as_the_language_of_each_side() {
        // The reason `begin` takes a version. A `.py` renamed to `.rs` has to
        // be Python on the left and Rust on the right; one grammar for both
        // would mis-colour whichever side lost.
        let file = File::renamed(
            RepoPath::new("a.py", Path::new("/repo")),
            RepoPath::new("a.rs", Path::new("/repo")),
        );
        let python = lines("def f():\n    pass\n");
        let rust = lines("fn f() {}\n");

        let mut left = begin(&file, DiffVersion::Original, &python);
        let mut right = begin(&file, DiffVersion::Modified, &rust);
        reach(&mut left, 1, &python);
        reach(&mut right, 1, &rust);
        assert!(!left.line(0).is_empty());
        assert!(!right.line(0).is_empty());
    }

    #[test]
    fn a_side_that_does_not_exist_is_not_coloured() {
        let added = File::added(RepoPath::new("a.rs", Path::new("/repo")));
        let read = begin(&added, DiffVersion::Original, &lines("fn f() {}\n"));
        assert!(read.finished());
    }

    #[test]
    fn switched_off_answers_for_every_line_without_a_file() {
        let off = Spans::Off;
        assert!(off.line(DiffVersion::Original, 0).is_empty());
        assert!(off.line(DiffVersion::Modified, 9_999).is_empty());
    }
}
