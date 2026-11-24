//! Shared state provided at the root by `App`.

use std::path::Path;
use std::rc::Rc;

use file_types::{File, RepoPath, Revs};
use loom::{SetState, context};

use super::revisions::RevisionSide;
use crate::RevisionRange;
use crate::keybindings::KeyMap;
use crate::services::{DiffService, SyntaxService, VcsService, WatcherService};
use crate::theme::Theme;

/// A selected directory and the changed files beneath it.
#[derive(Clone)]
pub struct DirectorySelection {
    pub path: RepoPath,
    pub revs: Revs,
    pub files: Rc<Vec<File>>,
}

/// Equal when the path, revisions and files are equal. Every listing builds a
/// new selection, and one that lists the same files is the same selection.
impl PartialEq for DirectorySelection {
    fn eq(&self, other: &Self) -> bool {
        self.path == other.path && self.revs == other.revs && self.files == other.files
    }
}

pub(crate) fn directory_key(path: &RepoPath, revs: &Revs) -> String {
    format!("{}/{}", revs.heading(), path.as_str())
}

/// Everything a component reads.
#[derive(Clone)]
pub struct Context {
    pub config: Rc<config::Config>,
    pub keybindings: Rc<KeyMap>,
    pub theme: Rc<Theme>,
    pub branch_drawing: bool,
    pub repo: Rc<Path>,
    pub revision_range: RevisionRange,
    pub set_revision_range: Option<SetState<RevisionRange>>,
    pub picking: Option<RevisionSide>,
    pub set_picking: Option<SetState<Option<RevisionSide>>>,
    pub file: Option<Rc<File>>,
    pub set_file: Option<SetState<Option<Rc<File>>>>,
    pub directory: Option<Rc<DirectorySelection>>,
    pub set_directory: Option<SetState<Option<Rc<DirectorySelection>>>>,
    pub vcs: Option<Rc<VcsService>>,
    pub diff: Option<Rc<DiffService>>,
    pub syntax: Option<Rc<SyntaxService>>,
    pub watcher: Option<Rc<WatcherService>>,
}

impl Default for Context {
    fn default() -> Self {
        Self {
            config: Rc::new(config::Config::default()),
            keybindings: Rc::new(KeyMap::default()),
            theme: Rc::new(Theme::DARK),
            branch_drawing: false,
            repo: Rc::from(Path::new("")),
            revision_range: RevisionRange::Worktree,
            set_revision_range: None,
            picking: None,
            set_picking: None,
            file: None,
            set_file: None,
            directory: None,
            set_directory: None,
            vcs: None,
            diff: None,
            syntax: None,
            watcher: None,
        }
    }
}

impl Context {
    fn same(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.config, &other.config)
            && Rc::ptr_eq(&self.keybindings, &other.keybindings)
            && Rc::ptr_eq(&self.theme, &other.theme)
            && self.branch_drawing == other.branch_drawing
            && Rc::ptr_eq(&self.repo, &other.repo)
            && self.revision_range == other.revision_range
            && self.set_revision_range == other.set_revision_range
            && self.picking == other.picking
            && self.set_picking == other.set_picking
            && same_rc(&self.file, &other.file)
            && self.set_file == other.set_file
            && same_rc(&self.directory, &other.directory)
            && self.set_directory == other.set_directory
            && same_rc(&self.vcs, &other.vcs)
            && same_rc(&self.diff, &other.diff)
            && same_rc(&self.syntax, &other.syntax)
            && same_rc(&self.watcher, &other.watcher)
    }
}

fn same_rc<T: ?Sized>(a: &Option<Rc<T>>, b: &Option<Rc<T>>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) => Rc::ptr_eq(a, b),
        _ => false,
    }
}

context!(
    pub Ui: Context = Context::default(),
    |a: &Context, b: &Context| a.same(b)
);
