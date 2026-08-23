//! The file list.

use std::rc::Rc;

use file_types::File;
use loom::{
    Bubble, Column, ColumnProps, Layout, Listeners, Node, Scope, component, rsx, use_context,
    use_state,
};

use super::context::{CursorContext, ThemeContext, ViewLinesContext};
use super::entry::{Body, Content, Entry, EntryProps, Indent, Status};

/// The list worker's answer, as the explorer reads it.
#[derive(Clone, Default)]
pub struct Listing {
    inner: Rc<std::cell::RefCell<ListingInner>>,
}

#[derive(Default)]
struct ListingInner {
    files: Rc<Vec<File>>,
    listeners: Vec<loom::Notify>,
}

impl Listing {
    pub fn new() -> Self {
        Self::default()
    }

    /// Replaces the list, and tells every reader.
    pub fn fill(&self, files: Vec<File>) {
        let listeners = {
            let mut inner = self.inner.borrow_mut();
            inner.files = Rc::new(files);
            inner.listeners.clone()
        };
        for listener in listeners {
            listener.changed();
        }
    }

    pub fn files(&self) -> Rc<Vec<File>> {
        Rc::clone(&self.inner.borrow().files)
    }
}

impl loom::ExternalStore for Listing {
    type Value = Vec<File>;

    fn subscribe(&self, notify: loom::Notify) -> loom::Subscription {
        self.inner.borrow_mut().listeners.push(notify);
        let inner = Rc::clone(&self.inner);
        loom::Subscription::new(move || inner.borrow_mut().listeners.clear())
    }

    fn snapshot(&self) -> loom::Snapshot<Self::Value> {
        loom::Snapshot::from(self.files())
    }
}

loom::context!(pub ListingContext: Listing = Listing::new(), |_a: &Listing, _b: &Listing| true);

/// The file list.
///
/// Subscribes to the list worker rather than being handed the files.
#[component]
pub fn Explorer(scope: &mut Scope, on_open: Rc<dyn Fn(File)>) -> Node {
    let theme = use_context::<ThemeContext>(scope);
    let view_lines = use_context::<ViewLinesContext>(scope);
    let cursor = use_context::<CursorContext>(scope);
    let listing = use_context::<ListingContext>(scope);

    let files = loom::use_sync_external_store(scope, &listing);
    // Tree mode shows short names and indent lines; list mode shows full
    // paths and none. The mode is toggled by a key and lives here.
    let (tree, set_tree) = use_state(scope, || true);

    let on_open = Rc::clone(on_open);
    let listeners = Listeners::new().on_key(move |key| {
        if key == crokey::key!(t) {
            set_tree(&|shown| !shown);
            return Bubble::Stop;
        }
        Bubble::Continue
    });
    let _ = on_open;

    let rows: Vec<Node> = std::iter::once(Content::Heading {
        name: Rc::from("Changes"),
        files: files.len(),
    })
    .chain(files.iter().map(|file| Content::File {
        name: Rc::from(if tree { file.path().file_name() } else { file.path().as_str() }),
    }))
    .enumerate()
    .skip(view_lines.start as usize)
    .take(view_lines.len())
    .map(|(offset, content)| {
        let indent = Indent {
            lines: Rc::from(match (&content, tree) {
                (Content::File { .. }, true) => "  ",
                _ => "",
            }),
        };
        let (body, status) = match &content {
            Content::Heading { name, files } => (
                Body { icon: " ", text: Rc::from(format!("{name} ({files})").as_str()) },
                None,
            ),
            Content::Directory { name, open } => (
                Body { icon: if *open { "▾" } else { "▸" }, text: Rc::clone(name) },
                None,
            ),
            Content::File { name } => (
                Body { icon: " ", text: Rc::clone(name) },
                Some(Status { added: 0, removed: 0, letter: "M" }),
            ),
        };

        rsx! {
            Entry {
                key: offset,
                indent: indent,
                body: body,
                status: status,
                selected: offset as u32 == cursor,
            }
        }
    })
    .collect();

    rsx! {
        Column {
            layout: Layout {
                basis: loom::Basis::Length(28),
                min_width: 8,
                fill: Some(theme.normal),
                ..Default::default()
            },
            listeners: listeners,
            ..,
            { rows }
        }
    }
}
