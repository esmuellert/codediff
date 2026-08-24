//! Shared state, provided at the root.
//!
//! Screens and the status line read these with `use_context`. Bricks read
//! only `theme` and take per-row values as props.

use std::rc::Rc;

use file_types::File;
use loom::context;

use crate::theme::Theme;

context!(
    /// Colours and styles for every component.
    pub ThemeContext: Rc<Theme> = Rc::new(Theme::DARK),
    |a: &Rc<Theme>, b: &Rc<Theme>| Rc::ptr_eq(a, b)
);

context!(
    /// The repository path.
    pub RepoContext: Option<Rc<std::path::Path>> = None
);

context!(
    /// The focused file, or `None` in the explorer.
    pub FileContext: Option<Rc<File>> = None
);

context!(
    /// Which rows to render.
    pub ViewLinesContext: std::ops::Range<u32> = 0..0
);

context!(
    /// Which row the cursor is on.
    pub CursorContext: u32 = 0
);

context!(
    /// Horizontal scroll offset in cells.
    pub FirstCellContext: u32 = 0
);

context!(
    /// An error or warning to display.
    pub NoticeContext: Option<Rc<str>> = None
);

/// What the diff and syntax workers have filled in for the open file.
///
/// A store rather than a context value: a worker finishing redraws the
/// component that subscribed, and nothing else.
pub struct DiffData {
    inner: Rc<std::cell::RefCell<DiffDataInner>>,
}

struct DiffDataInner {
    reading: Rc<Loaded>,
    listeners: Vec<loom::Notify>,
}

/// One reading of what the workers have produced.
pub struct Loaded {
    pub diff: Option<Rc<pipeline::file::Diff>>,
    pub colours: Rc<syntax::Store>,
    pub syntax_on: bool,
}

impl Default for DiffData {
    fn default() -> Self {
        Self::new()
    }
}

impl DiffData {
    pub fn new() -> Self {
        Self {
            inner: Rc::new(std::cell::RefCell::new(DiffDataInner {
                reading: Rc::new(Loaded {
                    diff: None,
                    colours: Rc::new(syntax::Store::new()),
                    syntax_on: true,
                }),
                listeners: Vec::new(),
            })),
        }
    }

    /// Replaces what the workers have produced, and tells every reader.
    ///
    /// A new `Rc` is a new reading, which is what a subscriber compares.
    pub fn fill(&self, reading: Loaded) {
        let listeners = {
            let mut inner = self.inner.borrow_mut();
            inner.reading = Rc::new(reading);
            inner.listeners.clone()
        };
        for listener in listeners {
            listener.changed();
        }
    }

    pub fn reading(&self) -> Rc<Loaded> {
        Rc::clone(&self.inner.borrow().reading)
    }
}

impl Clone for DiffData {
    fn clone(&self) -> Self {
        Self { inner: Rc::clone(&self.inner) }
    }
}

impl loom::ExternalStore for DiffData {
    type Value = Loaded;

    fn subscribe(&self, notify: loom::Notify) -> loom::Subscription {
        self.inner.borrow_mut().listeners.push(notify);
        let inner = Rc::clone(&self.inner);
        loom::Subscription::new(move || inner.borrow_mut().listeners.clear())
    }

    fn snapshot(&self) -> loom::Snapshot<Self::Value> {
        loom::Snapshot::from(self.reading())
    }
}

context!(
    /// The store itself, because it is one object for the life of the session.
    pub DiffDataContext: DiffData = DiffData::new(),
    |_a: &DiffData, _b: &DiffData| true
);
