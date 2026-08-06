//! The whole screen: the body, and the status line under it.
//!
//! The outermost renderer, and the one place a buffer kind is dispatched on
//! for drawing. Adding a kind is a new arm here and a new file beside this
//! one, and the compiler names the arm that is missing.
//!
//! A tab holds one pane or two, and this walks whichever it has. Everything
//! below is handed a rectangle and knows nothing about how many there are.

use ratatui::buffer::Buffer as Cells;
use ratatui::layout::Rect;

use crate::draw::{Look, explorer, inline, side_by_side, single_file, status};
use crate::render::{cells, layout};
use crate::syntax::Store;
use crate::theme::Theme;
use crate::view::Buffer;
use crate::view::buffer::BufferType;
use crate::view::{Layout, PaneId, View, Viewport};

/// Renders the whole interface into the terminal's cell grid.
///
/// `view` is taken by mutable reference for one reason: the frame is where a
/// pane's height becomes known, and page motions need it. A terminal resize
/// therefore needs no event of its own — the next frame simply has a different
/// height, and the viewport re-examines itself when told.
pub fn render(
    cells: &mut Cells,
    area: Rect,
    view: &mut View,
    theme: &Theme,
    store: &Store,
    notice: Option<&str>,
) {
    let look = Look {
        theme,
        syntax: view.syntax(),
        store,
    };
    let Some((body, status_area)) = layout::screen(area) else {
        return too_small(cells, area, theme);
    };

    if !body_of(cells, body, view, look) {
        return too_small(cells, area, theme);
    }

    let (buffer, viewport) = view.focused_mut();
    status::draw(
        cells,
        status_area,
        &summary(buffer, viewport, notice),
        theme,
    );
}

/// Draws every pane the tab has, in its own rectangle.
///
/// The split is refused rather than squeezed when the screen cannot hold both,
/// and the focused pane gets the whole body instead — a diff twelve columns
/// wide would be worse than a list the reader can close.
fn body_of(cells: &mut Cells, body: Rect, view: &mut View, look: Look<'_>) -> bool {
    let places = match view.tab().layout() {
        Layout::Split { left } => layout::split(body, left),
        Layout::Full => None,
    };
    let Some((left_area, border, right_area)) = places else {
        return one(cells, body, view, view.tab().focus(), look);
    };

    // A row at a time: `hatch` draws one row, and handing it a full-height
    // rectangle drew the border only on the top line — as `side_by_side` does
    // for the divider between its own two columns.
    let style = look.theme.normal.patch(look.theme.divider);
    for y in border.y..border.bottom() {
        cells::hatch(
            cells,
            Rect {
                y,
                height: 1,
                ..border
            },
            "│",
            style,
        );
    }

    // A pane that cannot draw falls back to one pane rather than failing the
    // whole screen. Whether a diff fits depends on how wide its line numbers
    // are, which the rectangle arithmetic cannot know, so the only place that
    // can answer "does this fit" is the attempt — and a screen that says
    // "terminal too small" while the list beside it would have drawn perfectly
    // is worse than the list on its own.
    let panes: Vec<PaneId> = view.tab().ids().collect();
    let fits = panes.iter().enumerate().all(|(index, &id)| {
        let rect = if index == 0 { left_area } else { right_area };
        one(cells, rect, view, id, look)
    });
    if fits {
        return true;
    }
    // Nothing is cleared first: every renderer fills the rows it is given
    // edge to edge, so the second attempt covers the border and whatever the
    // first attempt had drawn. Clearing here as well was measurably
    // redundant — removing it changed no test and no frame.
    one(cells, body, view, view.tab().focus(), look)
}

/// Draws one pane of the tab into `rect`.
fn one(cells: &mut Cells, rect: Rect, view: &mut View, id: PaneId, look: Look<'_>) -> bool {
    let focused = view.tab().focus() == id;
    let (buffer, viewport) = view.pane_mut(id);
    viewport.set_height(u32::from(rect.height), buffer.view_lines());
    pane(cells, rect, buffer, viewport, look, focused)
}

/// Draws one pane's buffer, whatever kind it is.
///
/// The only place a buffer kind is examined for drawing. Returns `false` if
/// the pane is too small to draw meaningfully.
fn pane(
    cells: &mut Cells,
    area: Rect,
    buffer: &Buffer,
    viewport: &Viewport,
    look: Look<'_>,
    focused: bool,
) -> bool {
    // The one place a buffer kind decides which renderer runs. Side by side
    // and inline are separate variants, so the layout needs no field of its
    // own to be read here.
    match buffer.buffer_type() {
        BufferType::SideBySide(data) => {
            side_by_side::draw(cells, area, buffer, data, viewport, look)
        }
        BufferType::Inline(data) => inline::draw(cells, area, buffer, data, viewport, look),
        BufferType::SingleFile(data) => single_file::draw(cells, area, data, viewport, look),
        BufferType::Explorer(data) => {
            explorer::draw(cells, area, data, viewport, look.theme, focused)
        }
    }
}

/// What the status line says about the focused pane.
fn summary<'a>(
    buffer: &'a Buffer,
    viewport: &Viewport,
    notice: Option<&'a str>,
) -> status::Status<'a> {
    // Every field comes off the parent, whatever kind it is: a lone file
    // reports no changes because it has none, not because this asked a
    // different question of it.
    status::Status {
        file: buffer.file(),
        view_line: viewport.cursor(),
        view_lines: buffer.view_lines(),
        changes: buffer.blocks().len(),
        change: buffer.block_at(viewport.cursor()),
        timed_out: buffer.hit_timeout(),
        exhausted: buffer.exhausted(),
        notice,
    }
}

fn too_small(cells: &mut Cells, area: Rect, theme: &Theme) {
    let style = theme.normal;
    for y in area.y..area.bottom() {
        cells::fill(
            cells,
            Rect {
                y,
                height: 1,
                ..area
            },
            style,
        );
    }
    if area.height > 0 {
        cells::write(
            cells,
            Rect {
                y: area.y,
                height: 1,
                ..area
            },
            0,
            "terminal too small",
            style,
        );
    }
}
