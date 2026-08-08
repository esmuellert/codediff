//! Buffer-level actions and their keybindings.
//!
//! The innermost level of the view model, and so the first consulted: a
//! binding here shadows the same keys at every level above. That is how a
//! buffer kind claims a key without anyone else having to know — `<` narrows
//! a diff's column divider here, and falls through to the tab elsewhere.

use crokey::{KeyCombination, key};
use file_types::DiffType;

use crate::input::command::Action;
use crate::input::keymap::{Binding, KeymapType};
use crate::input::view::ViewAction;

/// Something the focused buffer does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BufferAction {
    Motion(Motion),
    NextChange,
    PrevChange,
    /// Switch tree/flat mode in the explorer.
    ToggleViewMode,
    /// Open or fold the selected row in the explorer.
    Toggle,
}

/// Movement that needs only the number of rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Motion {
    Down,
    Up,
    PageDown,
    PageUp,
    /// First view line, or the one a count names.
    Top,
    /// Last row, or the row a count names.
    Bottom,
    ScrollLeft,
    ScrollRight,
}

/// Cells `ScrollLeft` and `ScrollRight` move, per repeat.
///
/// There is no cursor column in a read-only view, so a single column would be
/// uselessly slow. `5l` scrolls five times this.
pub const SCROLL_STEP: u32 = 4;

/// The buffer-level lists live for one kind of buffer, in order.
///
/// The only level whose bindings depend on anything: which keys a buffer
/// understands is a property of what it holds. Every level above binds the
/// same keys whatever has focus.
pub const fn bindings(keymap_type: KeymapType) -> &'static [&'static [Binding]] {
    match keymap_type {
        KeymapType::File(DiffType::SideBySide) => &[MOTIONS, CHANGES, TWO_COLUMNS],
        KeymapType::File(DiffType::Inline) => &[MOTIONS, CHANGES],
        KeymapType::File(DiffType::Single) => &[MOTIONS],
        KeymapType::Explorer => &[MOTIONS, LIST],
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
];

/// What the list of changed files adds to the motions.
///
/// `i` and `<CR>` are the plugin's own keys, so the two tools do not need
/// different fingers for the same thought. The `z` folds are deliberately not
/// here: they turned out to fold only the section the cursor is in, which is
/// not what "close all folds" says, and one key that opens and shuts the row
/// under the cursor is the whole of what a read-only list needs.
pub const LIST: &[Binding] = &[
    Binding {
        keys: &[key!(enter)],
        action: Action::View(ViewAction::Open),
    },
    // `l` opens the selected row — the same as Enter, and the same as vim's
    // netrw. `h` folds it shut if it is foldable.
    Binding {
        keys: &[key!(l)],
        action: Action::View(ViewAction::Open),
    },
    buffer(&[key!(h)], BufferAction::Toggle),
    buffer(&[key!(i)], BufferAction::ToggleViewMode),
];

/// What any diff adds to the motions, however it is laid out.
///
/// Shared by both layouts rather than listed twice, so `]c` cannot come to
/// mean one thing side by side and another inline.
pub const CHANGES: &[Binding] = &[
    // `]` and `[` are deliberately unbound on their own, like `g`: they are
    // internal nodes of the trie. Vim's own diff-change motions, which also
    // leaves `n` and `N` free for search — see D9.
    buffer(&[key!(']'), key!(c)], BufferAction::NextChange),
    buffer(&[key!('['), key!(c)], BufferAction::PrevChange),
    // Horizontal scroll — the diff pane's meaning of h/l. Inline has no
    // TWO_COLUMNS list, so these are here to cover both layouts.
    motion(&[key!(h)], Motion::ScrollLeft),
    motion(&[key!(l)], Motion::ScrollRight),
];

/// What only two columns can offer.
///
/// `h`/`l` are horizontal scroll here — unlike in the explorer, where they
/// mean fold/open — because a diff's lines can be wider than the pane.
pub const TWO_COLUMNS: &[Binding] = &[
    motion(&[key!(h)], Motion::ScrollLeft),
    motion(&[key!(l)], Motion::ScrollRight),
];

// A plain file adds nothing: there are no changes to step through and no
// second column to resize, so those keys are simply not live. That is why they
// cannot become silent no-ops there.
