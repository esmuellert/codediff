//! Shared state, and the one provider that owns it.

use std::path::Path;
use std::rc::Rc;

use diff_service::DiffService;
use file_types::{File, RepoPath, Revs};
use loom::{Node, Scope, SetState, component, context, rsx, use_memo, use_state};

use crate::keybindings::KeyMap;
use crate::services::version_control::VersionControlService;
use crate::services::watcher::WatcherService;
use crate::theme::Theme;

/// A selected directory and the changed files beneath it.
#[derive(Clone)]
pub struct DirectorySelection {
    pub path: RepoPath,
    pub revs: Revs,
    pub files: Rc<Vec<File>>,
}

impl PartialEq for DirectorySelection {
    fn eq(&self, other: &Self) -> bool {
        self.path == other.path && self.revs == other.revs && Rc::ptr_eq(&self.files, &other.files)
    }
}

/// Everything a component reads.
#[derive(Clone)]
pub struct Context {
    pub config: Rc<config::Config>,
    pub keybindings: Rc<KeyMap>,
    pub theme: Rc<Theme>,
    pub repo: Rc<Path>,
    pub file: Option<Rc<File>>,
    pub set_file: Option<SetState<Option<Rc<File>>>>,
    pub directory: Option<Rc<DirectorySelection>>,
    pub set_directory: Option<SetState<Option<Rc<DirectorySelection>>>>,
    pub diff_service: Option<Rc<DiffService>>,
    pub version_control_service: Option<Rc<VersionControlService>>,
    pub watcher_service: Option<Rc<WatcherService>>,
    pub set_repo: Option<SetState<Rc<Path>>>,
}

impl Default for Context {
    fn default() -> Self {
        Self {
            config: Rc::new(config::Config::default()),
            keybindings: Rc::new(KeyMap::default()),
            theme: Rc::new(Theme::DARK),
            repo: Rc::from(Path::new("")),
            file: None,
            set_file: None,
            directory: None,
            set_directory: None,
            diff_service: None,
            version_control_service: None,
            watcher_service: None,
            set_repo: None,
        }
    }
}

impl Context {
    fn same(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.config, &other.config)
            && Rc::ptr_eq(&self.keybindings, &other.keybindings)
            && Rc::ptr_eq(&self.theme, &other.theme)
            && Rc::ptr_eq(&self.repo, &other.repo)
            && same_rc(&self.file, &other.file)
            && self.set_file == other.set_file
            && same_rc(&self.directory, &other.directory)
            && self.set_directory == other.set_directory
            && same_rc(&self.diff_service, &other.diff_service)
            && same_rc(
                &self.version_control_service,
                &other.version_control_service,
            )
            && same_rc(&self.watcher_service, &other.watcher_service)
            && self.set_repo == other.set_repo
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

/// Provides shared state and service handles.
#[component]
pub fn UiProvider(
    scope: &mut Scope,
    cwd: Rc<Path>,
    config: Rc<config::Config>,
    keybindings: Rc<KeyMap>,
    diff_service: Rc<DiffService>,
    version_control_service: Rc<VersionControlService>,
    watcher_service: Rc<WatcherService>,
    children: loom::Children,
) -> Node {
    let initial = Rc::clone(cwd);
    let (repo, set_repo) = use_state(scope, || initial);
    let (file, set_file) = use_state(scope, || None::<Rc<File>>);
    let (directory, set_directory) = use_state(scope, || None::<Rc<DirectorySelection>>);

    let theme_name = config.ui.theme.clone();
    let theme = use_memo(scope, theme_name.clone(), move || {
        configured_theme(&theme_name)
    });

    rsx! {
        Ui {
            value: Context {
                config: Rc::clone(config),
                keybindings: Rc::clone(keybindings),
                theme,
                repo,
                file: file.as_ref().map(Rc::clone),
                set_file: Some(set_file),
                directory: directory.as_ref().map(Rc::clone),
                set_directory: Some(set_directory),
                diff_service: Some(Rc::clone(diff_service)),
                version_control_service: Some(Rc::clone(version_control_service)),
                watcher_service: Some(Rc::clone(watcher_service)),
                set_repo: Some(set_repo),
            },
            { children.clone() }
        }
    }
}

fn configured_theme(name: &str) -> Theme {
    if name == "auto" {
        return Theme::from_environment();
    }
    Theme::named(name).unwrap_or_else(Theme::from_environment)
}
