//! What every component reads instead of being handed it.

use std::rc::Rc;

use loom::context;

use crate::theme::Theme;

context!(pub ThemeContext: Rc<Theme> = Rc::new(Theme::DARK), |a: &Rc<Theme>, b: &Rc<Theme>| Rc::ptr_eq(a, b));

/// The diff the open file was read into: its alignment, and the keys the
/// syntax store is filled under.
context!(pub FileContext: Option<Rc<pipeline::file::Diff>> = None, |a: &Option<Rc<pipeline::file::Diff>>, b: &Option<Rc<pipeline::file::Diff>>| {
    match (a, b) {
        (Some(a), Some(b)) => Rc::ptr_eq(a, b),
        (None, None) => true,
        _ => false,
    }
});

/// The path shown in the status line.
context!(pub PathContext: Option<Rc<str>> = None);

/// Which view lines are on screen.
context!(pub ViewLinesContext: std::ops::Range<u32> = 0..0);
/// Which view line the cursor is on.
context!(pub CursorContext: u32 = 0);

/// The horizontal scroll, in cells.
context!(pub FirstCellContext: u32 = 0);

/// What the language says about each line, as far as it has been read.
context!(pub SyntaxContext: Rc<syntax::Store> = Rc::new(syntax::Store::new()), |a: &Rc<syntax::Store>, b: &Rc<syntax::Store>| Rc::ptr_eq(a, b));

/// Whether syntax colouring is on.
context!(pub SyntaxOnContext: bool = true);

/// A message the status line shows instead of the file's name.
context!(pub NoticeContext: Option<Rc<str>> = None);
