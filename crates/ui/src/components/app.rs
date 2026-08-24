//! The root: the explorer beside a diff, with the status line under both.
//!
//! Everything the reader can move is a `use_state` here, offered downwards as
//! context. Nothing hands this component a model, and nothing below it writes
//! one: a key changes a state slot, and the frame follows from the slots.

use std::cell::Cell;
use std::rc::Rc;

use file_types::{DiffType, File};
use loom::{
    Basis, Bubble, Column, ColumnProps, Divider, DividerProps, Layout, Listeners, Node, Row,
    RowProps, Scope, Text, TextProps, component, context, rsx, use_context, use_layout_effect,
    use_memo, use_ref, use_state, use_sync_external_store,
};

use super::context::{
    ArrangementContext, ArrangementContextProps, CursorCellContext, CursorContext,
    CursorContextProps, DiffStoreContext, ExhaustedContext, ExhaustedContextProps, FileContext,
    FileContextProps, FileListStoreContext, FirstCellContext, FirstCellContextProps,
    LayoutCellContext, LayoutContext, LayoutContextProps, NoticeContext, NoticeContextProps,
    OnSelectContext, OnSelectContextProps, OpenContext, PaneContext, PaneContextProps,
    ScreenMapCellContext, SelectionCellContext, SelectionContext, SelectionContextProps,
    SyntaxOnContext, SyntaxOnContextProps, ThemeContext, ViewLinesCellContext, ViewLinesContext,
    ViewLinesContextProps,
};
use super::{
    Explorer, ExplorerProps, Inline, InlineProps, SideBySide, SideBySideProps, SingleFile,
    SingleFileProps, StatusLine, StatusLineProps,
};
use crate::app::Flow;
use crate::input::{
    Action, BufferAction, KeymapType, ProgramAction, Resolution, Resolver, TabAction, ViewAction,
};
use crate::screen_map::PaneId;
use crate::state::buffer::explorer::{Arrangement, Explorer as Model};
use crate::state::selection::Selection;
use crate::state::{Direction, Viewport};

/// Columns the list gets once something is open beside it.
///
/// Wide enough for a name, an indent and a status letter without wrapping,
/// which is what the plugin also settled on.
const LIST_WIDTH: u16 = 40;
/// What each pane needs before the screen gives up on showing both: a list
/// asks for less than a diff.
const MIN_LIST: u16 = 8;
const MIN_DIFF: u16 = 20;
/// View lines one turn of the wheel moves.
const WHEEL: i32 = 3;

context!(
    /// What the interface asks the session to do next.
    ///
    /// A component cannot leave the program or hand the terminal back, so it
    /// says which and the session does it. `Root` provides it; the default
    /// does nothing, which is what a tree mounted without a session wants.
    pub FlowContext: Rc<dyn Fn(Flow)> = Rc::new(|_| {}),
    |a: &Rc<dyn Fn(Flow)>, b: &Rc<dyn Fn(Flow)>| Rc::ptr_eq(a, b)
);

/// The whole interface.
#[component]
pub fn App(scope: &mut Scope) -> Node {
    let theme = use_context::<ThemeContext>(scope);
    let store = use_context::<DiffStoreContext>(scope);
    let listing = use_context::<FileListStoreContext>(scope);
    let on_flow = use_context::<FlowContext>(scope);
    let on_open = use_context::<OpenContext>(scope);
    // The workers fill the stores; this subscribes rather than being handed
    // what they produced.
    let reading = use_sync_external_store(scope, &store);
    let files = use_sync_external_store(scope, &listing);

    let cursor_cell = use_context::<CursorCellContext>(scope);
    let vl_cell = use_context::<ViewLinesCellContext>(scope);
    let layout_cell = use_context::<LayoutCellContext>(scope);
    let selection_cell = use_context::<SelectionCellContext>(scope);
    let map_cell = use_context::<ScreenMapCellContext>(scope);

    // How the reader has arranged the list: which rows are folded, and
    // whether it is nested or flat. The rows themselves follow from this and
    // the store, so they are worked out rather than kept and changed in place
    // — the component that draws them works out the same rows from the same
    // two things.
    let (arrangement, set_arrangement) = use_state(scope, Arrangement::default);
    let model = use_memo(scope, (files.clone(), arrangement.clone()), || {
        Model::arranged(files.to_vec(), &arrangement)
    });
    // The list the last render worked out. A rebuilt list renumbers every
    // row, so the file the reader was on is named in the old one and looked
    // up in the new (D54).
    let previous = use_ref(scope, || Rc::clone(&model));
    let before = Rc::clone(&previous.current());
    *previous.current() = Rc::clone(&model);

    // One position per pane. The list and the diff are different documents, so
    // where the reader is in one says nothing about the other.
    let (list, set_list) = use_state(scope, || {
        // The reader starts on the first file, not on the heading above it
        // (D48), so where the list begins is settled with the model.
        let mut viewport = Viewport::new();
        viewport.place(model.first_file(), model.view_lines());
        viewport
    });
    let (diff, set_diff) = use_state(scope, Viewport::new);
    let (on_diff, set_on_diff) = use_state(scope, || false);

    let (layout, set_layout) = use_state(scope, || DiffType::SideBySide);
    let (notice, set_notice) = use_state(scope, || None::<Rc<str>>);
    let (syntax_on, set_syntax_on) = use_state(scope, || true);
    let (selection, set_selection) = use_state(scope, || None::<Selection>);
    // Which way `]c` or `[c` went with nowhere to go, cleared by the next key.
    let (exhausted, set_exhausted) = use_state(scope, || None::<Direction>);

    // The row of panes and each pane in it, measured after layout; and the
    // keys typed so far that have not resolved. None is worth a frame.
    let body = use_ref(scope, || None::<loom::NodeHandle>);
    let list_node = use_ref(scope, || None::<loom::NodeHandle>);
    let diff_node = use_ref(scope, || None::<loom::NodeHandle>);
    let resolver = use_ref(scope, Resolver::new);
    // The file the diff pane was showing on the last frame, so that opening
    // another can tell itself from re-reading the same one.
    let shown = use_ref(scope, || None::<File>);

    let list_rows = model.view_lines();
    let list_cursor = list.cursor();
    let list_top = list.top();

    let alignment = reading.content.as_ref().and_then(|c| c.alignment());
    // What is on screen decides the layout, not the state slot: a one-sided
    // file has only the one, so the toggle has nothing to say about it. Its
    // length is its lines, since there is no pairing to lay out.
    let (effective_layout, view_lines_count) = match reading.content.as_deref() {
        Some(pipeline::file::DiffContent::Diff(diff)) => {
            (layout, diff.alignment.view_line_count(layout))
        }
        Some(pipeline::file::DiffContent::SingleFile(single)) => {
            (DiffType::Single, single.lines.len() as u32)
        }
        None => (layout, 0),
    };
    // A walk of every view line, so it is done once per diff rather than once
    // per frame. Change navigation reads it; the status line counts its own.
    let blocks = use_memo(scope, (reading.clone(), layout), || {
        alignment.map(|alignment| alignment.blocks(layout)).unwrap_or_default()
    });

    let has_list = !files.is_empty();
    let has_diff = reading.content.is_some();
    // Which pane the keys and the status line mean. The list is where a
    // reader starts; with nothing beside it, or with no list at all, there is
    // no choice to make.
    let focus_diff = if has_list { on_diff && has_diff } else { true };
    let list_id = PaneId::new(0);
    let diff_id = PaneId::new(if has_list { 1 } else { 0 });

    let (cursor, rows) = if focus_diff {
        (diff.cursor(), view_lines_count)
    } else {
        (list_cursor, list_rows)
    };
    cursor_cell.set(cursor);
    vl_cell.set(rows);
    layout_cell.set(effective_layout);
    *selection_cell.borrow_mut() = selection;

    // Where the cursor would land in the other layout, and how long that
    // layout is. Worked out here rather than in the key handler, which cannot
    // borrow the alignment: view line 40 side by side is a different line
    // inline, so the number cannot be carried across — the file line can.
    let flipped = alignment.and_then(|alignment| {
        let (version, line) = alignment.line_at(layout, diff.cursor())?;
        let landing = alignment.view_line_at(layout.other(), version, line)?;
        Some((landing, alignment.view_line_count(layout.other())))
    });

    // A new list from the store. The reader keeps their place by name, since a
    // row number means nothing across a rebuild (D54).
    let landed = Rc::clone(&model);
    use_layout_effect(scope, files.clone(), move || {
        let landing = landed.line_after(&before, list_cursor);
        let rows = landed.view_lines();
        set_list(&move |mut viewport: Viewport| {
            viewport.place(landing, rows);
            viewport
        });
    });

    // A comparison arrived. A different file starts at its own top; the same
    // one re-read keeps the reader's place, clamped in case it grew shorter.
    // Told apart by the store's version rather than by the file, since
    // re-reading a file gives back the same name and the same revisions.
    let arrived = reading.content.as_ref().map(|content| content.file().clone());
    use_layout_effect(scope, reading.version, move || {
        let same = *shown.current() == arrived;
        *shown.current() = arrived;
        set_selection(&|_| None);
        set_diff(&move |viewport: Viewport| {
            if same {
                let mut kept = viewport;
                let at = kept.cursor().min(view_lines_count.saturating_sub(1));
                kept.place(at, view_lines_count);
                kept
            } else {
                rewound(&viewport)
            }
        });
    });

    // Layout knows how many rows the panes have; the render body does not.
    // Both viewports are told as soon as layout has decided, so a page motion
    // agrees with what is on screen — and so is the screen map, which is
    // cleared here before the diff screens append their own text areas.
    use_layout_effect(scope, loom::Always, move || {
        let area = body.current().map_or(ratatui::layout::Rect::ZERO, |node| node.area());
        let rows = u32::from(area.height);
        set_list(&move |mut viewport: Viewport| {
            viewport.set_height(rows, list_rows);
            viewport
        });
        set_diff(&move |mut viewport: Viewport| {
            viewport.set_height(rows, view_lines_count);
            viewport
        });

        let mut map = map_cell.borrow_mut();
        map.clear();
        map.body = area;
        for (slot, id) in [(list_node, list_id), (diff_node, diff_id)] {
            if let Some(node) = *slot.current() {
                map.panes.push((id, node.area()));
            }
        }
    });

    // Folding a row, and the cursor named again afterwards because the rows
    // it counted have moved. Answers whether there was anything to fold.
    let folding_model = Rc::clone(&model);
    let folding_files = files.clone();
    let fold: Rc<dyn Fn(u32) -> bool> = Rc::new(move |line: u32| {
        let Some(next) = folding_model.folded(line) else {
            return false;
        };
        let rows = Model::arranged(folding_files.to_vec(), &next).view_lines();
        set_arrangement(&move |_| next.clone());
        set_list(&move |mut viewport: Viewport| {
            let at = viewport.cursor().min(rows.saturating_sub(1));
            viewport.place(at, rows);
            viewport
        });
        true
    });

    // Opening a row, whether the gesture was Enter or a click. A heading and
    // a directory have nothing to open, so they fold instead.
    let open_row: Rc<dyn Fn(u32)> = {
        let folding = Rc::clone(&fold);
        let request = Rc::clone(&on_open);
        let held = Rc::clone(&model);
        Rc::new(move |line: u32| {
            if folding(line) {
                return;
            }
            if let Some(file) = held.file(line).cloned() {
                request(file);
            }
        })
    };

    // The list has the keys while the reader is in it, because someone
    // looking at the list is choosing a file rather than reading one.
    let keymap_type = if focus_diff {
        KeymapType::File(effective_layout)
    } else {
        KeymapType::Explorer
    };

    let jumps = Rc::clone(&blocks);
    let opening = Rc::clone(&open_row);
    let folding = Rc::clone(&fold);
    let flipping_model = Rc::clone(&model);
    let flipping_files = files.clone();
    let keys = Listeners::new().on_key(move |key| {
        // Both answered the key before this one, and neither survives it.
        set_notice(&|_| None);
        set_exhausted(&|_| None);

        let resolution = resolver.current().key(key, keymap_type);
        let Resolution::Run(command) = resolution else {
            // Half a sequence, or a count, is still this component's; a key
            // nothing is bound to belongs to whoever is above.
            return match resolution {
                Resolution::Unbound => Bubble::Continue,
                _ => Bubble::Stop,
            };
        };

        let count = command.repeat();
        // Nowhere to go is an answer, and the status line is where it is
        // given, so whether the jump moved has to come back out.
        let step = |direction: Direction| {
            let moved = Cell::new(false);
            set_diff(&|mut viewport: Viewport| {
                let starts = || jumps.iter().map(|block| block.start);
                let went = viewport.jump_to(count, view_lines_count, |from| match direction {
                    Direction::Next => starts().find(|&start| start > from),
                    Direction::Previous => starts().rev().find(|&start| start < from),
                });
                moved.set(went);
                viewport
            });
            if !moved.get() {
                set_exhausted(&|_| Some(direction));
            }
        };

        match command.action {
            Action::Buffer(BufferAction::Motion(motion)) => {
                if focus_diff {
                    set_diff(&|mut viewport: Viewport| {
                        viewport.motion(motion, count, view_lines_count);
                        viewport
                    });
                } else {
                    set_list(&|mut viewport: Viewport| {
                        viewport.motion(motion, count, list_rows);
                        viewport
                    });
                }
            }
            Action::Buffer(BufferAction::NextChange) => step(Direction::Next),
            Action::Buffer(BufferAction::PrevChange) => step(Direction::Previous),
            // The list's own keys. A fold and a change of arrangement both
            // renumber the rows, so the cursor is named again afterwards.
            Action::Buffer(BufferAction::Toggle) => {
                folding(list_cursor);
            }
            Action::Buffer(BufferAction::ToggleViewMode) => {
                let next = flipping_model.arrangement().other_mode();
                let flipped = Model::arranged(flipping_files.to_vec(), &next);
                let landing = flipped.line_after(&flipping_model, list_cursor);
                let rows = flipped.view_lines();
                set_arrangement(&move |_| next.clone());
                set_list(&move |mut viewport: Viewport| {
                    viewport.place(landing, rows);
                    viewport
                });
            }
            Action::View(ViewAction::Open) => opening(list_cursor),
            // A one-sided file has no other layout to go to, and the cursor
            // travels by file line rather than by view line.
            Action::View(ViewAction::ToggleLayout) => {
                set_layout(&|layout: DiffType| layout.other());
                set_selection(&|_| None);
                if let Some((landing, lines)) = flipped {
                    set_diff(&move |mut viewport: Viewport| {
                        viewport.place(landing, lines);
                        viewport
                    });
                }
            }
            Action::View(ViewAction::ToggleSyntax) => set_syntax_on(&|on: bool| !on),
            // With one pane there is nowhere else for the focus to go, and no
            // border between panes to move.
            Action::Tab(TabAction::FocusNext | TabAction::FocusPrev) => {
                if has_list && has_diff {
                    set_on_diff(&|on: bool| !on);
                }
            }
            Action::Tab(TabAction::WidenLeft | TabAction::NarrowLeft) => {}
            Action::Pane(action) => match action {},
            Action::Program(ProgramAction::Quit) => on_flow(Flow::Quit),
            Action::Program(ProgramAction::Suspend) => on_flow(Flow::Suspend),
            #[cfg(debug_assertions)]
            Action::Program(ProgramAction::Rebuild) => on_flow(Flow::Rebuild),
        }
        Bubble::Stop
    });

    // The wheel turns whatever is under the pointer, which need not be what
    // has focus, and a press in the list chooses the row it landed on.
    let opening = Rc::clone(&open_row);
    let list_keys = Listeners::new()
        .on_wheel(move |delta| {
            set_list(&|mut viewport: Viewport| {
                viewport.scroll(delta * WHEEL, list_rows);
                viewport
            });
            Bubble::Stop
        })
        .on_mouse_down(move |mouse| {
            let line = list_top + u32::from(mouse.local.y);
            if line < list_rows {
                set_on_diff(&|_| false);
                set_list(&move |mut viewport: Viewport| {
                    viewport.place(line, list_rows);
                    viewport
                });
                opening(line);
            }
            Bubble::Stop
        });

    let diff_keys = Listeners::new()
        .on_wheel(move |delta| {
            set_diff(&|mut viewport: Viewport| {
                viewport.scroll(delta * WHEEL, view_lines_count);
                viewport
            });
            Bubble::Stop
        })
        // The text columns take the press first and let it through, so a
        // click anywhere in the diff moves the focus into it.
        .on_mouse_down(move |_| {
            set_on_diff(&|_| true);
            Bubble::Stop
        });

    let on_select: Rc<dyn Fn(Option<Selection>)> =
        Rc::new(move |held| set_selection(&move |_| held));

    // One pane's providers: each is looking at its own document, so each gets
    // its own rather than one set for the whole tree.
    let list_pane = |alone: bool| {
        let layout = if has_diff && !alone {
            Layout { basis: Basis::Length(LIST_WIDTH), min_width: MIN_LIST, ..Default::default() }
        } else {
            Layout { grow: 1, min_width: MIN_LIST, ..Default::default() }
        };
        rsx! {
            Row {
                ref: Some(list_node),
                layout: layout,
                listeners: list_keys.clone(),
                ..,
                PaneContext {
                    value: Some(list_id),
                    ViewLinesContext {
                        value: list.visible(list_rows),
                        CursorContext {
                            value: list_cursor,
                            ArrangementContext {
                                value: arrangement.clone(),
                                Explorer { on_open: Rc::clone(&on_open) }
                            }
                        }
                    }
                }
            }
        }
    };

    let diff_pane = || {
        rsx! {
            Row {
                ref: Some(diff_node),
                layout: Layout { grow: 1, min_width: MIN_DIFF, ..Default::default() },
                listeners: diff_keys.clone(),
                ..,
                PaneContext {
                    value: Some(diff_id),
                    ViewLinesContext {
                        value: diff.visible(view_lines_count),
                        CursorContext {
                            value: diff.cursor(),
                            FirstCellContext {
                                value: diff.left(),
                                SelectionContext {
                                    value: selection,
                                    OnSelectContext {
                                        value: Rc::clone(&on_select),
                                        match effective_layout {
                                            DiffType::SideBySide => { SideBySide {} }
                                            DiffType::Inline => { Inline {} }
                                            DiffType::Single => { SingleFile {} }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    };

    let mut panes: Vec<Node> = Vec::new();
    if has_list {
        panes.push(list_pane(false));
    }
    if has_list && has_diff {
        // The list keeps a divider beside it, so the two never touch.
        panes.push(rsx! {
            Divider {
                layout: Layout { basis: Basis::Length(1), shrink: 0, ..Default::default() },
                symbol: "│",
                style: theme.normal.patch(theme.divider),
                ..
            }
        });
    }
    if has_diff {
        panes.push(diff_pane());
    }

    // Whether a diff fits beside the list depends on how wide its line
    // numbers are, which no arithmetic here can know. The only thing that can
    // answer is the attempt, so the fallback is the pane the reader is
    // working in, on its own — better than saying the terminal is too small
    // while the list beside it would have drawn perfectly.
    let alone = (has_list && has_diff)
        .then(|| if focus_diff { diff_pane() } else { list_pane(true) });

    // The status line reads the focused pane, and a list of changed files is
    // not a file: it has no name to show, no changes to count, and no engine
    // that could have given up on it.
    let shown_file = focus_diff
        .then(|| reading.content.as_ref().map(|content| Rc::new(content.file().clone())))
        .flatten();

    rsx! {
        Column {
            listeners: keys,
            // When the minimum does not fit, loom shows this instead of the
            // tree.
            too_small: Some(rsx! { Text { text: "terminal too small".into(), .. } }),
            // What the screen needs is whatever is in it: a list asks for
            // less than a diff, so the panes carry their own minimums.
            layout: Layout { grow: 1, min_height: 2, ..Default::default() },
            ..,
            FileContext {
                value: shown_file,
                NoticeContext {
                    value: notice,
                    SyntaxOnContext {
                        value: syntax_on,
                        Row {
                            ref: Some(body),
                            layout: Layout { grow: 1, ..Default::default() },
                            too_small: alone,
                            ..,
                            { panes }
                        }
                        // The status line counts the document, not the
                        // window onto it.
                        ViewLinesContext {
                            value: 0..rows,
                            CursorContext {
                                value: cursor,
                                // Two things the store cannot answer.
                                LayoutContext {
                                    value: effective_layout,
                                    ExhaustedContext {
                                        value: exhausted,
                                        StatusLine {}
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

/// A position at the top of a new document, keeping the height the last frame
/// measured — the file changed, the screen did not.
fn rewound(previous: &Viewport) -> Viewport {
    let mut fresh = Viewport::new();
    fresh.set_height(previous.height(), 0);
    fresh
}
