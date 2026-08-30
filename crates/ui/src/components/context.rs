//! Shared state, and the one provider that owns it.

use std::ops::Range;
use std::path::Path;
use std::rc::Rc;

use file_types::File;
use loom::{
    Node, Scope, SetState, component, context,
    rsx, use_effect, use_memo, use_state,
};
use syntax::Store;

use super::explorer::scroll_top;
use crate::services::diff::DiffService;
use crate::services::syntax::SyntaxService;
use crate::services::files::FilesService;
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
    pub set_cursor: Option<SetState<u32>>,
    pub file: Option<Rc<File>>,
    pub set_file: Option<SetState<Option<Rc<File>>>>,
    pub diff: Option<Rc<pipeline::diff::DiffContent>>,
    pub syntax: Option<Rc<Store>>,
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
            set_cursor: None,
            file: None,
            set_file: None,
            diff: None,
            syntax: None,
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
            && self.set_cursor == other.set_cursor
            && same_rc(&self.file, &other.file)
            && self.set_file == other.set_file
            && same_rc(&self.diff, &other.diff)
            && same_rc(&self.syntax, &other.syntax)
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

/// Owns the shared state and the effects that keep it current.
#[component]
pub fn UiProvider(
    scope: &mut Scope,
    cwd: Rc<Path>,
    file_service: Rc<FilesService>,
    diff_service: Rc<DiffService>,
    syntax_service: Rc<SyntaxService>,
    rows: u32,
    children: loom::Children,
) -> Node {
    let initial = Rc::clone(cwd);
    let (repo, set_repo) = use_state(scope, || initial);
    let (file_list, set_file_list) = use_state(scope, || Rc::new(Vec::<File>::new()));
    let (cursor, set_cursor) = use_state(scope, || 0u32);
    let (top, set_top) = use_state(scope, || 0u32);
    let (file, set_file) = use_state(scope, || None::<Rc<File>>);
    let (diff, set_diff) = use_state(scope, || None::<Rc<pipeline::diff::DiffContent>>);
    let (syntax, set_syntax) = use_state(scope, || None::<Rc<Store>>);

    // Read once. A new Rc each render would tell every reader the
    // context changed, and the tree would never settle.
    let theme = use_memo(scope, (), Theme::from_environment);

    // Get the file list.
    let svc = Rc::clone(file_service);
    let repo_path = Rc::clone(&repo);
    let set_files_for_fetch = set_file_list.clone();
    use_effect(scope, Rc::clone(&repo), move || {
        svc.get(&repo_path).subscribe(move |list: Vec<File>| {
            set_files_for_fetch(&move |_| Rc::new(list.clone()));
        });
    });

    // When the filesystem changes, re-fetch the file list.
    let svc_fs = Rc::clone(file_service);
    let repo_for_fs = Rc::clone(&repo);
    use_effect(scope, (), move || {
        svc_fs.on_fs_changed().subscribe(move |what: watcher::Refresh| {
            if what.worktree || what.index {
                svc_fs.refresh(&repo_for_fs);
            }
        });
    });

    // Subscribe to syntax updates — each chunk delivers a new store snapshot.
    let ssvc = Rc::clone(syntax_service);
    use_effect(scope, (), move || {
        ssvc.subscribe().subscribe(move |store: Rc<Store>| {
            set_syntax(&move |_| Some(Rc::clone(&store)));
        });
    });

    // When a file is focused, fetch its diff.
    let dsvc = Rc::clone(diff_service);
    let ssvc2 = Rc::clone(syntax_service);
    let file_for_effect = file.clone();
    use_effect(scope, file.clone(), move || {
        if let Some(ref file) = file_for_effect {
            ssvc2.new_file();
            dsvc.get(file).subscribe(move |response: pipeline::diff::Response| {
                match response.content {
                    Ok(content) => {
                        let rc = Rc::new(content);
                        set_diff(&move |_| Some(Rc::clone(&rc)));
                    }
                    Err(_) => set_diff(&|_| None),
                }
            });
        }
    });

    // Request syntax colours for whatever diff is showing.
    if let Some(ref content) = diff.as_deref() {
        if let pipeline::diff::DiffContent::Diff(d) = content {
            let last = cursor.saturating_add(2000);
            for version in [file_types::DiffVersion::Original, file_types::DiffVersion::Modified] {
                syntax_service.request(&d.file, version, d.alignment.text(version), last);
            }
        }
    }

    // Compute the scroll.
    let total = file_list.len() as u32;
    let new_top = scroll_top(cursor, total, *rows, top);
    if new_top != top {
        set_top(&move |_| new_top);
    }

    rsx! {
        Ui {
            value: Context {
                theme,
                repo,
                files: file_list,
                cursor,
                view_lines: new_top..new_top + rows,
                set_repo: Some(set_repo),
                set_cursor: Some(set_cursor),
                file: file.as_ref().map(Rc::clone),
                set_file: Some(set_file),
                diff: diff.clone(),
                syntax: syntax.clone(),
            },
            { children.clone() }
        }
    }
}
