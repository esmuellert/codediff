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
    /// The repository path.
    pub RepoContext: Option<Rc<std::path::Path>> = None
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
    /// How the reader has arranged the file list: which rows are folded, and
    /// whether it is nested or flat.
    ///
    /// The rows themselves are not here. They follow from this and the file
    /// list store, so whoever needs them works them out from the two.
    pub ArrangementContext: crate::components::explorer::model::Arrangement =
        crate::components::explorer::model::Arrangement::default()
);

context!(
    /// Which pane a subtree is drawn in, for whatever records where it landed.
    pub PaneContext: Option<crate::screen_map::PaneId> = None
);

context!(
    /// The live text selection. Held above the diff screens because a new
    /// file and a new layout both end it, and neither is theirs to know.
    /// A screen reads it from here and changes it through its `on_select`
    /// prop, the way a controlled component is written.
    pub SelectionContext: Option<crate::components::selection::Selection> = None
);

context!(
    /// How the interface asks for a file to be compared. A component cannot
    /// reach a worker, so it says which file and the session sends it.
    pub OpenContext: Rc<dyn Fn(File)> = Rc::new(|_| {}),
    |a: &Rc<dyn Fn(File)>, b: &Rc<dyn Fn(File)>| Rc::ptr_eq(a, b)
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
    /// Which layout the diff is drawn in.
    ///
    /// A view line is a different row side by side than it is inline, so
    /// whoever counts rows has to know which numbering the cursor is in.
    pub LayoutContext: file_types::DiffType = file_types::DiffType::SideBySide
);

context!(
    /// Which way a change key went with nowhere to go, cleared by the next
    /// key. Only the status line has anything to say about it.
    pub ExhaustedContext: Option<crate::components::Direction> = None
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
    pub content: Option<Rc<pipeline::file::DiffContent>>,
    /// Which content this is a reading of. Bumped by every new comparison and
    /// not by new colours, so a re-read of the same file — which has the same
    /// name and the same revisions — is still a different thing to show.
    pub version: syntax::Version,
    /// Shared and mutable, because the worker fills it while the screen reads
    /// it. A reading cannot own a copy: the spans of a large file are the
    /// biggest thing the program holds, and a frame would copy them all.
    pub colours: Rc<RefCell<syntax::Store>>,
}

struct DiffStoreInner {
    content: Option<Rc<pipeline::file::DiffContent>>,
    colours: Rc<RefCell<syntax::Store>>,
    /// Which content the colours describe. Bumped by every new diff, so an
    /// answer for the file that was open before is refused rather than mixed
    /// into the one that replaced it.
    version: syntax::Version,
    /// What `snapshot` hands out, rebuilt only by a write. Readers compare
    /// readings by pointer, so a fresh one per render would read as a change
    /// every render.
    reading: Rc<DiffStoreSnapshot>,
    listeners: Vec<Option<Notify>>,
}

impl DiffStore {
    pub fn new() -> Self {
        let colours = Rc::new(RefCell::new(syntax::Store::new()));
        let reading = Rc::new(DiffStoreSnapshot {
            content: None,
            version: syntax::Version(0),
            colours: Rc::clone(&colours),
        });
        Self {
            inner: Rc::new(RefCell::new(DiffStoreInner {
                content: None,
                colours,
                version: syntax::Version(0),
                reading,
                listeners: Vec::new(),
            })),
        }
    }

    /// The diff to draw, or `None` while there is none.
    pub fn set_content(&self, content: Option<Rc<pipeline::file::DiffContent>>) {
        let listeners = {
            let mut inner = self.inner.borrow_mut();
            inner.content = content;
            inner.version = syntax::Version(inner.version.0 + 1);
            inner.refresh();
            inner.listeners.clone()
        };
        announce(listeners);
    }

    /// Which content the colours are for, for whoever asks the worker.
    pub fn version(&self) -> syntax::Version {
        self.inner.borrow().version
    }

    /// Takes a piece of colouring from the worker, and says whether it was
    /// taken — a piece for content that has moved on is refused.
    pub fn install_colours(&self, response: syntax::SyntaxResponse) -> bool {
        self.colours().borrow_mut().install(response)
    }

    pub fn content(&self) -> Option<Rc<pipeline::file::DiffContent>> {
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

    pub fn colours(&self) -> Rc<RefCell<syntax::Store>> {
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
            content: self.content.clone(),
            version: self.version,
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

context!(
    /// A shared cell the root writes the focused pane's cursor into each
    /// render, so the session can read it from outside the tree.
    pub CursorCellContext: Rc<std::cell::Cell<u32>> = Rc::new(std::cell::Cell::new(0)),
    |a: &Rc<std::cell::Cell<u32>>, b: &Rc<std::cell::Cell<u32>>| Rc::ptr_eq(a, b)
);

context!(
    /// The same, for how long the focused pane's document is.
    pub ViewLinesCellContext: Rc<std::cell::Cell<u32>> = Rc::new(std::cell::Cell::new(0)),
    |a: &Rc<std::cell::Cell<u32>>, b: &Rc<std::cell::Cell<u32>>| Rc::ptr_eq(a, b)
);

context!(
    /// A shared cell the root writes the layout into. The state slot is not
    /// the answer on its own — a one-sided file overrides it — so what was
    /// drawn is what goes here.
    pub LayoutCellContext: Rc<std::cell::Cell<file_types::DiffType>> =
        Rc::new(std::cell::Cell::new(file_types::DiffType::SideBySide)),
    |a: &Rc<std::cell::Cell<file_types::DiffType>>,
     b: &Rc<std::cell::Cell<file_types::DiffType>>| Rc::ptr_eq(a, b)
);

context!(
    /// The active text selection, written each render by whoever holds it.
    /// Tests read it through Session.
    pub SelectionCellContext: Rc<std::cell::RefCell<Option<crate::components::selection::Selection>>> =
        Rc::new(std::cell::RefCell::new(None)),
    |a: &Rc<std::cell::RefCell<Option<crate::components::selection::Selection>>>,
     b: &Rc<std::cell::RefCell<Option<crate::components::selection::Selection>>>| Rc::ptr_eq(a, b)
);

context!(
    /// Where things landed on screen. Filled by layout effects, read by
    /// whoever has to say what is under the mouse.
    pub ScreenMapCellContext: Rc<std::cell::RefCell<crate::screen_map::ScreenMap>> =
        Rc::new(std::cell::RefCell::new(crate::screen_map::ScreenMap::default())),
    |a: &Rc<std::cell::RefCell<crate::screen_map::ScreenMap>>,
     b: &Rc<std::cell::RefCell<crate::screen_map::ScreenMap>>| Rc::ptr_eq(a, b)
);
