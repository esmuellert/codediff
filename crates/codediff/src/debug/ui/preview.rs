//! The small gallery shell around production components.

use std::rc::Rc;
use std::sync::{Arc, Mutex};

use diff::DiffContent;
use loom::{
    Bubble, Column, ColumnProps, Layout, Listeners, Node, Row, RowProps, Scope, component, rsx,
    use_exit, use_memo, use_ref, use_state,
};
use ui::components::border::{Border, BorderProps};
use ui::components::diff_viewer::{
    DiffCard, DiffCardProps, DiffCards, DiffCardsProps, DiffViewer, DiffViewerContainer,
    DiffViewerContainerProps, ScrollPosition,
};
use ui::components::explorer::Explorer;
use ui::components::revision_header::RevisionHeader;
use ui::components::revision_picker::RevisionPicker;
use ui::components::{Context, DirectorySelection, Ui, UiProps};
use ui::services::Resource;

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
    files: Rc<Vec<Resource<Arc<DiffContent>>>>,
    navigate: Option<Rc<dyn Fn(PreviewAction)>>,
) -> Node {
    let (selected_file, set_selected_file) = use_state(scope, || None);
    let (selected_directory, set_selected_directory) =
        use_state(scope, || None::<Rc<DirectorySelection>>);
    let initial_range = base_context.revision_range.clone();
    let (revision_range, set_revision_range) = use_state(scope, move || initial_range);
    let initial_picking = base_context.picking;
    let (picking, set_picking) = use_state(scope, move || initial_picking);
    let mut context = base_context.clone();
    context.revision_range = revision_range;
    context.set_revision_range = Some(set_revision_range);
    context.picking = picking;
    context.set_picking = Some(set_picking);
    context.file = selected_file.as_ref().map(Rc::clone);
    context.set_file = Some(set_selected_file);
    context.directory = selected_directory.as_ref().map(Rc::clone);
    context.set_directory = Some(set_selected_directory);

    let exit = use_exit(scope);
    let navigate_key = navigate.as_ref().map(Rc::clone);
    let listeners = Listeners::new().on_key(move |key| {
        if key == loom::crokey::key!(q) || key == loom::crokey::key!(ctrl - c) {
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
    let content = files.first().and_then(Resource::get).and_then(Result::ok);
    let content_key = content
        .as_ref()
        .map(|content| Arc::as_ptr(content) as usize);
    let syntax = context.syntax.clone();
    let content_for_colours = content.clone();
    let colours = use_memo(scope, content_key, move || {
        match (syntax, content_for_colours) {
            (Some(syntax), Some(content)) => syntax.highlight(content),
            _ => Arc::new(Mutex::new([Vec::new(), Vec::new()])),
        }
    });
    let colours = Arc::clone(&colours);
    let scroll_position = use_ref(scope, ScrollPosition::default);
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
        StoryComponent::RevisionPicker => rsx! { RevisionPicker {} },
        StoryComponent::RevisionHeader => rsx! { RevisionHeader {} },
        StoryComponent::SideBySide => rsx! {
            DiffViewerContainer {
                key: content.as_ref().map_or(0, |content| Arc::as_ptr(content) as usize),
                content: content.clone(),
                colours: Arc::clone(&colours),
                viewer_mode: file_types::ViewerMode::SideBySide,
                scroll_position,
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
                viewer_mode: file_types::ViewerMode::Inline,
                scroll_position,
                wrap: true,
                compact: definition.compact,
                auto_focus: false,
            }
        },
        StoryComponent::SingleFile => rsx! {
            DiffViewerContainer {
                content: content.clone(),
                colours: Arc::clone(&colours),
                viewer_mode: file_types::ViewerMode::Single,
                scroll_position,
                wrap: true,
                compact: false,
                auto_focus: false,
            }
        },
        StoryComponent::Card => match content.clone() {
            Some(content) => rsx! {
                DiffCard {
                    content,
                    colours: Arc::clone(&colours),
                    viewer_mode: file_types::ViewerMode::SideBySide,
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
                viewer_mode: file_types::ViewerMode::Inline,
                wrap: false,
            }
        },
    };

    rsx! {
        Ui {
            value: context,
            Column {
                listeners,
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
