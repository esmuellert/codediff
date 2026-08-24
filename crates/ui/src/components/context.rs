//! Shared state, provided at the root.
//!
//! Screens and the status line read these with `use_context`. Bricks read
//! only `theme` and take per-row values as props.

use std::rc::Rc;

use file_types::File;
use loom::context;

use crate::theme::Theme;

context!(
    /// Colours and styles for every component.
    pub ThemeContext: Rc<Theme> = Rc::new(Theme::DARK),
    |a: &Rc<Theme>, b: &Rc<Theme>| Rc::ptr_eq(a, b)
);

context!(
    /// The repository path.
    pub RepoContext: Option<Rc<std::path::Path>> = None
);

context!(
    /// The focused file, or `None` in the explorer.
    pub FileContext: Option<Rc<File>> = None
);

context!(
    /// Which rows to render.
    pub ViewLinesContext: std::ops::Range<u32> = 0..0
);

context!(
    /// Which row the cursor is on.
    pub CursorContext: u32 = 0
);

context!(
    /// Horizontal scroll offset in cells.
    pub FirstCellContext: u32 = 0
);

context!(
    /// An error or warning to display.
    pub NoticeContext: Option<Rc<str>> = None
);

context!(
    /// What the syntax worker has coloured so far.
    pub ColoursContext: Rc<std::cell::RefCell<syntax::Store>> =
        Rc::new(std::cell::RefCell::new(syntax::Store::new())),
    |a: &Rc<std::cell::RefCell<syntax::Store>>, b: &Rc<std::cell::RefCell<syntax::Store>>| {
        Rc::ptr_eq(a, b)
    }
);

context!(
    /// Whether code is coloured by its language.
    pub SyntaxOnContext: bool = true
);

context!(
    /// Where each pane and text column landed, for whoever has to say what is
    /// under the mouse. Filled by the screens as layout decides.
    pub ScreenMapContext: Rc<std::cell::RefCell<crate::screen_map::ScreenMap>> =
        Rc::new(std::cell::RefCell::new(crate::screen_map::ScreenMap::default())),
    |a: &Rc<std::cell::RefCell<crate::screen_map::ScreenMap>>,
     b: &Rc<std::cell::RefCell<crate::screen_map::ScreenMap>>| Rc::ptr_eq(a, b)
);

context!(
    /// Which pane the component belongs to.
    pub PaneContext: Option<crate::state::PaneId> = None
);
