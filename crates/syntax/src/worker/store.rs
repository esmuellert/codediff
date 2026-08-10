//! LRU cache of syntax spans for open files.
//!
//! All of the cache is on this side. A frame that finds what it needs here
//! draws and sends nothing, so switching between two files costs one lookup
//! rather than two parses. The worker keeps no results at all — only its place
//! in a file it has not finished, which is a bookmark and not a copy.
//!
//! Entries are dropped least-recently-used, capped by total lines cached rather than
//! by number of files, because files differ by three orders of magnitude and
//! counting them measures nothing. The file being coloured is never dropped: it is
//! used every frame, so it is never the least recent, and that falls out of
//! the ordering instead of needing a rule.

use std::collections::HashMap;

use crate::Span;
use align::DiffVersion;

use super::message::{SyntaxResponse, Version};

/// Max cached lines before the LRU file is evicted.
///
/// Spans measured at 3.5 per line on our own source, so roughly 80 bytes a
/// line: eight hundred thousand lines is about sixty-four megabytes. Generous
/// on purpose — dropping an entry costs a re-parse, and a review moves between
/// a handful of files far more often than it opens a thousand.
const BUDGET: usize = 800_000;

/// One version of one file, coloured as far as the answers have arrived.
///
/// Spans and nothing else. Deliberately not a `Highlighted`: that holds an
/// engine's position, which is the worker's business, and the interface has no
/// use for one when it is not doing the colouring.
#[derive(Debug, Default)]
pub struct Colours {
    lines: Vec<Vec<Span>>,
    /// Which content these describe. An answer for anything else is thrown
    /// away rather than mixed in.
    version: Version,
}

impl Colours {
    /// How the given line is coloured, or nothing if it has not arrived.
    ///
    /// Nothing is the ordinary answer for a line not yet reached, and means
    /// "draw it plainly" rather than "this line has no colour".
    pub fn line(&self, line: u32) -> &[Span] {
        self.lines
            .get(line as usize)
            .map(Vec::as_slice)
            .unwrap_or_default()
    }

    /// How many lines have arrived.
    pub fn lines(&self) -> u32 {
        self.lines.len() as u32
    }

    /// Adds a piece, and says whether it was taken.
    ///
    /// Refused when it is for other content, or when it does not begin exactly
    /// where the last one ended. The worker sends in order, but a stale answer
    /// must not be able to shorten or reorder what is already drawn.
    fn install(&mut self, response: SyntaxResponse) -> bool {
        if response.version != self.version || response.from as usize != self.lines.len() {
            return false;
        }
        self.lines.extend(response.spans);
        true
    }
}

/// The colours of every file open, and the order they were last used in.
#[derive(Debug, Default)]
pub struct Store {
    entries: HashMap<String, Colours>,
    /// Most recently used last. A `Vec` rather than a queue because it is
    /// searched by key every time one is used, and at the handful of entries a review
    /// holds a scan beats a second index.
    order: Vec<String>,
    cached_lines: usize,
}

impl Store {
    pub fn new() -> Self {
        Self::default()
    }

    /// The colours for one file, if any have arrived.
    ///
    /// Does not count as a use. Drawing asks for this many times a frame, and
    /// what should keep an entry alive is a reader looking at the file, which
    /// is what [`want`](Self::want) records.
    pub fn get_colours(&self, key: &str) -> Option<&Colours> {
        self.entries.get(key)
    }

    /// How many lines of a file are already coloured.
    pub fn get_lines_coloured(&self, key: &str) -> u32 {
        self.entries.get(key).map_or(0, Colours::lines)
    }

    /// Marks a file as wanted now, and starts an entry if it has none.
    ///
    /// Called when a request is about to be sent. A file whose content has
    /// changed loses what was cached of it, because the old colours describe
    /// text that is gone.
    pub fn ensure_cache(&mut self, key: &str, version: Version) {
        match self.entries.get_mut(key) {
            Some(colours) if colours.version == version => {}
            Some(colours) => {
                self.cached_lines -= colours.lines.len();
                *colours = Colours {
                    lines: Vec::new(),
                    version,
                };
            }
            None => {
                self.entries.insert(
                    key.to_owned(),
                    Colours {
                        lines: Vec::new(),
                        version,
                    },
                );
            }
        }
        self.mark_used(key);
    }

    /// Installs a piece, and says whether the screen may have changed.
    ///
    /// An answer for an evicted file is dropped: it was removed while
    /// the worker was busy, and installing half of it would leave the file
    /// looking coloured when most of it is not.
    pub fn install(&mut self, response: SyntaxResponse) -> bool {
        let key = response.key.clone();
        let Some(colours) = self.entries.get_mut(&key) else {
            return false;
        };
        let before = colours.lines.len();
        if !colours.install(response) {
            return false;
        }
        self.cached_lines += colours.lines.len() - before;
        self.evict(&key);
        true
    }

    /// Drops least recently wanted files until inside the budget.
    ///
    /// `keeping` is never dropped however large it is — a single file over
    /// budget is still the file being coloured, and colouring it only to throw it
    /// away would loop.
    fn evict(&mut self, keeping: &str) {
        while self.cached_lines > BUDGET {
            let Some(position) = self.order.iter().position(|key| key != keeping) else {
                return;
            };
            let key = self.order.remove(position);
            if let Some(colours) = self.entries.remove(&key) {
                self.cached_lines -= colours.lines.len();
            }
        }
    }

    fn mark_used(&mut self, key: &str) {
        if let Some(position) = self.order.iter().position(|k| k == key) {
            self.order.remove(position);
        }
        self.order.push(key.to_owned());
    }

    /// Total lines cached across all files.
    pub fn get_cached_lines(&self) -> usize {
        self.cached_lines
    }

    /// How many file versions are cached.
    pub fn cached_count(&self) -> usize {
        self.entries.len()
    }
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
    One(&'a Colours),
    /// Both, which is what a diff has.
    Both {
        original: Option<&'a Colours>,
        modified: Option<&'a Colours>,
    },
}

impl<'a> Spans<'a> {
    /// How line `number` of one version is coloured.
    ///
    /// Numbered from 1, like [`Alignment::line`] and like the gutter, and
    /// unlike the spans underneath, which are indexed from 0. The two
    /// conventions meet here and nowhere else: written at each call site
    /// instead, it was wrong at one of them, and a whole file coloured one line
    /// out still looks coloured.
    ///
    /// [`Alignment::line`]: align::Alignment::line
    pub fn line(&self, version: DiffVersion, number: u32) -> &'a [Span] {
        let Some(index) = number.checked_sub(1) else {
            return &[];
        };
        match self {
            Spans::Off => &[],
            Spans::One(read) => read.line(index),
            Spans::Both { original, modified } => {
                let side = match version {
                    DiffVersion::Original => original,
                    DiffVersion::Modified => modified,
                };
                side.map_or(&[][..], |colours| colours.line(index))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Pen, Style};

    fn key(path: &str) -> String {
        format!("worktree:{path}")
    }

    fn span() -> Span {
        Span::new(0..1, Style::pen(Pen(0)))
    }

    fn piece(key: &str, version: Version, from: u32, lines: usize, more: bool) -> SyntaxResponse {
        SyntaxResponse {
            key: key.to_owned(),
            version,
            from,
            spans: vec![vec![span()]; lines],
            more,
        }
    }

    #[test]
    fn a_file_never_asked_for_has_nothing() {
        let store = Store::new();
        assert!(store.get_colours(&key("a.rs")).is_none());
        assert_eq!(store.get_lines_coloured(&key("a.rs")), 0);
    }

    #[test]
    fn what_is_installed_can_be_read_back() {
        let mut store = Store::new();
        let a = key("a.rs");
        store.ensure_cache(&a, Version(1));
        assert!(store.install(piece(&a, Version(1), 0, 3, false)));
        assert_eq!(store.get_lines_coloured(&a), 3);
        assert!(!store.get_colours(&a).unwrap().line(0).is_empty());
    }

    #[test]
    fn a_piece_out_of_order_is_refused() {
        // The worker sends in order. A piece that does not continue where the
        // last ended is stale, and taking it would misplace every line after.
        let mut store = Store::new();
        let a = key("a.rs");
        store.ensure_cache(&a, Version(1));
        assert!(!store.install(piece(&a, Version(1), 5, 2, false)));
        assert_eq!(store.get_lines_coloured(&a), 0);
    }

    #[test]
    fn an_answer_for_content_that_has_changed_is_thrown_away() {
        let mut store = Store::new();
        let a = key("a.rs");
        store.ensure_cache(&a, Version(2));
        assert!(!store.install(piece(&a, Version(1), 0, 3, false)));
        assert_eq!(store.get_lines_coloured(&a), 0);
    }

    #[test]
    fn asking_again_for_new_content_forgets_the_old_colours() {
        let mut store = Store::new();
        let a = key("a.rs");
        store.ensure_cache(&a, Version(1));
        store.install(piece(&a, Version(1), 0, 4, false));
        store.ensure_cache(&a, Version(2));
        assert_eq!(
            store.get_lines_coloured(&a),
            0,
            "the old colours describe text that is gone"
        );
        assert_eq!(
            store.get_cached_lines(),
            0,
            "and are not still counted against the budget"
        );
    }

    #[test]
    fn asking_again_for_the_same_content_keeps_what_arrived() {
        // The whole point: come back to a file and it is still coloured.
        let mut store = Store::new();
        let a = key("a.rs");
        store.ensure_cache(&a, Version(1));
        store.install(piece(&a, Version(1), 0, 4, false));
        store.ensure_cache(&a, Version(1));
        assert_eq!(store.get_lines_coloured(&a), 4);
    }

    #[test]
    fn an_answer_for_an_evicted_file_is_dropped() {
        let mut store = Store::new();
        let gone = key("gone.rs");
        assert!(!store.install(piece(&gone, Version(1), 0, 1, false)));
    }

    #[test]
    fn the_least_recently_wanted_file_goes_first() {
        let mut store = Store::new();
        let (a, b, c) = (key("a.rs"), key("b.rs"), key("c.rs"));
        for file in [&a, &b, &c] {
            store.ensure_cache(file, Version(1));
            store.install(piece(file, Version(1), 0, BUDGET / 2, false));
        }
        assert!(store.get_colours(&a).is_none(), "the oldest went");
        assert!(store.get_colours(&c).is_some(), "the newest stayed");
    }

    #[test]
    fn looking_at_a_file_again_saves_it_from_eviction() {
        let mut store = Store::new();
        let (a, b, c) = (key("a.rs"), key("b.rs"), key("c.rs"));
        store.ensure_cache(&a, Version(1));
        store.install(piece(&a, Version(1), 0, BUDGET / 2, false));
        store.ensure_cache(&b, Version(1));
        store.install(piece(&b, Version(1), 0, BUDGET / 4, false));

        store.ensure_cache(&a, Version(1)); // looked at again

        store.ensure_cache(&c, Version(1));
        store.install(piece(&c, Version(1), 0, BUDGET / 2, false));
        assert!(store.get_colours(&a).is_some(), "recently wanted, so kept");
        assert!(
            store.get_colours(&b).is_none(),
            "the least recent went instead"
        );
    }

    #[test]
    fn a_file_bigger_than_the_budget_is_still_kept() {
        // Otherwise the one file being coloured would be coloured and thrown away
        // over and over.
        let mut store = Store::new();
        let a = key("huge.rs");
        store.ensure_cache(&a, Version(1));
        store.install(piece(&a, Version(1), 0, BUDGET + 10, false));
        assert_eq!(store.get_lines_coloured(&a), BUDGET as u32 + 10);
    }

    #[test]
    fn a_side_with_no_colours_draws_plainly() {
        let spans = Spans::Both {
            original: None,
            modified: None,
        };
        assert!(spans.line(DiffVersion::Original, 1).is_empty());
    }

    #[test]
    fn line_numbers_are_counted_from_one() {
        let mut store = Store::new();
        let a = key("a.rs");
        store.ensure_cache(&a, Version(1));
        store.install(piece(&a, Version(1), 0, 2, false));
        let colours = store.get_colours(&a).unwrap();
        let spans = Spans::One(colours);
        assert!(
            spans.line(DiffVersion::Modified, 0).is_empty(),
            "there is no line 0"
        );
        assert!(!spans.line(DiffVersion::Modified, 1).is_empty());
    }
}
