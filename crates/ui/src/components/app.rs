//! The root: the explorer beside a diff, with the status line under both.
//!
//! Everything the reader can move is a `use_state` here, offered downwards as
//! one context. Nothing hands this component a model, and nothing below it
//! writes one: a key changes a state slot, and the frame follows from the
//! slots.

use std::cell::Cell;
use std::rc::Rc;

use file_types::{DiffType, File};
use loom::{
    Basis, Bubble, Column, ColumnProps, Divider, DividerProps, Layout, Listeners, Node, Row,
    RowProps, Scope, Text, TextProps, component, rsx, use_context, use_layout_effect, use_memo,
    use_ref, use_state, use_sync_external_store,
};

use super::context::{Context, DiffStoreCtx, FileListStoreCtx, ObservedCtx, Ui, UiProps};
use super::selection::Selection;
use super::{Direction, Viewport};
use super::explorer::{Explorer, ExplorerProps};
use super::{
    Inline, InlineProps, SideBySide, SideBySideProps, SingleFile,
    SingleFileProps, StatusLine, StatusLineProps,
};
use crate::app::Flow;
use crate::input::{
    Action, BufferAction, KeymapType, ProgramAction, Resolution, Resolver, TabAction, ViewAction,
};

/// Columns the list gets once something is open beside it.
///
/// Wide enough for a name, an indent and a status letter.
const LIST_WIDTH: u16 = 40;
/// What each pane needs before the screen gives up on showing both: a list
/// asks for less than a diff.
const MIN_LIST: u16 = 8;
const MIN_DIFF: u16 = 20;
/// View lines one turn of the wheel moves.
const WHEEL: i32 = 3;

/// The whole interface.
///
/// No props: the theme and the repository come down as context, the diff and
/// the file list from the two stores, and the two things only the session can
/// do — stop the loop, reach a worker — are left in `Observed` for this to
/// call.
#[component]
pub fn App(scope: &mut Scope) -> Node {
    // What lasts as long as the session, put here by the root. The rest of
    // the context is filled in below and provided again.
    let session = use_context::<Ui>(scope);
    let observed = use_context::<ObservedCtx>(scope);
    let store = use_context::<DiffStoreCtx>(scope);
    let listing = use_context::<FileListStoreCtx>(scope);
    // The workers fill the stores; this subscribes rather than being handed
    // what they produced.
    let reading = use_sync_external_store(scope, &store);
    let files = use_sync_external_store(scope, &listing);

    // One position per pane. The list and the diff are different documents, so
    // where the reader is in one says nothing about the other.
    let (list, set_list) = use_state(scope, Viewport::new);
    let (diff, set_diff) = use_state(scope, Viewport::new);
    let (on_diff, set_on_diff) = use_state(scope, || false);
    // How long the list turned out to be. The explorer works the rows out
    // from the files and the reader's folds and says how many there were,
    // because the pane holds the viewport and a viewport is clamped against
    // a document it cannot see.
    let (list_rows, set_list_rows) = use_state(scope, || 0u32);

    let (diff_view_type, set_diff_view_type) = use_state(scope, || DiffType::SideBySide);
    let (notice, set_notice) = use_state(scope, || None::<Rc<str>>);
    let (selection, set_selection) = use_state(scope, || None::<Selection>);
    // Which way `]c` or `[c` went with nowhere to go, cleared by the next key.
    let (exhausted, set_exhausted) = use_state(scope, || None::<Direction>);

    // The row of panes, measured after layout; and the keys typed so far that
    // have not resolved. None is worth a frame.
    let body = use_ref(scope, || None::<loom::NodeHandle>);
    let resolver = use_ref(scope, Resolver::new);
    // The file the diff pane was showing on the last frame, so that opening
    // another can tell itself from re-reading the same one.
    let shown = use_ref(scope, || None::<File>);

    let list_cursor = list.cursor();

    let alignment = reading.content.as_ref().and_then(|c| c.alignment());
    // What is on screen decides the layout, not the state slot: a one-sided
    // file has only the one, so the toggle has nothing to say about it. Its
    // length is its lines, since there is no pairing to lay out.
    let (effective_layout, view_lines_count) = match reading.content.as_deref() {
        Some(pipeline::file::DiffContent::Diff(diff)) => {
            (diff_view_type, diff.alignment.view_line_count(diff_view_type))
        }
        Some(pipeline::file::DiffContent::SingleFile(single)) => {
            (DiffType::Single, single.lines.len() as u32)
        }
        None => (diff_view_type, 0),
    };
    // A walk of every view line, so it is done once per diff rather than once
    // per frame. Change navigation reads it; the status line counts its own.
    let blocks = use_memo(scope, (reading.clone(), diff_view_type), || {
        alignment.map(|alignment| alignment.blocks(diff_view_type)).unwrap_or_default()
    });

    let has_list = !files.is_empty();
    let has_diff = reading.content.is_some();
    // Which pane the keys and the status line mean. The list is where a
    // reader starts; with nothing beside it, or with no list at all, there is
    // no choice to make.
    let focus_diff = if has_list { on_diff && has_diff } else { true };

    let (cursor, rows) = if focus_diff {
        (diff.cursor(), view_lines_count)
    } else {
        (list_cursor, list_rows)
    };
    observed.cursor.set(cursor);
    observed.view_lines.set(rows);
    observed.layout.set(effective_layout);
    observed.exhausted.set(exhausted);
    *observed.selection.borrow_mut() = selection;

    // The selection goes down as context and comes back up through this: a
    // screen says what the pointer selected, and the slot here is what every
    // screen then draws from. Left where the screens can reach it, because a
    // screen takes no props.
    *observed.set_selection.borrow_mut() =
        Some(Box::new(move |held: Option<Selection>| set_selection(&move |_| held)));

    // What the list turned out to be, once the explorer has worked it out.
    // A rebuilt list renumbers every row, so it says where the reader now is
    // as well as how many rows there are; a press says the same two things.
    *observed.set_list_cursor.borrow_mut() = Some(Box::new(move |rows: u32, line: u32| {
        set_list_rows(&move |_| rows);
        set_list(&move |mut viewport: Viewport| {
            viewport.place(line, rows);
            viewport
        });
    }));

    // Where the cursor would land in the other layout, and how long that
    // layout is. Worked out here rather than in the key handler, which cannot
    // borrow the alignment: view line 40 side by side is a different line
    // inline, so the number cannot be carried across — the file line can.
    let flipped = alignment.and_then(|alignment| {
        let (version, line) = alignment.line_at(diff_view_type, diff.cursor())?;
        let landing = alignment.view_line_at(diff_view_type.other(), version, line)?;
        Some((landing, alignment.view_line_count(diff_view_type.other())))
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
    // agrees with what is on screen.
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
    });

    // The list has the keys while the reader is in it, because someone
    // looking at the list is choosing a file rather than reading one.
    let keymap_type = if focus_diff {
        KeymapType::File(effective_layout)
    } else {
        KeymapType::Explorer
    };

    let jumps = Rc::clone(&blocks);
    let flow = observed.on_flow.clone();
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
            // The list's own keys, answered by the explorer, which is where
            // the folds and the arrangement live. The key reaches here as
            // well, so whatever a key clears is still cleared.
            Action::Buffer(BufferAction::Toggle | BufferAction::ToggleViewMode) => {}
            Action::View(ViewAction::Open) => {}
            // A one-sided file has no other layout to go to, and the cursor
            // travels by file line rather than by view line.
            Action::View(ViewAction::ToggleLayout) => {
                set_diff_view_type(&|diff_view_type: DiffType| diff_view_type.other());
                set_selection(&|_| None);
                if let Some((landing, lines)) = flipped {
                    set_diff(&move |mut viewport: Viewport| {
                        viewport.place(landing, lines);
                        viewport
                    });
                }
            }
            // With one pane there is nowhere else for the focus to go, and no
            // border between panes to move.
            Action::Tab(TabAction::FocusNext | TabAction::FocusPrev) => {
                if has_list && has_diff {
                    set_on_diff(&|on: bool| !on);
                }
            }
            Action::Tab(TabAction::WidenLeft | TabAction::NarrowLeft) => {}
            Action::Pane(action) => match action {},
            Action::Program(ProgramAction::Quit) => ask(&flow, Flow::Quit),
            Action::Program(ProgramAction::Suspend) => ask(&flow, Flow::Suspend),
            #[cfg(debug_assertions)]
            Action::Program(ProgramAction::Rebuild) => ask(&flow, Flow::Rebuild),
        }
        Bubble::Stop
    });

    // The wheel turns whatever is under the pointer, which need not be what
    // has focus. A press anywhere in the list moves the focus into it, the
    // way a press anywhere in the diff moves it the other way.
    let list_keys = Listeners::new()
        .on_wheel(move |delta| {
            set_list(&|mut viewport: Viewport| {
                viewport.scroll(delta * WHEEL, list_rows);
                viewport
            });
            Bubble::Stop
        })
        .on_mouse_down(move |_| {
            set_on_diff(&|_| false);
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

    // The file the reader chose. Only the session can reach a worker, so it
    // leaves the way to ask in `Observed`; with nothing there, a row opens
    // nothing.
    let open: Rc<dyn Fn(File)> = match &observed.on_open {
        Some(open) => Rc::clone(open),
        None => Rc::new(|_| {}),
    };

    // The status line reads the focused pane, and a list of changed files is
    // not a file: it has no name to show, no changes to count, and no engine
    // that could have given up on it.
    let shown_file = focus_diff
        .then(|| reading.content.as_ref().map(|content| Rc::new(content.file().clone())))
        .flatten();

    // What everything below reads. The rows and the cursor here are the
    // focused pane's *document*, which is what the status line counts; each
    // pane provides its own window onto its own.
    let base = Context {
        theme: Rc::clone(&session.theme),
        repo: session.repo.clone(),
        file: shown_file,
        view_lines: 0..rows,
        cursor,
        first_cell: 0,
        selection,
        notice,
        diff_view_type: effective_layout,
    };

    // One pane's context: each is looking at its own document, so each gets
    // its own rather than one for the whole tree.
    let list_pane = |alone: bool| {
        let layout = if has_diff && !alone {
            Layout { basis: Basis::Length(LIST_WIDTH), min_width: MIN_LIST, ..Default::default() }
        } else {
            Layout { grow: 1, min_width: MIN_LIST, ..Default::default() }
        };
        rsx! {
            Row {
                layout: layout,
                listeners: list_keys.clone(),
                ..,
                Ui {
                    value: Context {
                        view_lines: list.visible(list_rows),
                        cursor: list_cursor,
                        ..base.clone()
                    },
                    Explorer { on_open: Rc::clone(&open) }
                }
            }
        }
    };

    let diff_pane = || {
        rsx! {
            Row {
                layout: Layout { grow: 1, min_width: MIN_DIFF, ..Default::default() },
                listeners: diff_keys.clone(),
                ..,
                Ui {
                    value: Context {
                        view_lines: diff.visible(view_lines_count),
                        cursor: diff.cursor(),
                        first_cell: diff.left(),
                        ..base.clone()
                    },
                    match effective_layout {
                        DiffType::SideBySide => { SideBySide {} }
                        DiffType::Inline => { Inline {} }
                        DiffType::Single => { SingleFile {} }
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
                style: base.theme.normal.patch(base.theme.divider),
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
            Ui {
                value: base.clone(),
                Row {
                    ref: Some(body),
                    layout: Layout { grow: 1, ..Default::default() },
                    too_small: alone,
                    ..,
                    { panes }
                }
                // Everything it needs is in the context and the store.
                StatusLine {}
            }
        }
    }
}

/// Asks the loop to do what only it can. With nothing there — a tree built
/// without a session — the key does nothing.
fn ask(flow: &Option<Rc<dyn Fn(Flow)>>, what: Flow) {
    if let Some(flow) = flow {
        flow(what);
    }
}

/// A position at the top of a new document, keeping the height the last frame
/// measured — the file changed, the screen did not.
fn rewound(previous: &Viewport) -> Viewport {
    let mut fresh = Viewport::new();
    fresh.set_height(previous.height(), 0);
    fresh
}
