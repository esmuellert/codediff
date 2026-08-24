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
    CursorContext, CursorContextProps, DiffStoreContext, FileContext, FileContextProps,
    FirstCellContext, FirstCellContextProps, NoticeContext, NoticeContextProps, SyntaxOnContext,
    SyntaxOnContextProps, ThemeContext, ViewLinesContext, ViewLinesContextProps,
};
use super::{
    Explorer, ExplorerProps, Inline, InlineProps, SideBySide, SideBySideProps, SingleFile,
    SingleFileProps, StatusLine, StatusLineProps,
};
use crate::app::Flow;
use crate::input::{
    Action, BufferAction, KeymapType, ProgramAction, Resolution, Resolver, ViewAction,
};
use crate::state::{Direction, Viewport};

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
    let on_flow = use_context::<FlowContext>(scope);
    // The workers fill the store; this subscribes rather than being handed
    // what they produced.
    let reading = use_sync_external_store(scope, &store);

    let (viewport, set_viewport) = use_state(scope, Viewport::new);
    let (layout, set_layout) = use_state(scope, || DiffType::SideBySide);
    let (show_explorer, set_show_explorer) = use_state(scope, || false);
    let (notice, set_notice) = use_state(scope, || None::<Rc<str>>);
    let (file, set_file) = use_state(scope, || None::<Rc<File>>);
    let (syntax_on, set_syntax_on) = use_state(scope, || true);
    // Which way `]c` or `[c` went with nowhere to go, cleared by the next key.
    let (exhausted, set_exhausted) = use_state(scope, || None::<Direction>);

    // The row of panes, measured after layout; and the keys typed so far that
    // have not resolved. Neither is worth a frame of its own.
    let body = use_ref(scope, || None::<loom::NodeHandle>);
    let resolver = use_ref(scope, Resolver::new);

    let alignment = reading.diff.as_ref().map(|diff| &diff.alignment);
    let view_lines_count = alignment.map_or(0, |alignment| alignment.view_line_count(layout));
    let timed_out = alignment.is_some_and(|alignment| alignment.hit_timeout());
    // A walk of every view line, so it is done once per diff rather than once
    // per frame. The status line reads it, and so does change navigation.
    let blocks = use_memo(scope, (reading.clone(), layout), || {
        alignment.map(|alignment| alignment.blocks(layout)).unwrap_or_default()
    });

    let cursor = viewport.cursor();

    // Layout knows how many rows the panes have; the render body does not.
    // The viewport is told as soon as layout has decided, so a page motion
    // agrees with what is on screen.
    use_layout_effect(scope, loom::Always, move || {
        let rows = body.current().map_or(0, |node| u32::from(node.area().height));
        set_viewport(&move |mut viewport: Viewport| {
            viewport.set_height(rows, view_lines_count);
            viewport
        });
    });

    // A file replaces the list. Where the reader was in one file means nothing
    // in another, so the position starts again from the top.
    let on_open: Rc<dyn Fn(File)> = Rc::new(move |file| {
        let file = Rc::new(file);
        set_file(&move |_| Some(Rc::clone(&file)));
        set_show_explorer(&|_| false);
        set_viewport(&|viewport: Viewport| rewound(&viewport));
    });

    // The list has the keys while it is open, because a reader looking at it
    // is choosing a file rather than reading one.
    let keymap_type = if show_explorer {
        KeymapType::Explorer
    } else {
        KeymapType::File(layout)
    };

    let jumps = Rc::clone(&blocks);
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
            set_viewport(&|mut viewport: Viewport| {
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
                set_viewport(&|mut viewport: Viewport| {
                    viewport.motion(motion, count, view_lines_count);
                    viewport
                });
            }
            Action::Buffer(BufferAction::NextChange) => step(Direction::Next),
            Action::Buffer(BufferAction::PrevChange) => step(Direction::Previous),
            // The list owns its folds and its shape, and knows which file a
            // row is, so it answers these itself.
            Action::Buffer(BufferAction::Toggle | BufferAction::ToggleViewMode)
            | Action::View(ViewAction::Open) => {}
            // A one-sided file has no other layout to go to.
            Action::View(ViewAction::ToggleLayout) => set_layout(&|layout: DiffType| layout.other()),
            Action::View(ViewAction::ToggleSyntax) => set_syntax_on(&|on: bool| !on),
            // One pane and one tab, so there is nothing to focus or resize.
            Action::Pane(_) | Action::Tab(_) => {}
            Action::Program(ProgramAction::Quit) => on_flow(Flow::Quit),
            Action::Program(ProgramAction::Suspend) => on_flow(Flow::Suspend),
            #[cfg(debug_assertions)]
            Action::Program(ProgramAction::Rebuild) => on_flow(Flow::Rebuild),
        }
        Bubble::Stop
    });

    // Whether a diff fits beside the list depends on how wide its line
    // numbers are, which no arithmetic here can know. The only thing that can
    // answer is the attempt, so the fallback is the pane the reader is
    // working in, on its own — better than saying the terminal is too small
    // while the list beside it would have drawn perfectly. With the list
    // hidden there is only the diff, and nothing to fall back to.
    let alone = show_explorer.then(|| {
        rsx! { Explorer { on_open: Rc::clone(&on_open) } }
    });

    let panes = rsx! {
        Row {
            ref: Some(body),
            layout: Layout { grow: 1, ..Default::default() },
            too_small: alone,
            ..,
            if show_explorer {
                Explorer { on_open: Rc::clone(&on_open) }
                // The list keeps a divider beside it, so the two never touch.
                Divider {
                    layout: Layout { basis: Basis::Length(1), shrink: 0, ..Default::default() },
                    symbol: "│",
                    style: theme.normal.patch(theme.divider),
                    ..
                }
            }
            match layout {
                DiffType::SideBySide => { SideBySide {} }
                DiffType::Inline => { Inline {} }
                DiffType::Single => { SingleFile {} }
            }
        }
    };

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
                value: file,
                CursorContext {
                    value: cursor,
                    FirstCellContext {
                        value: viewport.left(),
                        NoticeContext {
                            value: notice,
                            SyntaxOnContext {
                                value: syntax_on,
                                ViewLinesContext {
                                    value: viewport.visible(view_lines_count),
                                    { panes }
                                }
                                // The status line counts the document, not
                                // the window onto it.
                                ViewLinesContext {
                                    value: 0..view_lines_count,
                                    StatusLine {
                                        changes: blocks.len(),
                                        change: blocks.iter().position(|b| b.contains(&cursor)),
                                        timed_out: timed_out,
                                        exhausted: exhausted,
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
