//! Shared state, provided at the root.

use std::cell::{Cell, RefCell};
use std::ops::Range;
use std::path::Path;
use std::rc::Rc;

use file_types::{DiffType, File};
use loom::context;

use crate::app::Flow;
use crate::components::Direction;
use crate::components::selection::Selection;
use crate::theme::Theme;

/// Everything a screen reads.
#[derive(Clone)]
pub struct Context {
    pub theme: Rc<Theme>,
    pub repo: Option<Rc<Path>>,
    pub file: Option<Rc<File>>,
    pub view_lines: Range<u32>,
    pub cursor: u32,
    pub first_cell: u32,
    pub selection: Option<Selection>,
    pub notice: Option<Rc<str>>,
    pub diff_view_type: DiffType,
    /// The diff on screen, or `None` while there is none.
    pub diff: Option<Rc<pipeline::file::DiffContent>>,
    /// Bumped by every new diff, so a stale colour response is refused.
    pub diff_version: syntax::Version,
    /// Shared with Session: the worker fills it, screens read it.
    pub colours: Rc<RefCell<syntax::Store>>,
    /// The files this review changes.
    pub files: Rc<Vec<File>>,
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
            diff: None,
            diff_version: syntax::Version(0),
            colours: Rc::new(RefCell::new(syntax::Store::new())),
            files: Rc::new(Vec::new()),
        }
    }
}

impl Context {
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
            && same_rc(&self.diff, &other.diff)
            && self.diff_version == other.diff_version
            && Rc::ptr_eq(&self.colours, &other.colours)
            && Rc::ptr_eq(&self.files, &other.files)
    }
}

fn same_rc<T: ?Sized>(a: &Option<Rc<T>>, b: &Option<Rc<T>>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) => Rc::ptr_eq(a, b),
        _ => false,
    }
}

#[derive(Default)]
pub struct Observed {
    pub cursor: Cell<u32>,
    pub view_lines: Cell<u32>,
    pub layout: Cell<DiffType>,
    pub selection: RefCell<Option<Selection>>,
    pub exhausted: Cell<Option<Direction>>,
    pub on_flow: Option<Rc<dyn Fn(Flow)>>,
    pub on_open: Option<Rc<dyn Fn(File)>>,
    pub set_selection: RefCell<Option<Box<dyn Fn(Option<Selection>)>>>,
    pub set_list_cursor: RefCell<Option<Box<dyn Fn(u32, u32)>>>,
}

impl Observed {
    pub fn select(&self, held: Option<Selection>) {
        if let Some(set) = self.set_selection.borrow().as_ref() {
            set(held);
        }
    }

    pub fn place_in_list(&self, rows: u32, line: u32) {
        if let Some(set) = self.set_list_cursor.borrow().as_ref() {
            set(rows, line);
        }
    }
}

context!(
    pub Ui: Context = Context::default(),
    |a: &Context, b: &Context| a.same(b)
);

context!(
    pub ObservedCtx: Rc<Observed> = Rc::new(Observed::default()),
    |a: &Rc<Observed>, b: &Rc<Observed>| Rc::ptr_eq(a, b)
);
