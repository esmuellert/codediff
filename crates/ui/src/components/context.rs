//! Shared state, and the one provider that owns it.

use std::ops::Range;
use std::path::Path;
use std::rc::Rc;

use file_types::File;
use loom::{
    Node, Scope, SetState, component, context,
    rsx, use_effect, use_state,
};

use crate::services::file::FileService;
use crate::theme::Theme;

/// Everything a component reads.
#[derive(Clone)]
pub struct Context {
    pub theme: Rc<Theme>,
    pub repo: Rc<Path>,
    pub files: Rc<Vec<File>>,
    pub cursor: u32,
    pub view_lines: Range<u32>,
    pub set_repo: Option<SetState<Rc<Path>>>,
}

impl Default for Context {
    fn default() -> Self {
        Self {
            theme: Rc::new(Theme::DARK),
            repo: Rc::from(Path::new("")),
            files: Rc::new(Vec::new()),
            cursor: 0,
            view_lines: 0..0,
            set_repo: None,
        }
    }
}

impl Context {
    fn same(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.theme, &other.theme)
            && Rc::ptr_eq(&self.repo, &other.repo)
            && Rc::ptr_eq(&self.files, &other.files)
            && self.cursor == other.cursor
            && self.view_lines == other.view_lines
            && self.set_repo == other.set_repo
    }
}

context!(
    pub Ui: Context = Context::default(),
    |a: &Context, b: &Context| a.same(b)
);

/// Owns the shared state and the effects that keep it current.
#[component]
pub fn UiProvider(
    scope: &mut Scope,
    cwd: Rc<Path>,
    file_service: Rc<FileService>,
    children: loom::Children,
) -> Node {
    let initial = Rc::clone(cwd);
    let (repo, set_repo) = use_state(scope, || initial);
    let (file_list, set_file_list) = use_state(scope, || Rc::new(Vec::<File>::new()));
    let (cursor, _set_cursor) = use_state(scope, || 0u32);
    // Bumped when the filesystem changes, so the file-fetching effect re-runs.
    let (version, set_version) = use_state(scope, || 0u32);

    let theme = Rc::new(Theme::from_environment());

    // Get the file list. Re-runs when the repo changes or the worktree does.
    let svc = Rc::clone(file_service);
    let repo_path = Rc::clone(&repo);
    use_effect(scope, (Rc::clone(&repo), version), move || {
        svc.get(&repo_path).subscribe(move |list: Vec<File>| {
            set_file_list(&move |_| Rc::new(list.clone()));
        });
    });

    // When the filesystem changes, bump the version so the effect above re-runs.
    let svc_fs = Rc::clone(file_service);
    use_effect(scope, (), move || {
        svc_fs.on_fs_changed().subscribe(move |what: watcher::Refresh| {
            if what.worktree || what.index {
                set_version(&|v| v + 1);
            }
        });
    });

    rsx! {
        Ui {
            value: Context {
                theme,
                repo,
                files: file_list,
                cursor,
                view_lines: 0..0,
                set_repo: Some(set_repo),
            },
            { children.clone() }
        }
    }
}
