//! The small gallery shell around production components.

// The opened files are spelled out in full rather than hidden behind a name.
#![allow(clippy::type_complexity)]

use std::rc::Rc;
use std::sync::{Arc, Mutex};

use loom::{
    Bubble, Column, ColumnProps, Layout, Listeners, Node, Row, RowProps, Scope, component, rsx,
    use_exit, use_ref, use_state,
};
use pipeline::diff::DiffContent;
use syntax::Span;
use ui::components::border::{Border, BorderProps};
use ui::components::diff_card::{DiffCard, DiffCardProps};
use ui::components::diff_cards::{DiffCards, DiffCardsProps};
use ui::components::diff_viewer::{DiffViewer, ViewState};
use ui::components::diff_viewer_container::{DiffViewerContainer, DiffViewerContainerProps};
use ui::components::explorer::Explorer;
use ui::components::{Context, DirectorySelection, Ui, UiProps};

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
    files: Rc<
        Vec<(
            Arc<Mutex<Option<Result<Arc<DiffContent>, String>>>>,
            Arc<Mutex<[Vec<Vec<Span>>; 2]>>,
        )>,
    >,
    navigate: Option<Rc<dyn Fn(PreviewAction)>>,
) -> Node {
    let (selected_file, set_selected_file) = use_state(scope, || None);
    let (selected_directory, set_selected_directory) =
        use_state(scope, || None::<Rc<DirectorySelection>>);
    let mut context = base_context.clone();
    context.file = selected_file.as_ref().map(Rc::clone);
    context.set_file = Some(set_selected_file);
    context.directory = selected_directory.as_ref().map(Rc::clone);
    context.set_directory = Some(set_selected_directory);

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
    // Code stories show the files they opened, once they have been diffed.
    let content = files
        .first()
        .and_then(|(diff, _)| diff.lock().unwrap().clone())
        .and_then(Result::ok);
    let colours = files
        .first()
        .map_or_else(Arc::default, |(_, colours)| Arc::clone(colours));
    let view_state = use_ref(scope, ViewState::default);
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
        StoryComponent::ExplorerCards => rsx! {
            Row {
                layout: Layout { grow: 1, ..Default::default() },
                ..,
                Border {
                    layout: Layout { basis: loom::Basis::Length(context.config.ui.explorer_width), shrink: 1, ..Default::default() },
                    Explorer {}
                }
                Border {
                    layout: Layout { grow: 1, ..Default::default() },
                    DiffViewer {}
                }
            }
        },
        StoryComponent::SideBySide => rsx! {
            DiffViewerContainer {
                key: content.as_ref().map_or(0, |content| Arc::as_ptr(content) as usize),
                content: content.clone(),
                colours: Arc::clone(&colours),
                view_layout: file_types::DiffType::SideBySide,
                view_state: view_state,
                wrap: true,
                compact: definition.compact,
                auto_focus: false,
            }
        },
        StoryComponent::Inline => rsx! {
            DiffViewerContainer {
                key: content.as_ref().map_or(0, |content| Arc::as_ptr(content) as usize),
                content: content.clone(),
                colours: Arc::clone(&colours),
                view_layout: file_types::DiffType::Inline,
                view_state: view_state,
                wrap: true,
                compact: definition.compact,
                auto_focus: false,
            }
        },
        StoryComponent::SingleFile => rsx! {
            DiffViewerContainer {
                content: content.clone(),
                colours: Arc::clone(&colours),
                view_layout: file_types::DiffType::Single,
                view_state: view_state,
                wrap: true,
                compact: false,
                auto_focus: false,
            }
        },
        StoryComponent::Card => match content.clone() {
            Some(content) => rsx! {
                DiffCard {
                    content: content,
                    colours: Arc::clone(&colours),
                    view_layout: file_types::DiffType::SideBySide,
                    wrap: false,
                    height: None,
                    auto_focus: true,
                }
            },
            None => Node::Empty,
        },
        StoryComponent::Cards => rsx! {
            DiffCards {
                files: Rc::clone(files),
                more_files: false,
                open_next: Rc::new(|| {}) as Rc<dyn Fn()>,
                view_layout: file_types::DiffType::Inline,
                wrap: false,
            }
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
