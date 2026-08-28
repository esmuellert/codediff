//! The root component: owns all UI state, provides context with values and
//! setters, routes keys.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use file_types::{DiffType, File};
use loom::{
    Basis, Bubble, Column, ColumnProps, Divider, DividerProps, Layout, Listeners, Node, Row,
    RowProps, Scope, Text, TextProps, component, rsx, use_layout_effect, use_memo,
    use_ref, use_state,
};

use super::context::{Context, Ui, UiProps};
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

const LIST_WIDTH: u16 = 40;
const MIN_LIST: u16 = 8;
const MIN_DIFF: u16 = 20;
const WHEEL: i32 = 3;

#[component]
pub fn App(
    scope: &mut Scope,
    theme: Rc<crate::theme::Theme>,
    repo: Option<Rc<std::path::Path>>,
    diff: Option<Rc<pipeline::file::DiffContent>>,
    diff_version: syntax::Version,
    colours: Rc<RefCell<syntax::Store>>,
    files: Rc<Vec<File>>,
    on_open: Option<Rc<dyn Fn(File)>>,
    on_flow: Option<Rc<dyn Fn(Flow)>>,
    read_back: Option<Rc<super::context::ReadBack>>,
) -> Node {
    let (list, set_list) = use_state(scope, Viewport::new);
    let (diff_vp, set_diff) = use_state(scope, Viewport::new);
    let (on_diff, set_on_diff) = use_state(scope, || false);
    let (list_rows, set_list_rows) = use_state(scope, || 0u32);
    let (diff_view_type, set_diff_view_type) = use_state(scope, || DiffType::SideBySide);
    let (notice, set_notice) = use_state(scope, || None::<Rc<str>>);
    let (selection, set_selection) = use_state(scope, || None::<Selection>);
    let (exhausted, set_exhausted) = use_state(scope, || None::<Direction>);

    let body = use_ref(scope, || None::<loom::NodeHandle>);
    let resolver = use_ref(scope, Resolver::new);
    let shown = use_ref(scope, || None::<File>);

    let list_cursor = list.cursor();

    let alignment = diff.as_ref().and_then(|c| c.alignment());
    let (effective_layout, view_lines_count) = match diff.as_deref() {
        Some(pipeline::file::DiffContent::Diff(d)) => {
            (diff_view_type, d.alignment.view_line_count(diff_view_type))
        }
        Some(pipeline::file::DiffContent::SingleFile(single)) => {
            (DiffType::Single, single.lines.len() as u32)
        }
        None => (diff_view_type, 0),
    };
    let blocks = use_memo(scope, (*diff_version, diff_view_type), || {
        alignment.map(|alignment| alignment.blocks(diff_view_type)).unwrap_or_default()
    });

    let has_list = !files.is_empty();
    let has_diff = diff.is_some();
    let focus_diff = if has_list { on_diff && has_diff } else { true };

    let (cursor, rows) = if focus_diff {
        (diff_vp.cursor(), view_lines_count)
    } else {
        (list_cursor, list_rows)
    };

    // A comparison arrived.
    let arrived = diff.as_ref().map(|content| content.file().clone());
    let dv = *diff_version;
    use_layout_effect(scope, dv, move || {
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

    let keymap_type = if focus_diff {
        KeymapType::File(effective_layout)
    } else {
        KeymapType::Explorer
    };

    let flipped = alignment.and_then(|alignment| {
        let (version, line) = alignment.line_at(diff_view_type, diff_vp.cursor())?;
        let landing = alignment.view_line_at(diff_view_type.other(), version, line)?;
        Some((landing, alignment.view_line_count(diff_view_type.other())))
    });

    let jumps = Rc::clone(&blocks);
    let flow = on_flow.clone();
    let keys = Listeners::new().on_key(move |key| {
        set_notice(&|_| None);
        set_exhausted(&|_| None);

        let resolution = resolver.current().key(key, keymap_type);
        let Resolution::Run(command) = resolution else {
            return match resolution {
                Resolution::Unbound => Bubble::Continue,
                _ => Bubble::Stop,
            };
        };

        let count = command.repeat();
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
            Action::Buffer(BufferAction::Toggle | BufferAction::ToggleViewMode) => {}
            Action::View(ViewAction::Open) => {}
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
        .on_mouse_down(move |_| {
            set_on_diff(&|_| true);
            Bubble::Stop
        });

    let open: Rc<dyn Fn(File)> = match on_open {
        Some(open) => Rc::clone(&open),
        None => Rc::new(|_| {}),
    };

    let shown_file = focus_diff
        .then(|| diff.as_ref().map(|content| Rc::new(content.file().clone())))
        .flatten();

    // Write what Session reads after the frame.
    if let Some(rb) = read_back.as_ref() {
        rb.cursor.set(cursor);
        rb.view_lines.set(rows);
        rb.layout.set(effective_layout);
        *rb.selection.borrow_mut() = selection;
    }

    let ctx = Context {
        theme: Rc::clone(theme),
        repo: repo.clone(),
        file: shown_file,
        diff_view_type: effective_layout,
        notice: notice.clone(),
        selection,
        exhausted,
        focus_diff,
        list_cursor,
        list_view_lines: list.visible(list_rows),
        diff_cursor: diff_vp.cursor(),
        diff_view_lines: diff_vp.visible(view_lines_count),
        first_cell: diff_vp.left(),
        diff: diff.clone(),
        diff_version: *diff_version,
        colours: Rc::clone(colours),
        files: Rc::clone(files),
        set_selection: Some(set_selection),
        set_list_rows: Some(set_list_rows),
        set_list_viewport: Some(set_list),
        on_open: on_open.clone(),
        on_flow: on_flow.clone(),
        read_back: read_back.clone(),
    };

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
                Explorer { on_open: Rc::clone(&open) }
            }
        }
    };

    let diff_pane = || {
        rsx! {
            Row {
                layout: Layout { grow: 1, min_width: MIN_DIFF, ..Default::default() },
                listeners: diff_keys.clone(),
                ..,
                match effective_layout {
                    DiffType::SideBySide => { SideBySide {} }
                    DiffType::Inline => { Inline {} }
                    DiffType::Single => { SingleFile {} }
                }
            }
        }
    };

    let mut panes: Vec<Node> = Vec::new();
    if has_list {
        panes.push(list_pane(false));
    }
    if has_list && has_diff {
        panes.push(rsx! {
            Divider {
                layout: Layout { basis: Basis::Length(1), shrink: 0, ..Default::default() },
                symbol: "│",
                style: ctx.theme.normal.patch(ctx.theme.divider),
                ..
            }
        });
    }
    if has_diff {
        panes.push(diff_pane());
    }

    let alone = (has_list && has_diff)
        .then(|| if focus_diff { diff_pane() } else { list_pane(true) });

    rsx! {
        Column {
            listeners: keys,
            too_small: Some(rsx! { Text { text: "terminal too small".into(), .. } }),
            layout: Layout { grow: 1, min_height: 2, ..Default::default() },
            ..,
            Ui {
                value: ctx,
                Row {
                    ref: Some(body),
                    layout: Layout { grow: 1, ..Default::default() },
                    too_small: alone,
                    ..,
                    { panes }
                }
                StatusLine {}
            }
        }
    }
}

fn ask(flow: &Option<Rc<dyn Fn(Flow)>>, what: Flow) {
    if let Some(flow) = flow {
        flow(what);
    }
}

fn rewound(previous: &Viewport) -> Viewport {
    let mut fresh = Viewport::new();
    fresh.set_height(previous.height(), 0);
    fresh
}
