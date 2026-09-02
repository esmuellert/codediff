//! Shared state, and the one provider that owns it.

use std::path::Path;
use std::rc::Rc;

use file_types::File;
use loom::{Node, Scope, SetState, component, context, rsx, use_memo, use_state};

use crate::services::diff::DiffService;
use crate::services::files::FilesService;
use crate::services::syntax::SyntaxService;
use crate::services::version_control::VersionControlService;
use crate::theme::Theme;

/// Everything a component reads.
#[derive(Clone)]
pub struct Context {
    pub theme: Rc<Theme>,
    pub repo: Rc<Path>,
    pub file: Option<Rc<File>>,
    pub set_file: Option<SetState<Option<Rc<File>>>>,
    pub file_service: Option<Rc<FilesService>>,
    pub diff_service: Option<Rc<DiffService>>,
    pub syntax_service: Option<Rc<SyntaxService>>,
    pub version_control_service: Option<Rc<VersionControlService>>,
    pub set_repo: Option<SetState<Rc<Path>>>,
}

impl Default for Context {
    fn default() -> Self {
        Self {
            theme: Rc::new(Theme::DARK),
            repo: Rc::from(Path::new("")),
            file: None,
            set_file: None,
            file_service: None,
            diff_service: None,
            syntax_service: None,
            version_control_service: None,
            set_repo: None,
        }
    }
}

impl Context {
    fn same(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.theme, &other.theme)
            && Rc::ptr_eq(&self.repo, &other.repo)
            && same_rc(&self.file, &other.file)
            && self.set_file == other.set_file
            && same_rc(&self.file_service, &other.file_service)
            && same_rc(&self.diff_service, &other.diff_service)
            && same_rc(&self.syntax_service, &other.syntax_service)
            && same_rc(
                &self.version_control_service,
                &other.version_control_service,
            )
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
    file_service: Rc<FilesService>,
    diff_service: Rc<DiffService>,
    syntax_service: Rc<SyntaxService>,
    version_control_service: Rc<VersionControlService>,
    children: loom::Children,
) -> Node {
    let initial = Rc::clone(cwd);
    let (repo, set_repo) = use_state(scope, || initial);
    let (file, set_file) = use_state(scope, || None::<Rc<File>>);

    let theme = use_memo(scope, (), Theme::from_environment);

    rsx! {
        Ui {
            value: Context {
                theme,
                repo,
                file: file.as_ref().map(Rc::clone),
                set_file: Some(set_file),
                file_service: Some(Rc::clone(file_service)),
                diff_service: Some(Rc::clone(diff_service)),
                syntax_service: Some(Rc::clone(syntax_service)),
                version_control_service: Some(Rc::clone(version_control_service)),
                set_repo: Some(set_repo),
            },
            { children.clone() }
        }
    }
}
