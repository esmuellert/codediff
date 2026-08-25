//! Shared state, provided at the root.
//!
//! One context holds everything a screen reads; the two stores are read with
//! `use_sync_external_store`. Workers write the stores; nothing else does.

use std::cell::{Cell, RefCell};
use std::ops::Range;
use std::path::Path;
use std::rc::Rc;

use file_types::{DiffType, File};
use loom::{ExternalStore, Notify, Snapshot, Subscription, context};

use crate::app::Flow;
use crate::components::Direction;
use crate::components::selection::Selection;
use crate::theme::Theme;

/// Everything a screen reads.
///
/// One struct rather than one context per value: they are read together, by
/// components that need most of them, and a reading of half of them is not a
/// frame. `App` builds it, and each pane provides it again with its own
/// window onto its own document.
#[derive(Clone)]
pub struct Context {
    /// Colours and styles for every component.
    pub theme: Rc<Theme>,
    /// The repository path.
    pub repo: Option<Rc<Path>>,
    /// The focused file, or `None` when the reader is in the list.
    pub file: Option<Rc<File>>,
    /// Which rows to render.
    pub view_lines: Range<u32>,
    /// Which row the cursor is on.
    pub cursor: u32,
    /// Horizontal scroll offset in cells.
    pub first_cell: u32,
    /// The live text selection. A screen reads it here and changes it through
    /// the setter `App` leaves in [`Observed`].
    pub selection: Option<Selection>,
    /// An error or warning to display.
    pub notice: Option<Rc<str>>,
    /// Which way the open file is laid out. What is on screen, not what the
    /// toggle says: a one-sided file overrides it.
    pub diff_view_type: DiffType,
}

impl Default for Context {
    fn default() -> Self {
        Self {
            theme: Rc::new(Theme::DARK),
            repo: None,
            file: None,
            view_lines: 0..0,
            cursor: 0,
            first_cell: 0,
            selection: None,
            notice: None,
            diff_view_type: DiffType::SideBySide,
        }
    }
}

impl Context {
    /// Whether an offer says the same as the last, so readers can stay put.
    ///
    /// What is shared is compared by pointer: a theme is the same theme when
    /// it is the same allocation, and answering otherwise means a walk over
    /// every colour in it.
    fn same(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.theme, &other.theme)
            && same_rc(&self.repo, &other.repo)
            && same_rc(&self.file, &other.file)
            && self.view_lines == other.view_lines
            && self.cursor == other.cursor
            && self.first_cell == other.first_cell
            && self.selection == other.selection
            && same_rc(&self.notice, &other.notice)
            && self.diff_view_type == other.diff_view_type
    }
}

fn same_rc<T: ?Sized>(a: &Option<Rc<T>>, b: &Option<Rc<T>>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) => Rc::ptr_eq(a, b),
        _ => false,
    }
}

/// What the frame decided, for a caller outside the tree — and the few
/// callbacks that cannot be worked out inside it.
///
/// The session cannot ask a component anything — a key only marks state
/// dirty, and the frame after it is what settles the answer — so `App`
/// writes down what it drew and the session reads it here. The read-ahead
/// needs the cursor; the rest is what a test asks about.
///
/// It runs the other way too. A screen takes no props, so a screen that has
/// something to say says it through a setter `App` leaves here, and the
/// session leaves what only it can do — stop the loop, reach a worker — the
/// same way.
#[derive(Default)]
pub struct Observed {
    /// The focused pane's cursor.
    pub cursor: Cell<u32>,
    /// How long the focused pane's document is.
    pub view_lines: Cell<u32>,
    /// The layout that was drawn, which the state slot does not answer on its
    /// own — a one-sided file overrides it.
    pub layout: Cell<DiffType>,
    pub selection: RefCell<Option<Selection>>,
    /// Which way `]c` or `[c` went with nowhere to go, for the status line to
    /// report.
    pub exhausted: Cell<Option<Direction>>,
    /// What the reader asked of the loop. Only the session can stop it.
    pub on_flow: Option<Rc<dyn Fn(Flow)>>,
    /// The file the reader chose. Only the session can reach a worker.
    pub on_open: Option<Rc<dyn Fn(File)>>,
    /// Where a diff screen puts what the pointer selected. Left here by `App`
    /// each render, because the value goes down as context and the screens
    /// that change it take no props.
    pub set_selection: RefCell<Option<Box<dyn Fn(Option<Selection>)>>>,
    /// How long the list turned out to be, and which row the cursor is on.
    /// Only the explorer knows the first; only the pane holds the second.
    pub set_list_cursor: RefCell<Option<Box<dyn Fn(u32, u32)>>>,
}

impl Observed {
    /// Hands a selection to whoever owns it.
    ///
    /// The borrow is held across the call because nothing renders inside a
    /// listener: a write marks state dirty, and the frame after it is what
    /// replaces the setter.
    pub fn select(&self, held: Option<Selection>) {
        if let Some(set) = self.set_selection.borrow().as_ref() {
            set(held);
        }
    }

    /// Says how long the list is and where in it the reader now is.
    pub fn place_in_list(&self, rows: u32, line: u32) {
        if let Some(set) = self.set_list_cursor.borrow().as_ref() {
            set(rows, line);
        }
    }
}

context!(
    /// Everything a screen reads.
    pub Ui: Context = Context::default(),
    |a: &Context, b: &Context| a.same(b)
);

context!(
    /// The diff on screen and the colours for it.
    pub DiffStoreCtx: DiffStore = DiffStore::new(),
    |a: &DiffStore, b: &DiffStore| Rc::ptr_eq(&a.inner, &b.inner)
);

context!(
    /// The files this review changes.
    pub FileListStoreCtx: FileListStore = FileListStore::new(),
    |a: &FileListStore, b: &FileListStore| Rc::ptr_eq(&a.inner, &b.inner)
);

context!(
    /// Where the frame writes what it decided, for the session to read.
    pub ObservedCtx: Rc<Observed> = Rc::new(Observed::default()),
    |a: &Rc<Observed>, b: &Rc<Observed>| Rc::ptr_eq(a, b)
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
    /// Identifies which comparison produced these colours. Bumped by every
    /// new comparison, so a re-read of the same file is a fresh reading.
    pub version: syntax::Version,
    /// Shared: the worker fills it while the screen reads it.
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
