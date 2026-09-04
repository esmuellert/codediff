//! The small gallery shell around production components.

use std::rc::Rc;

use loom::{
    Bubble, Column, ColumnProps, Layout, Listeners, Node, Scope, component, rsx, use_exit,
    use_state,
};
use pipeline::diff::DiffContent;
use ui::components::diff_viewer::DiffViewer;
use ui::components::explorer::Explorer;
use ui::components::side_by_side::{SideBySide, SideBySideProps};
use ui::components::single_file::{SingleFile, SingleFileProps};
use ui::components::{Context, Ui, UiProps};

use super::definition::{StoryComponent, StoryDefinition};
use super::gallery_header::{GalleryHeader, GalleryHeaderProps, Shortcut};

#[derive(Clone, Copy)]
pub(super) enum PreviewAction {
    Catalog,
    Previous,
    Next,
    Reset,
}

#[component]
pub(super) fn StoryPreview(
    scope: &mut Scope,
    definition: &'static StoryDefinition,
    base_context: Context,
    content: Option<Rc<DiffContent>>,
    navigate: Option<Rc<dyn Fn(PreviewAction)>>,
) -> Node {
    let (selected_file, set_selected_file) = use_state(scope, || None);
    let mut context = base_context.clone();
    context.file = selected_file.as_ref().map(Rc::clone);
    context.set_file = Some(set_selected_file);

    let exit = use_exit(scope);
    let navigate_key = navigate.as_ref().map(Rc::clone);
    let listeners = Listeners::new().on_key(move |key| {
        if key == loom::crokey::key!(q) {
            exit();
            return Bubble::Stop;
        }
        let Some(navigate) = &navigate_key else {
            return Bubble::Continue;
        };
        let action = if key == loom::crokey::key!(esc) {
            PreviewAction::Catalog
        } else if key == loom::crokey::key!('[') {
            PreviewAction::Previous
        } else if key == loom::crokey::key!(']') {
            PreviewAction::Next
        } else if key == loom::crokey::key!(r) {
            PreviewAction::Reset
        } else {
            return Bubble::Continue;
        };
        navigate(action);
        Bubble::Stop
    });
    let theme = *context.theme;
    let shortcuts = if navigate.is_some() {
        vec![
            Shortcut {
                key: "Esc",
                label: "Story list",
            },
            Shortcut {
                key: "[",
                label: "Previous",
            },
            Shortcut {
                key: "]",
                label: "Next",
            },
            Shortcut {
                key: "r",
                label: "Restart",
            },
            Shortcut {
                key: "q",
                label: "Quit",
            },
        ]
    } else {
        vec![Shortcut {
            key: "q",
            label: "Quit",
        }]
    };
    let body = match definition.component {
        StoryComponent::Welcome => rsx! { DiffViewer {} },
        StoryComponent::Explorer => rsx! { Explorer {} },
        StoryComponent::SideBySide => rsx! {
            SideBySide { content: Rc::clone(content.as_ref().expect("side-by-side story content")) }
        },
        StoryComponent::SingleFile => rsx! {
            SingleFile { content: Rc::clone(content.as_ref().expect("single-file story content")) }
        },
    };

    rsx! {
        Ui {
            value: context,
            Column {
                listeners: listeners,
                layout: Layout { grow: 1, fill: Some(theme.normal), ..Default::default() },
                ..,
                GalleryHeader {
                    key: 0u32,
                    title: Rc::from("STORY"),
                    context: Rc::from(definition.id),
                    shortcuts: Rc::from(shortcuts),
                }
                { body }
            }
        }
    }
}
