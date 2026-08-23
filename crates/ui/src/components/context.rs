//! Shared state, provided at the root.
//!
//! Screens and the status line read these with `use_context`. Bricks read
//! only `theme` and take per-row values as props.

use std::rc::Rc;

use file_types::File;
use loom::context;

use crate::theme::Theme;
use crate::view::selection::Selection;

/// Colours and styles for every component.
context!(pub ThemeContext: Rc<Theme> = Rc::new(Theme::DARK), |a: &Rc<Theme>, b: &Rc<Theme>| Rc::ptr_eq(a, b));

/// The repository path.
context!(pub RepoContext: Option<Rc<std::path::Path>> = None);

/// The focused file, or `None` in the explorer.
context!(pub FileContext: Option<Rc<File>> = None);

/// Which rows to render.
context!(pub ViewLinesContext: std::ops::Range<u32> = 0..0);

/// Which row the cursor is on.
context!(pub CursorContext: u32 = 0);

/// Horizontal scroll offset in cells.
context!(pub FirstCellContext: u32 = 0);

/// An error or warning to display.
context!(pub NoticeContext: Option<Rc<str>> = None);

/// What the diff and syntax workers have filled in for the open file.
///
/// A store rather than a context value: a worker finishing redraws the
/// component that subscribed, and nothing else.
pub struct Diffs {
    inner: Rc<std::cell::RefCell<DiffsInner>>,
}

struct DiffsInner {
    /// Bumped whenever a worker fills something in, so a reader that compares
    /// snapshots sees a new one.
    reading: Rc<Reading>,
    listeners: Vec<loom::Notify>,
}

/// One reading of what the workers have produced.
pub struct Reading {
    pub diff: Option<Rc<pipeline::file::Diff>>,
    pub colours: Rc<syntax::Store>,
    pub syntax_on: bool,
}

impl Default for Diffs {
    fn default() -> Self {
        Self::new()
    }
}

impl Diffs {
    pub fn new() -> Self {
        Self {
            inner: Rc::new(std::cell::RefCell::new(DiffsInner {
                reading: Rc::new(Reading {
                    diff: None,
                    colours: Rc::new(syntax::Store::new()),
                    syntax_on: true,
                }),
                listeners: Vec::new(),
            })),
        }
    }

    /// Replaces what the workers have produced, and tells every reader.
    pub fn fill(&self, reading: Reading) {
        let listeners = {
            let mut inner = self.inner.borrow_mut();
            inner.reading = Rc::new(reading);
            inner.listeners.clone()
        };
        for listener in listeners {
            listener.changed();
        }
    }

    pub fn reading(&self) -> Rc<Reading> {
        Rc::clone(&self.inner.borrow().reading)
    }
}

impl Clone for Diffs {
    fn clone(&self) -> Self {
        Self { inner: Rc::clone(&self.inner) }
    }
}

impl loom::ExternalStore for Diffs {
    type Value = Reading;

    fn subscribe(&self, notify: loom::Notify) -> loom::Subscription {
        self.inner.borrow_mut().listeners.push(notify);
        let inner = Rc::clone(&self.inner);
        loom::Subscription::new(move || {
            // The runtime drops this when the reader unmounts.
            inner.borrow_mut().listeners.clear();
        })
    }

    fn snapshot(&self) -> loom::Snapshot<Self::Value> {
        loom::Snapshot::from(self.reading())
    }
}

/// The store itself reaches components through context, because it is one
/// object for the life of the session.
context!(pub DiffsContext: Diffs = Diffs::new(), |_a: &Diffs, _b: &Diffs| true);

/// Where the mouse has selected text, when it has.
///
/// Held by whichever screen owns the pointer, so nothing here offers it.
pub type MaybeSelection = Option<Selection>;
