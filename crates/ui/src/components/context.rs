//! Shared state, provided at the root.
//!
//! Components read the values with `use_context` and the two stores with
//! `use_sync_external_store`. Workers write the stores; nothing else does.

use std::cell::RefCell;
use std::rc::Rc;

use file_types::File;
use loom::{ExternalStore, Notify, Snapshot, Subscription, context};

use crate::theme::Theme;

context!(
    /// Colours and styles for every component.
    pub ThemeContext: Rc<Theme> = Rc::new(Theme::DARK),
    |a: &Rc<Theme>, b: &Rc<Theme>| Rc::ptr_eq(a, b)
);

context!(
    /// The focused file, or `None` when no file is open.
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

context!(
    /// Whether code is coloured by its language.
    pub SyntaxOnContext: bool = true
);

context!(
    /// The diff on screen and the colours for it.
    pub DiffStoreContext: DiffStore = DiffStore::new(),
    |a: &DiffStore, b: &DiffStore| Rc::ptr_eq(&a.inner, &b.inner)
);

context!(
    /// The files this review changes.
    pub FileListStoreContext: FileListStore = FileListStore::new(),
    |a: &FileListStore, b: &FileListStore| Rc::ptr_eq(&a.inner, &b.inner)
);

/// The diff on screen, and the colours for it. One file at a time: opening
/// another replaces both.
///
/// The file worker sets the diff whole; the syntax worker hands over colours
/// as they arrive.
#[derive(Clone)]
pub struct DiffStore {
    inner: Rc<RefCell<DiffStoreInner>>,
}

/// One reading of a [`DiffStore`].
pub struct DiffStoreSnapshot {
    pub diff: Option<Rc<pipeline::file::Diff>>,
    pub colours: Rc<syntax::Store>,
}

struct DiffStoreInner {
    content: Option<Rc<pipeline::file::Diff>>,
    colours: Rc<syntax::Store>,
    /// What `snapshot` hands out, rebuilt only by a write. Readers compare
    /// readings by pointer, so a fresh one per render would read as a change
    /// every render.
    reading: Rc<DiffStoreSnapshot>,
    listeners: Vec<Option<Notify>>,
}

impl DiffStore {
    pub fn new() -> Self {
        let colours = Rc::new(syntax::Store::new());
        let reading = Rc::new(DiffStoreSnapshot { diff: None, colours: Rc::clone(&colours) });
        Self {
            inner: Rc::new(RefCell::new(DiffStoreInner {
                content: None,
                colours,
                reading,
                listeners: Vec::new(),
            })),
        }
    }

    /// The diff to draw, or `None` while there is none.
    pub fn set_diff(&self, diff: Option<Rc<pipeline::file::Diff>>) {
        let listeners = {
            let mut inner = self.inner.borrow_mut();
            inner.content = diff;
            inner.refresh();
            inner.listeners.clone()
        };
        announce(listeners);
    }

    /// What the syntax worker has coloured so far.
    pub fn set_colours(&self, store: syntax::Store) {
        let listeners = {
            let mut inner = self.inner.borrow_mut();
            inner.colours = Rc::new(store);
            inner.refresh();
            inner.listeners.clone()
        };
        announce(listeners);
    }

    pub fn diff(&self) -> Option<Rc<pipeline::file::Diff>> {
        self.inner.borrow().content.clone()
    }

    /// A syntax response landed. The reading is replaced so every subscriber
    /// sees the new colours.
    pub fn notify_colours_changed(&self) {
        let listeners = {
            let mut inner = self.inner.borrow_mut();
            inner.refresh();
            inner.listeners.clone()
        };
        announce(listeners);
    }

    pub fn colours(&self) -> Rc<syntax::Store> {
        Rc::clone(&self.inner.borrow().colours)
    }
}

impl Default for DiffStore {
    fn default() -> Self {
        Self::new()
    }
}

impl DiffStoreInner {
    /// Builds the reading readers compare by pointer. Every write ends here.
    fn refresh(&mut self) {
        self.reading = Rc::new(DiffStoreSnapshot {
            diff: self.content.clone(),
            colours: Rc::clone(&self.colours),
        });
    }
}

impl ExternalStore for DiffStore {
    type Value = DiffStoreSnapshot;

    fn subscribe(&self, notify: Notify) -> Subscription {
        let slot = listen(&mut self.inner.borrow_mut().listeners, notify);
        let inner = Rc::clone(&self.inner);
        Subscription::new(move || inner.borrow_mut().listeners[slot] = None)
    }

    fn snapshot(&self) -> Snapshot<Self::Value> {
        Snapshot::from(Rc::clone(&self.inner.borrow().reading))
    }
}

/// The files this review changes, in the order the explorer shows them.
#[derive(Clone)]
pub struct FileListStore {
    inner: Rc<RefCell<FileListStoreInner>>,
}

#[derive(Default)]
struct FileListStoreInner {
    /// An `Rc` because it is the reading itself: a new one is a new reading.
    files: Rc<Vec<File>>,
    listeners: Vec<Option<Notify>>,
}

impl FileListStore {
    pub fn new() -> Self {
        Self { inner: Rc::new(RefCell::new(FileListStoreInner::default())) }
    }

    /// Replaces the list with what the list worker read.
    pub fn fill(&self, files: Vec<File>) {
        let listeners = {
            let mut inner = self.inner.borrow_mut();
            inner.files = Rc::new(files);
            inner.listeners.clone()
        };
        announce(listeners);
    }
}

impl Default for FileListStore {
    fn default() -> Self {
        Self::new()
    }
}

impl ExternalStore for FileListStore {
    type Value = Vec<File>;

    fn subscribe(&self, notify: Notify) -> Subscription {
        let slot = listen(&mut self.inner.borrow_mut().listeners, notify);
        let inner = Rc::clone(&self.inner);
        Subscription::new(move || inner.borrow_mut().listeners[slot] = None)
    }

    fn snapshot(&self) -> Snapshot<Self::Value> {
        Snapshot::from(Rc::clone(&self.inner.borrow().files))
    }
}

/// Takes a reader's place in the list, reusing a slot an earlier reader left.
/// The index it returns is the one that reader's `Subscription` empties, which
/// is why slots are emptied rather than removed: removing would shift the
/// indices the other readers hold.
fn listen(listeners: &mut Vec<Option<Notify>>, notify: Notify) -> usize {
    match listeners.iter().position(Option::is_none) {
        Some(free) => {
            listeners[free] = Some(notify);
            free
        }
        None => {
            listeners.push(Some(notify));
            listeners.len() - 1
        }
    }
}

/// Tells every reader that the store moved. Takes the list by value, so the
/// caller drops its borrow of the store before a reader can look at it.
fn announce(listeners: Vec<Option<Notify>>) {
    for notify in listeners.into_iter().flatten() {
        notify.changed();
    }
}

/// A shared cell the root writes the cursor into each render, so the
/// session can read it from outside the tree for tests.
context!(
    pub CursorCellContext: Rc<std::cell::Cell<u32>> = Rc::new(std::cell::Cell::new(0)),
    |a: &Rc<std::cell::Cell<u32>>, b: &Rc<std::cell::Cell<u32>>| Rc::ptr_eq(a, b)
);

/// A shared cell the root writes the view_lines count into.
context!(
    pub ViewLinesCellContext: Rc<std::cell::Cell<u32>> = Rc::new(std::cell::Cell::new(0)),
    |a: &Rc<std::cell::Cell<u32>>, b: &Rc<std::cell::Cell<u32>>| Rc::ptr_eq(a, b)
);
