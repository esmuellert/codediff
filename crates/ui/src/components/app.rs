//! The root: an explorer beside a diff, with the status line under both.

use std::rc::Rc;

use file_types::DiffType;
use loom::{
    Basis, Column, ColumnProps, Divider, DividerProps, Layout, Node, Row, RowProps, Scope, Text,
    TextProps, component, rsx, use_context, use_layout_effect, use_ref, use_state,
};

use super::context::{
    CursorContext, CursorContextProps, DiffDataContext, FileContext, FileContextProps,
    FirstCellContext, FirstCellContextProps, NoticeContext, NoticeContextProps, ThemeContext,
    ViewLinesContext, ViewLinesContextProps,
};
use super::explorer::{Explorer, ExplorerProps};
use super::{Inline, InlineProps, SideBySide, SideBySideProps, SingleFile, SingleFileProps};
use super::{StatusLine, StatusLineProps};
use crate::view::Viewport;

/// The whole interface.
///
/// Owns the pane state and passes it down through context, so every child
/// reads it directly. Theme, repository and the worker stores are provided
/// above it, because they last as long as the session does.
#[component]
pub fn App(scope: &mut Scope) -> Node {
    let theme = use_context::<ThemeContext>(scope);
    let diff_data = use_context::<DiffDataContext>(scope);

    let (viewport, set_viewport) = use_state(scope, Viewport::new);
    let (layout, _set_layout) = use_state(scope, || DiffType::SideBySide);
    let (show_explorer, _set_explorer) = use_state(scope, || false);
    let (notice, _set_notice) = use_state(scope, || None::<Rc<str>>);
    let (file, set_file) = use_state(scope, || None::<Rc<file_types::File>>);

    let loaded = diff_data.reading();
    let total = loaded
        .diff
        .as_ref()
        .map_or(0, |diff| diff.alignment.view_lines(layout).count() as u32);

    // How many rows the diff gets is layout's answer, not the render body's.
    // Loaded it back is what lets `view_lines` name a real range on the
    // frame that reaches the screen.
    let pane = use_ref(scope, || None::<loom::NodeHandle>);
    use_layout_effect(scope, loom::Always, move || {
        let height = u32::from(pane.current().map_or(0, |node| node.area().height));
        set_viewport(&move |mut held: Viewport| {
            held.set_height(height, total);
            held
        });
    });

    let on_open: Rc<dyn Fn(file_types::File)> = Rc::new(move |opened| {
        set_file(&move |_| Some(Rc::new(opened.clone())));
    });

    let screen = match layout {
        DiffType::SideBySide => rsx! { SideBySide {} },
        DiffType::Inline => rsx! { Inline {} },
        DiffType::Single => rsx! { SingleFile {} },
    };

    let body = rsx! {
        Column {
            // When the minimum does not fit, loom shows this instead of the
            // tree.
            too_small: Some(rsx! { Text { text: "terminal too small".into(), .. } }),
            layout: Layout { grow: 1, min_width: 24, min_height: 2, ..Default::default() },
            ..,
            Row {
                ref: Some(pane),
                layout: Layout { grow: 1, ..Default::default() },
                ..,
                if show_explorer {
                    Explorer { on_open: on_open }
                    Divider {
                        layout: Layout { basis: Basis::Length(1), shrink: 0, ..Default::default() },
                        symbol: "│",
                        style: theme.normal.patch(theme.divider),
                        ..
                    }
                }
                { screen }
            }
            StatusLine {}
        }
    };

    rsx! {
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
