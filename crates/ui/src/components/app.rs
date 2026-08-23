//! The root: an explorer beside a diff, with the status line under both.

use std::rc::Rc;

use file_types::DiffType;
use loom::{
    Basis, Bubble, Column, ColumnProps, Divider, DividerProps, Layout, Listeners, Node, Row,
    RowProps, Scope, Text, TextProps, component, rsx, use_layout_effect, use_ref, use_state,
};

use super::context::{
    CursorContext, CursorContextProps, DiffsContext, DiffsContextProps, FileContext,
    FileContextProps, FirstCellContext, FirstCellContextProps, NoticeContext, NoticeContextProps,
    RepoContext, RepoContextProps, ThemeContext, ThemeContextProps, ViewLinesContext,
    ViewLinesContextProps,
};
use super::{Inline, InlineProps, SideBySide, SideBySideProps, SingleFile, SingleFileProps};
use super::{StatusLine, StatusLineProps};
use crate::theme::Theme;
use crate::view::Viewport;

/// Everything the session hands the root once, when it mounts.
#[derive(Clone)]
pub struct Session {
    pub theme: Rc<Theme>,
    pub repo: Option<Rc<std::path::Path>>,
    pub diffs: super::context::Diffs,
}

impl PartialEq for Session {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.theme, &other.theme) && self.repo == other.repo
    }
}

/// The whole interface.
///
/// Owns the pane state and passes it down through context, so every child
/// reads it directly.
#[component]
pub fn App(scope: &mut Scope, session: Session) -> Node {
    let (viewport, set_viewport) = use_state(scope, Viewport::new);
    let (layout, _set_layout) = use_state(scope, || DiffType::SideBySide);
    let (show_explorer, set_explorer) = use_state(scope, || false);
    let (notice, _set_notice) = use_state(scope, || None::<Rc<str>>);
    let (file, _set_file) = use_state(scope, || None::<Rc<file_types::File>>);

    let session = session.clone();
    let reading = session.diffs.reading();
    let total = reading
        .diff
        .as_ref()
        .map_or(0, |diff| diff.alignment.view_lines(layout).count() as u32);

    // How many rows the diff gets is layout's answer, not the render body's.
    // Reading it here and writing it back is what lets `view_lines` name a
    // real range on the frame that reaches the screen.
    let pane = use_ref(scope, || None::<loom::NodeHandle>);
    use_layout_effect(scope, loom::Always, move || {
        let height = u32::from(pane.current().map_or(0, |node| node.area().height));
        set_viewport(&move |mut held: Viewport| {
            held.set_height(height, total);
            held
        });
    });

    let listeners = Listeners::new().on_key(move |_| {
        set_explorer(&|shown| !shown);
        Bubble::Continue
    });

    let screen = match layout {
        DiffType::SideBySide => rsx! { SideBySide {} },
        DiffType::Inline => rsx! { Inline {} },
        DiffType::Single => rsx! { SingleFile {} },
    };

    let body = rsx! {
        Column {
            listeners: listeners,
            // When the minimum does not fit, loom shows this instead of the
            // tree.
            too_small: Some(rsx! {
                Text { text: "terminal too small".into(), .. }
            }),
            layout: Layout { grow: 1, min_width: 24, min_height: 2, ..Default::default() },
            ..,
            Row {
                ref: Some(pane),
                layout: Layout { grow: 1, ..Default::default() },
                ..,
                if show_explorer {
                    super::Explorer { on_open: Rc::new(|_| {}) }
                    Divider {
                        layout: Layout { basis: Basis::Length(1), shrink: 0, ..Default::default() },
                        symbol: "│",
                        style: session.theme.normal.patch(session.theme.divider),
                        ..
                    }
                }
                { screen }
            }
            StatusLine {}
        }
    };

    rsx! {
        ThemeContext {
            value: Rc::clone(&session.theme),
            RepoContext {
                value: session.repo.clone(),
                DiffsContext {
                    value: session.diffs.clone(),
                    FileContext {
                        value: file,
                        ViewLinesContext {
                            value: viewport.visible(total),
                            CursorContext {
                                value: viewport.cursor(),
                                FirstCellContext {
                                    value: viewport.left(),
                                    NoticeContext {
                                        value: notice,
                                        { body }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}
