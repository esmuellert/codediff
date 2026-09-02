//! Loads the selected file and chooses its view.

use std::rc::Rc;

use loom::{Node, Scope, component, rsx, use_context, use_effect, use_state};
use pipeline::diff::DiffContent;

use super::context::Ui;
use super::side_by_side::{SideBySide, SideBySideProps};
use super::single_file::{SingleFile, SingleFileProps};
use super::welcome::Welcome;

#[component]
pub fn DiffViewer(scope: &mut Scope) -> Node {
    let ctx = use_context::<Ui>(scope);
    let (content, set_content) = use_state(scope, || None::<Rc<DiffContent>>);
    let selected_file = ctx.file.as_ref().map(Rc::clone);
    let file_for_request = selected_file.as_ref().map(Rc::clone);
    let diff_service = ctx.diff_service.as_ref().map(Rc::clone);
    let syntax_for_file = ctx.syntax_service.as_ref().map(Rc::clone);

    use_effect(scope, selected_file.clone(), move || {
        set_content(&|_| None);
        let (Some(requested_file), Some(diff_service)) = (file_for_request, diff_service) else {
            return;
        };
        if let Some(syntax_service) = syntax_for_file {
            syntax_service.new_file();
        }
        let requested_file_for_response = Rc::clone(&requested_file);
        diff_service
            .get(&requested_file)
            .subscribe(move |response| {
                if response.file != *requested_file_for_response {
                    return;
                }
                let next = response.content.ok().map(Rc::new);
                set_content(&move |_| next.clone());
            });
    });

    let content = content.filter(|content| {
        selected_file
            .as_deref()
            .is_some_and(|file| content.file() == file)
    });
    if let (Some(DiffContent::Diff(diff)), Some(syntax_service)) =
        (content.as_deref(), ctx.syntax_service.as_ref())
    {
        for version in [
            file_types::DiffVersion::Original,
            file_types::DiffVersion::Modified,
        ] {
            syntax_service.request(&diff.file, version, diff.alignment.text(version), 2000);
        }
    }

    match content {
        Some(content) => match content.as_ref() {
            DiffContent::Diff(_) => rsx! { SideBySide { content: Rc::clone(&content) } },
            DiffContent::SingleFile(_) => {
                rsx! { SingleFile { content: Rc::clone(&content) } }
            }
        },
        None => rsx! { Welcome {} },
    }
}
