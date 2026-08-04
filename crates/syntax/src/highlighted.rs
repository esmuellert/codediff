//! A file being coloured from the top, as far as anyone has looked.
//!
//! This is the whole of what VS Code needs a state store, an invalidation
//! queue and a convergence check for — and it is two fields, because we never
//! invalidate. A file under review is a snapshot: a git blob, or a worktree
//! file as it was when we read it. Nothing is typed into it, so the answer for
//! line 40 never changes, and a prefix once read is read for good.
//!
//! What remains is that the answer for line 500 depends on lines 1 to 499, so
//! reading can only ever go **forwards**. Scrolling back is free; jumping
//! ahead costs the gap, once.

use crate::engine::{Engine, Grammar, Palette, Reading};
use crate::limits;
use crate::style::Span;

/// One version of one file, coloured as far as it has been read.
pub struct Highlighted {
    /// Spans for lines `0..read.len()`, in order.
    read: Vec<Vec<Span>>,
    /// Where the engine got to, or `None` once there is nothing more to do —
    /// either the file is finished, or it was never worth starting.
    reading: Option<Box<Reading>>,
}

impl std::fmt::Debug for Highlighted {
    /// How far it has got, not every span it found.
    ///
    /// Written out rather than derived because the derived form is tens of
    /// thousands of byte ranges, which no failing test is easier to read for.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Highlighted")
            .field("read", &self.read.len())
            .field("finished", &self.finished())
            .finish()
    }
}

impl Highlighted {
    /// A file nobody will colour.
    ///
    /// What an unrecognised language, a binary file and a database dump all
    /// get. Not an error: every caller must already cope with having no spans
    /// for a line, because the first frame is drawn before the whole file has
    /// been read.
    pub fn none() -> Self {
        Self {
            read: Vec::new(),
            reading: None,
        }
    }

    /// Begins colouring a file, if it is worth colouring.
    pub fn new(engine: &Engine, grammar: Grammar, palette: &Palette, lines: &[String]) -> Self {
        let bytes = lines.iter().map(|line| line.len() + 1).sum();
        if !limits::worth_highlighting(bytes, lines.len()) {
            return Self::none();
        }
        Self {
            read: Vec::with_capacity(lines.len()),
            reading: Some(Box::new(engine.start(grammar, palette))),
        }
    }

    /// How the given line is coloured, or nothing if it has not been read.
    ///
    /// Nothing is the ordinary answer for a line below the point reached so
    /// far, and means "draw it plainly" rather than "this line has no colour".
    pub fn line(&self, line: u32) -> &[Span] {
        self.read
            .get(line as usize)
            .map(Vec::as_slice)
            .unwrap_or_default()
    }

    /// How many lines have been read.
    pub fn done(&self) -> u32 {
        self.read.len() as u32
    }

    /// Whether there is anything left to read.
    pub fn finished(&self) -> bool {
        self.reading.is_none()
    }

    /// Reads until `line` has been coloured, or until this frame has done
    /// enough.
    ///
    /// What a frame calls before drawing, with the last line it is about to
    /// show. Cheap when the answer is already known, which after the first
    /// frame it usually is.
    ///
    /// Stops after [`limits::LEAP`] lines and returns rather than holding the
    /// frame: a reader who jumps to the end of a very long file sees the text
    /// at once and its colour a moment later, which is the trade VS Code
    /// makes. [`caught_up`](Self::caught_up) is how a caller finds out that it
    /// happened.
    pub fn reach(&mut self, engine: &Engine, palette: &Palette, line: u32, lines: &[String]) {
        let want = line as usize + 1;
        self.read_to(
            engine,
            palette,
            want.min(self.read.len() + limits::LEAP),
            lines,
        );
    }

    /// Whether the given line has been coloured yet.
    ///
    /// False only just after a leap through a very long file. A caller that
    /// draws frames uses it to know that one more is worth drawing once the
    /// idle pass has caught up.
    pub fn caught_up(&self, line: u32) -> bool {
        self.finished() || self.read.len() > line as usize
    }

    /// Reads a little more, and says whether that changed anything.
    ///
    /// What an idle moment calls. The slice is small enough that a keypress
    /// arriving mid-file is answered promptly, and large enough that a file
    /// finishes in a handful of them — VS Code budgets by milliseconds because
    /// its engine is four times slower and it must not block a browser; a
    /// fixed count is enough here and needs no clock, which matters because
    /// the crate has none.
    pub fn read_more(&mut self, engine: &Engine, palette: &Palette, lines: &[String]) -> bool {
        if self.finished() {
            return false;
        }
        let target = self.read.len() + SLICE;
        self.read_to(engine, palette, target, lines);
        true
    }

    fn read_to(&mut self, engine: &Engine, palette: &Palette, target: usize, lines: &[String]) {
        let target = target.min(lines.len());
        if self.read.len() >= target {
            return;
        }
        let Some(reading) = self.reading.as_mut() else {
            return;
        };
        engine.read(
            reading,
            palette,
            &lines[self.read.len()..target],
            &mut self.read,
        );
        if self.read.len() == lines.len() {
            // Nothing left to carry forward. Dropping it returns the grammar's
            // context stack, which for a deeply nested file is not nothing.
            self.reading = None;
        }
    }
}

/// Lines read per idle slice.
///
/// A slice happens between a `poll` and the keypress that ends it, so its cost
/// is added to the latency of whatever the reader presses next. At the 18 500
/// lines a second this engine measures with a real theme, VS Code's 200 lines
/// would be **13 ms** — a whole frame of delay, which is exactly the sort of
/// thing that makes a terminal program feel sticky.
///
/// Sixty-four is about three and a half milliseconds, which is not noticeable,
/// and still colours some eleven thousand lines for every second the reader
/// spends deciding what to press. Finishing sooner is not the goal; the goal
/// is that nothing waits for it.
const SLICE: usize = 64;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::detect::Clues;
    use crate::style::{Pen, Rule, Style};

    fn palette() -> Palette {
        // `storage` as well as `keyword`, because Rust's `fn` is
        // `storage.type.function` — the kind of thing a scope path knows and a
        // fixed list of token names does not.
        let word = Style::pen(Pen(0));
        Palette::new(&[Rule::new("keyword", word), Rule::new("storage", word)])
    }

    fn rust(lines: &[&str]) -> (Engine, Palette, Vec<String>, Highlighted) {
        let engine = Engine::new();
        let palette = palette();
        let owned: Vec<String> = lines.iter().map(|l| (*l).to_owned()).collect();
        let grammar = engine
            .find(Clues::new("a.rs", None))
            .expect("rust is a language");
        let highlighted = Highlighted::new(&engine, grammar, &palette, &owned);
        (engine, palette, owned, highlighted)
    }

    #[test]
    fn nothing_is_read_until_someone_looks() {
        let (_, _, _, h) = rust(&["fn a() {}", "fn b() {}"]);
        assert_eq!(h.done(), 0);
        assert!(h.line(0).is_empty(), "not read yet, so nothing to say");
    }

    #[test]
    fn reaching_a_line_reads_everything_up_to_it() {
        let (engine, palette, lines, mut h) = rust(&["fn a() {}", "fn b() {}", "fn c() {}"]);
        h.reach(&engine, &palette, 1, &lines);
        assert_eq!(h.done(), 2);
        assert!(!h.line(0).is_empty(), "`fn` is a keyword");
        assert!(h.line(2).is_empty(), "not reached");
    }

    #[test]
    fn reaching_a_line_already_read_does_nothing() {
        let (engine, palette, lines, mut h) = rust(&["fn a() {}", "fn b() {}"]);
        h.reach(&engine, &palette, 1, &lines);
        let spans = h.line(0).to_vec();
        h.reach(&engine, &palette, 0, &lines);
        assert_eq!(h.done(), 2, "did not go backwards");
        assert_eq!(h.line(0), spans, "and did not change its mind");
    }

    #[test]
    fn a_file_read_to_its_end_reports_finished() {
        let (engine, palette, lines, mut h) = rust(&["fn a() {}"]);
        assert!(!h.finished());
        h.reach(&engine, &palette, 0, &lines);
        assert!(h.finished(), "nothing left to carry forward");
        assert!(!h.read_more(&engine, &palette, &lines));
    }

    #[test]
    fn idle_reading_gets_there_in_slices() {
        let long: Vec<&str> = std::iter::repeat_n("fn a() {}", SLICE + 10).collect();
        let (engine, palette, lines, mut h) = rust(&long);
        assert!(h.read_more(&engine, &palette, &lines));
        assert_eq!(h.done() as usize, SLICE);
        assert!(h.read_more(&engine, &palette, &lines));
        assert_eq!(h.done() as usize, lines.len());
        assert!(h.finished());
    }

    #[test]
    fn one_frame_does_not_colour_a_whole_enormous_file() {
        // The freeze this cap exists to prevent. Asking for the last line of
        // a file far longer than a leap must come back having done a leap's
        // worth of work, not all of it.
        let long: Vec<&str> = std::iter::repeat_n("fn a() {}", limits::LEAP * 2).collect();
        let (engine, palette, lines, mut h) = rust(&long);
        h.reach(&engine, &palette, lines.len() as u32 - 1, &lines);
        assert_eq!(h.done() as usize, limits::LEAP, "did what it could");
        assert!(!h.caught_up(lines.len() as u32 - 1), "and says so");

        // The idle pass finishes it, and then the answer is there.
        while h.read_more(&engine, &palette, &lines) {}
        assert!(h.caught_up(lines.len() as u32 - 1));
    }

    #[test]
    fn an_ordinary_file_is_never_deferred() {
        let (engine, palette, lines, mut h) = rust(&["fn a() {}", "fn b() {}"]);
        h.reach(&engine, &palette, 1, &lines);
        assert!(h.caught_up(1));
    }

    #[test]
    fn a_file_nobody_colours_answers_for_every_line() {
        let h = Highlighted::none();
        assert!(h.finished());
        assert!(h.line(0).is_empty());
        assert!(h.line(9_999).is_empty());
    }
}
