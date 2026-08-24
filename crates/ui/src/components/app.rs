//! The root: an explorer beside a diff, with the status line under both.

use std::cell::RefCell;
use std::rc::Rc;

use file_types::DiffType;
use loom::{
    Basis, Column, ColumnProps, Divider, DividerProps, Layout, Node, Row, RowProps, Scope, Text,
    TextProps, component, rsx, use_context, use_layout_effect, use_ref, use_state,
};

use super::context::{
    CursorContext, CursorContextProps, FileContext, FileContextProps, FirstCellContext,
    FirstCellContextProps, NoticeContext, NoticeContextProps, PaneContext, PaneContextProps,
    ScreenMapContext, ScreenMapContextProps, ThemeContext, ViewLinesContext, ViewLinesContextProps,
};
use super::explorer::{Explorer, ExplorerProps};
use super::{Inline, InlineProps, SideBySide, SideBySideProps, SingleFile, SingleFileProps};
use super::{StatusLine, StatusLineProps};
use crate::state::{PaneId, View};

/// How narrow the list may get, and how much a diff needs beside it.
const MIN_LIST: u16 = 8;
const MIN_DIFF: u16 = 20;

/// The whole interface.
///
/// The session owns the model and hands it in. What a key does to it is the
/// session's business; what the screen says about it is this.
#[component]
pub fn App(
    scope: &mut Scope,
    view: Rc<RefCell<View>>,
    notice: Option<Rc<str>>,
    map: Rc<RefCell<crate::screen_map::ScreenMap>>,
) -> Node {
    let theme = use_context::<ThemeContext>(scope);

    let body = use_ref(scope, || None::<loom::NodeHandle>);
    let (height, set_height) = use_state(scope, || 0u32);
    let (width, set_width) = use_state(scope, || u16::MAX);
    // One slot per pane. A tab holds at most two, and a hook cannot run
    // behind a condition, so both are made every render.
    let first = use_ref(scope, || None::<loom::NodeHandle>);
    let second = use_ref(scope, || None::<loom::NodeHandle>);

    let read = view.borrow();
    // Two panes need room for both and the divider between them. When that
    // does not fit, the reader sees the one they are working in rather than
    // nothing at all.
    let split = read.tab().is_split() && width >= MIN_LIST + 1 + MIN_DIFF;
    let panes: Vec<PaneId> = if split {
        read.tab().ids().collect()
    } else {
        vec![read.tab().focus()]
    };
    drop(read);

    let slots = [first, second];
    let recorded: Vec<(PaneId, loom::Ref<Option<loom::NodeHandle>>)> =
        panes.iter().copied().zip(slots).collect();

    // Layout knows how many rows a pane has; the render body does not. The
    // model is told as soon as layout has decided, so a page motion agrees
    // with what is on screen.
    let held = Rc::clone(view);
    let filling = Rc::clone(&map);
    use_layout_effect(scope, loom::Always, move || {
        let area = body.current().map_or(ratatui::layout::Rect::ZERO, |node| node.area());
        let rows = u32::from(area.height);
        set_height(&move |_| rows);
        let across = area.width;
        set_width(&move |_| across);
        {
            let mut view = held.borrow_mut();
            let ids: Vec<PaneId> = view.tab().ids().collect();
            for id in ids {
                let (buffer, viewport) = view.pane_mut(id);
                let lines = buffer.view_lines();
                viewport.set_height(rows, lines);
            }
        }
        // Where each pane landed. Cleared here, before the screens append
        // their own text areas, because this effect is queued first.
        let mut map = filling.borrow_mut();
        map.panes.clear();
        map.text_areas.clear();
        map.body = area;
        for (id, slot) in &recorded {
            if let Some(node) = *slot.current() {
                map.panes.push((*id, node.area()));
            }
        }
    });
    let _ = height;

    let divider = rsx! {
        Divider {
            layout: Layout { basis: Basis::Length(1), shrink: 0, ..Default::default() },
            symbol: "│",
            style: theme.normal.patch(theme.divider),
            ..
        }
    };

    let mut children = Vec::new();
    for (at, &id) in panes.iter().enumerate() {
        // The list keeps a divider beside it, so the two panes never touch.
        if at > 0 {
            children.push(divider.clone());
        }
        children.push(pane(Rc::clone(view), id, split && at == 0, slots[at], Rc::clone(map)));
    }

    rsx! {
        Column {
            // When the minimum does not fit, loom shows this instead of the
            // tree.
            too_small: Some(rsx! { Text { text: "terminal too small".into(), .. } }),
            // What the screen needs is whatever is in it: a list asks for
            // less than a diff, so the panes carry their own minimums.
            layout: Layout { grow: 1, min_height: 2, ..Default::default() },
            ..,
            Row {
                ref: Some(body),
                layout: Layout { grow: 1, ..Default::default() },
                ..,
                { children }
            }
            NoticeContext {
                value: notice.clone(),
                { status(Rc::clone(view)) }
            }
        }
    }
}

/// One pane, wrapped in the context that says where *it* is looking.
///
/// Each pane has its own position, so each gets its own providers rather than
/// one set for the whole tree.
fn pane(
    view: Rc<RefCell<View>>,
    id: PaneId,
    is_list: bool,
    slot: loom::Ref<Option<loom::NodeHandle>>,
    map: Rc<RefCell<crate::screen_map::ScreenMap>>,
) -> Node {
    let read = view.borrow();
    let held = read.tab().pane(id);
    let buffer = read.buffer(held.buffer);

    let lines = buffer.view_lines();
    let visible = held.viewport.visible(lines);
    let cursor = held.viewport.cursor();
    let first_cell = held.viewport.left();
    let file = buffer.file().cloned().map(Rc::new);
    let diff_type = buffer.diff_type();
    let is_explorer = buffer.as_explorer().is_some();
    let which = held.buffer;
    drop(read);

    let screen = if is_explorer {
        rsx! {
            Explorer {
                view: Rc::clone(&view),
                buffer: which,
                on_open: Rc::new(|_| {}),
            }
        }
    } else {
        match diff_type {
            Some(DiffType::SideBySide) => rsx! {
                SideBySide { view: Rc::clone(&view), buffer: which }
            },
            Some(DiffType::Inline) => rsx! {
                Inline { view: Rc::clone(&view), buffer: which }
            },
            Some(DiffType::Single) => rsx! {
                SingleFile { view: Rc::clone(&view), buffer: which }
            },
            None => Node::Empty,
        }
    };

    // What a pane needs is what it shows: a list asks for less than a diff.
    let least = if is_explorer { MIN_LIST } else { MIN_DIFF };
    let layout = if is_list {
        Layout { basis: Basis::Length(40), min_width: least, ..Default::default() }
    } else {
        Layout { grow: 1, min_width: least, ..Default::default() }
    };

    rsx! {
        Row {
            ref: Some(slot),
            layout: layout,
            ..,
            PaneContext {
                value: Some(id),
                ScreenMapContext {
                    value: map,
                    FileContext {
                        value: file,
                        ViewLinesContext {
                            value: visible,
                            CursorContext {
                                value: cursor,
                                FirstCellContext {
                                    value: first_cell,
                                    { screen }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// The status line reads the focused pane, whichever that is.
fn status(view: Rc<RefCell<View>>) -> Node {
    let read = view.borrow();
    let held = read.focused();
    let buffer = read.focused_buffer();
    let lines = buffer.view_lines();
    let cursor = held.viewport.cursor();
    let file = buffer.file().cloned().map(Rc::new);
    let changes = buffer.blocks().len();
    let change = buffer.block_at(cursor);
    let timed_out = buffer.hit_timeout();
    let exhausted = buffer.exhausted();
    drop(read);

    rsx! {
        FileContext {
            value: file,
            ViewLinesContext {
                value: 0..lines,
                CursorContext {
                    value: cursor,
                    StatusLine {
                        changes: changes,
                        change: change,
                        timed_out: timed_out,
                        exhausted: exhausted,
                    }
                }
            }
        }
    }
}
