//! What the focused buffer can do, and the keys that ask for it.
//!
//! The innermost level of the view model, and so the first consulted: a
//! binding here shadows the same keys at every level above. That is how a
//! buffer kind claims a key without anyone else having to know — `<` narrows
//! a diff's column divider here, and falls through to the tab elsewhere.

use crokey::{KeyCombination, key};

use crate::input::command::Action;
use crate::input::keymap::{Binding, Context};

/// Something the focused buffer does, to itself or to the viewport it is lent.
///
/// Motions live here rather than at a level of their own because they route to
/// exactly the same place. Every buffer kind answers them by delegating to one
/// shared [`Viewport`] helper, so a new kind gets them in a line.
///
/// [`Viewport`]: crate::view::Viewport
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BufferAction {
    /// Generic movement, needing only a row count.
    Motion(Motion),
    /// Move to the next or previous run of changed rows.
    ///
    /// A motion that has to ask the buffer where to go, which is why it is not
    /// a [`Motion`]: those need nothing but a row count.
    NextChange,
    PrevChange,
    /// Drag the divider between a side-by-side buffer's two columns.
    ///
    /// Not a pane boundary — both columns are inside one buffer — so this
    /// belongs to the buffer that draws them, not to the tab. Named for the
    /// column that grows, since that is what the reader is asking for.
    WidenOriginal,
    NarrowOriginal,
}

/// Movement that needs nothing but the number of rows.
///
/// Identical arithmetic for every buffer kind — a diff, a file, a list — which
/// is why it is one enum rather than something each kind reimplements.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Motion {
    Down,
    Up,
    PageDown,
    PageUp,
    /// First row, or the row a count names.
    Top,
    /// Last row, or the row a count names.
    Bottom,
    ScrollLeft,
    ScrollRight,
    /// Back to column zero.
    ScrollHome,
}

/// Cells `ScrollLeft` and `ScrollRight` move, per repeat.
///
/// There is no cursor column in a read-only view, so a single column would be
/// uselessly slow. `5l` scrolls five times this.
pub const SCROLL_STEP: u32 = 4;

/// Percentage points `WidenOriginal` and `NarrowOriginal` move, per repeat.
pub const DIVIDER_STEP: u16 = 5;

/// The buffer-level lists live for one kind of buffer, in order.
///
/// The only level whose bindings depend on anything: which keys a buffer
/// understands is a property of what it holds. Every level above binds the
/// same keys whatever has focus.
pub const fn bindings(context: Context) -> &'static [&'static [Binding]] {
    match context {
        Context::SideBySide => &[MOTIONS, SIDE_BY_SIDE],
        Context::SingleFile => &[MOTIONS],
    }
}

const fn motion(keys: &'static [KeyCombination], motion: Motion) -> Binding {
    Binding {
        keys,
        action: Action::Buffer(BufferAction::Motion(motion)),
    }
}

const fn buffer(keys: &'static [KeyCombination], action: BufferAction) -> Binding {
    Binding {
        keys,
        action: Action::Buffer(action),
    }
}

/// Movement every buffer kind understands, because it needs only a row count.
///
/// A shared list rather than a copy per kind: a kind that forgot the motions
/// would be unscrollable, and a test would have to notice. It is consulted
/// before the kind's own list, so a kind that wants to rebind one still can.
pub const MOTIONS: &[Binding] = &[
    motion(&[key!(j)], Motion::Down),
    motion(&[key!(down)], Motion::Down),
    motion(&[key!(k)], Motion::Up),
    motion(&[key!(up)], Motion::Up),
    motion(&[key!(ctrl - d)], Motion::PageDown),
    motion(&[key!(pagedown)], Motion::PageDown),
    motion(&[key!(ctrl - u)], Motion::PageUp),
    motion(&[key!(pageup)], Motion::PageUp),
    // `g` is deliberately unbound on its own: it is an internal node of the
    // trie, exactly as in vim.
    motion(&[key!(g), key!(g)], Motion::Top),
    motion(&[key!(home)], Motion::Top),
    motion(&[key!(shift - g)], Motion::Bottom),
    motion(&[key!(end)], Motion::Bottom),
    motion(&[key!(h)], Motion::ScrollLeft),
    motion(&[key!(left)], Motion::ScrollLeft),
    motion(&[key!(l)], Motion::ScrollRight),
    motion(&[key!(right)], Motion::ScrollRight),
    // A motion when no count is in progress, a digit when one is — vim's own
    // rule, and the only place counts and bindings interact.
    motion(&[key!('0')], Motion::ScrollHome),
];

/// What a side-by-side diff adds to the motions.
pub const SIDE_BY_SIDE: &[Binding] = &[
    // `]` and `[` are deliberately unbound on their own, like `g`: they are
    // internal nodes of the trie. Vim's own diff-change motions, which also
    // leaves `n` and `N` free for search — see D9.
    buffer(&[key!(']'), key!(c)], BufferAction::NextChange),
    buffer(&[key!('['), key!(c)], BufferAction::PrevChange),
    buffer(&[key!('>')], BufferAction::WidenOriginal),
    buffer(&[key!('<')], BufferAction::NarrowOriginal),
];

// A plain file adds nothing: there are no changes to step through and no
// second column to resize, so those keys are simply not live. That is why they
// cannot become silent no-ops there.
