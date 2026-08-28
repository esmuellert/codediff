//! Shared state, provided once at the root.
//!
//! The provider owns all UI state. Children read values and call setters
//! through context. Session data (diff, files, colours) arrives as props
//! on the provider and is placed into context alongside the UI state.

use std::cell::{Cell, RefCell};
use std::ops::Range;
use std::path::Path;
use std::rc::Rc;

use file_types::{DiffType, File};
use loom::{SetState, context};

use crate::components::selection::Selection;
use crate::components::viewport::Viewport;
use crate::components::Direction;
use crate::theme::Theme;

/// Everything a component reads or writes.
#[derive(Clone)]
pub struct Context {
    // ---- session data (read-only, pushed from outside the tree) ----

    pub theme: Rc<Theme>,
    pub repo: Option<Rc<Path>>,
    pub diff: Option<Rc<pipeline::file::DiffContent>>,
    pub diff_version: syntax::Version,
    pub colours: Rc<RefCell<syntax::Store>>,
    pub files: Rc<Vec<File>>,

    // ---- UI state (owned by the provider's use_state slots) ----

    pub file: Option<Rc<File>>,
    pub diff_view_type: DiffType,
    pub notice: Option<Rc<str>>,
    pub selection: Option<Selection>,
    pub focus_diff: bool,
    pub exhausted: Option<Direction>,

    // ---- per-document positions ----

    pub list_cursor: u32,
    pub list_view_lines: Range<u32>,
    pub diff_cursor: u32,
    pub diff_view_lines: Range<u32>,
    pub first_cell: u32,

    // ---- setters children call ----

    pub set_selection: Option<SetState<Option<Selection>>>,
    pub set_list_rows: Option<SetState<u32>>,
    pub set_list_viewport: Option<SetState<Viewport>>,

    // ---- callbacks to Session ----

    pub on_open: Option<Rc<dyn Fn(File)>>,
    pub on_flow: Option<Rc<dyn Fn(crate::app::Flow)>>,

    /// Session reads these after a frame.
    pub read_back: Option<Rc<ReadBack>>,
}

impl Context {
    fn empty() -> Self {
        Self {
            theme: Rc::new(Theme::DARK),
            repo: None,
            diff: None,
            diff_version: syntax::Version(0),
            colours: Rc::new(RefCell::new(syntax::Store::new())),
            files: Rc::new(Vec::new()),
            file: None,
            diff_view_type: DiffType::SideBySide,
            notice: None,
            selection: None,
            focus_diff: false,
            exhausted: None,
            list_cursor: 0,
            list_view_lines: 0..0,
            diff_cursor: 0,
            diff_view_lines: 0..0,
            first_cell: 0,
            set_selection: None,
            set_list_rows: None,
            set_list_viewport: None,
            on_open: None,
            on_flow: None,
            read_back: None,
        }
    }

    fn same(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.theme, &other.theme)
            && same_rc(&self.repo, &other.repo)
            && same_rc(&self.diff, &other.diff)
            && self.diff_version == other.diff_version
            && Rc::ptr_eq(&self.colours, &other.colours)
            && Rc::ptr_eq(&self.files, &other.files)
            && same_rc_file(&self.file, &other.file)
            && self.diff_view_type == other.diff_view_type
            && same_rc(&self.notice, &other.notice)
            && self.selection == other.selection
            && self.focus_diff == other.focus_diff
            && self.exhausted == other.exhausted
            && self.list_cursor == other.list_cursor
            && self.list_view_lines == other.list_view_lines
            && self.diff_cursor == other.diff_cursor
            && self.diff_view_lines == other.diff_view_lines
            && self.first_cell == other.first_cell
            && self.set_selection == other.set_selection
            && self.set_list_rows == other.set_list_rows
            && self.set_list_viewport == other.set_list_viewport
    }
}

fn same_rc<T: ?Sized>(a: &Option<Rc<T>>, b: &Option<Rc<T>>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) => Rc::ptr_eq(a, b),
        _ => false,
    }
}

fn same_rc_file(a: &Option<Rc<File>>, b: &Option<Rc<File>>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) => Rc::ptr_eq(a, b),
        _ => false,
    }
}

/// Values App writes during render for Session to read after.
pub struct ReadBack {
    pub cursor: Cell<u32>,
    pub view_lines: Cell<u32>,
    pub layout: Cell<DiffType>,
    pub selection: RefCell<Option<Selection>>,
}

impl Default for ReadBack {
    fn default() -> Self {
        Self {
            cursor: Cell::new(0),
            view_lines: Cell::new(0),
            layout: Cell::new(DiffType::SideBySide),
            selection: RefCell::new(None),
        }
    }
}

context!(
    pub Ui: Context = Context::empty(),
    |a: &Context, b: &Context| a.same(b)
);
