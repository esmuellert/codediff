//! The list of changed files.
//!
//! ```text
//! mod.rs     state and key handling
//! group.rs   which comparison a file is in
//! style.rs   tree or flat arrangement
//! tree.rs    the nested arrangement — directories and folds
//! list.rs    the flat arrangement — paths only
//! order.rs   sort order
//! filter.rs  glob filtering
//! ```
//!
//! A group is a revision pair (e.g. "Staged Changes" = index vs commit).
//! An arrangement is handed one group's files and knows nothing about groups.
//! Drawing (characters, colours) is in `draw::buffer::explorer`.

mod filter;
mod group;
mod list;
mod order;
mod style;
mod tree;

pub use group::Group;
pub use list::List;
pub use style::Style;
pub use tree::{Node, NodeId, Tree};

use file_types::{File, Revs, Stats};

/// Which arrangement the reader has chosen.
///
/// Their choice rather than a setting read from anywhere: there is no config
/// yet, so this starts nested and only the key bound to
/// [`BufferAction::ToggleViewMode`] changes it. A default read from a file
/// would arrive as an argument to [`Explorer::new`], the way `--theme` already
/// reaches the interface, and nothing else here would move.
///
/// [`BufferAction::ToggleViewMode`]: crate::input::BufferAction
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ViewMode {
    List,
    #[default]
    Tree,
}

/// One line of the file list, as facts rather than as text.
///
/// Everything `draw` needs to write a line and nothing about how it looks —
/// `▾`, `│ ` and `+4` are chosen beside the theme that colours them. That is
/// what lets one file draw a line of any arrangement. See D65.
///
/// The counterpart of [`align::ViewLine`], which is one line of a *diff* on
/// the same terms, and named the same because it is the same idea. Two crates
/// naming one idea alike is not the collision D28 removed — that was one idea
/// with two names.
#[derive(Debug)]
pub enum ViewLine<'a> {
    /// A comparison's heading: what it is called, how many files it holds, and
    /// their total. Never produced by an arrangement — always by the explorer.
    Heading {
        name: &'a str,
        files: usize,
        stats: Stats,
    },
    /// A directory, and whether it is open.
    Directory { name: &'a str, open: bool },
    /// One changed file. Its own name in the nested arrangement, and its whole
    /// path in the flat one, where nothing above it says where it is.
    File { name: &'a str, file: &'a File },
}

/// One file, named so it can be found again after the lines are rebuilt.
///
/// The comparison as well as the path, because a file staged and then edited
/// again is listed twice and the two are different diffs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Anchor {
    pub revs: Revs,
    pub path: String,
}

/// How the reader has arranged the list: nested or flat, and which rows are
/// shut.
///
/// Held apart from the list itself so it can be put on another built from the
/// same files. That is what lets the list be worked out from the file list and
/// this, rather than kept somewhere and changed in place — the component that
/// draws it and the one that holds the keys each build their own, and the two
/// agree because they were given the same two things.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Arrangement {
    mode: ViewMode,
    /// One per group, in the order the groups are in: whether the heading is
    /// open, and which of the group's nodes are shut.
    groups: Vec<(bool, Vec<NodeId>)>,
}

impl Arrangement {
    /// The same list, flat if it was nested and nested if it was flat.
    ///
    /// The folds do not come with it: a node means nothing once the
    /// arrangement it belonged to has been rebuilt.
    pub fn other_mode(&self) -> Self {
        Self {
            mode: match self.mode {
                ViewMode::Tree => ViewMode::List,
                ViewMode::List => ViewMode::Tree,
            },
            groups: self
                .groups
                .iter()
                .map(|&(open, _)| (open, Vec::new()))
                .collect(),
        }
    }
}

/// The list of changed files, as the reader is looking at it.
#[derive(Debug)]
pub struct Explorer {
    /// Every file, whatever a pattern is hiding. What a cleared pattern
    /// brings back, and so never narrowed in place.
    files: Vec<File>,
    /// The files the groups were built from — everything, or what a pattern
    /// left. Kept apart from [`Self::files`] because an arrangement holds
    /// places in *this* list, and narrowing the other one would silently
    /// renumber them.
    shown: Vec<File>,
    groups: Vec<Group>,
    mode: ViewMode,
    /// Whether the line counts are shown. A drawing choice, remembered here
    /// because `draw` holds no state.
    stats_shown: bool,
    /// What is hiding files, as the reader typed it.
    pattern: Option<String>,
}

impl Explorer {
    /// Builds the explorer over a set of files, everything open.
    pub fn new(files: Vec<File>) -> Self {
        let mut explorer = Self {
            files,
            shown: Vec::new(),
            groups: Vec::new(),
            mode: ViewMode::default(),
            stats_shown: true,
            pattern: None,
        };
        explorer.reshape();
        explorer
    }

    /// The list those files make, arranged the way the reader has asked.
    pub fn arranged(files: Vec<File>, arrangement: &Arrangement) -> Self {
        let mut explorer = Self::new(files);
        explorer.arrange(arrangement);
        explorer
    }

    /// How many lines the reader can scroll over, which is what every motion
    /// is clamped against.
    ///
    /// One heading each, plus what each open group's arrangement produced.
    pub fn view_lines(&self) -> u32 {
        group::view_lines(&self.groups)
    }

    pub fn is_empty(&self) -> bool {
        self.groups.is_empty()
    }

    pub fn mode(&self) -> ViewMode {
        self.mode
    }

    pub fn stats_shown(&self) -> bool {
        self.stats_shown
    }

    pub fn pattern(&self) -> Option<&str> {
        self.pattern.as_deref()
    }

    /// What is on a line, or `None` past the end.
    pub fn view_line(&self, line: u32) -> Option<ViewLine<'_>> {
        if let Some(index) = group::get_heading_line(&self.groups, line) {
            let group = &self.groups[index];
            return Some(ViewLine::Heading {
                name: group.heading,
                files: group.files.len(),
                stats: if self.stats_shown {
                    group
                        .files
                        .iter()
                        .map(|&index| self.shown[index].get_stats().unwrap_or_default())
                        .sum()
                } else {
                    Stats::default()
                },
            });
        }
        let (group, line) = group::get_line_style(&self.groups, line)?;
        self.groups[group].style.view_line(line, &self.shown)
    }

    /// The nested arrangement a line belongs to, and which node it is.
    ///
    /// For whatever draws the indent, which is a question only this
    /// arrangement can answer — a flat list has no ancestors to describe.
    pub fn nested_at(&self, line: u32) -> Option<(&Tree, NodeId)> {
        let (group, line) = group::get_line_style(&self.groups, line)?;
        let Style::Tree(tree) = &self.groups[group].style else {
            return None;
        };
        Some((tree, *tree.view_lines().get(line)?))
    }

    /// The file under the cursor, or `None` on a heading or a directory.
    pub fn file(&self, cursor: u32) -> Option<&File> {
        let (group, line) = group::get_line_style(&self.groups, cursor)?;
        self.shown.get(self.groups[group].style.file_on(line)?)
    }

    /// Opens or shuts whatever the cursor is on.
    ///
    /// Returns whether it did, so a key bound to both this and opening a file
    /// can tell which of the two happened. A heading folds in every
    /// arrangement, because a heading is not part of one.
    pub fn toggle(&mut self, cursor: u32) -> bool {
        if let Some(index) = group::get_heading_line(&self.groups, cursor) {
            self.groups[index].open = !self.groups[index].open;
            return true;
        }
        match group::get_line_style(&self.groups, cursor) {
            Some((group, line)) => self.groups[group].style.toggle(line),
            None => false,
        }
    }

    /// The first line a reader can do anything with.
    ///
    /// Line zero is a heading, which can be folded but not opened, so starting
    /// there would mean the first key press did nothing. See D48.
    pub fn first_file(&self) -> u32 {
        (0..self.view_lines())
            .find(|&line| self.file(line).is_some())
            .unwrap_or(0)
    }

    /// Reshapes the list, keeping the reader on the file they were on.
    ///
    /// Returns the line to land on. A line number means nothing across a
    /// rebuild — the view mode renumbers every line — so the file is named
    /// before and looked up after. A file that is no longer listed leaves the
    /// cursor where it was, clamped. See D54.
    pub fn reshape_around(&mut self, cursor: u32, change: impl FnOnce(&mut Self)) -> u32 {
        let anchor = self.anchor(cursor);
        change(self);
        let landing = anchor
            .and_then(|anchor| self.line_of(&anchor))
            .unwrap_or(cursor);
        landing.min(self.view_lines().saturating_sub(1))
    }

    /// Where the file on `line` of `previous` is in this list.
    ///
    /// The same as [`Self::reshape_around`] for a list that is built afresh
    /// rather than changed in place: the file is named in the old one and
    /// looked up in this one, because a line number means nothing across a
    /// rebuild. See D54.
    pub fn line_after(&self, previous: &Self, line: u32) -> u32 {
        previous
            .anchor(line)
            .and_then(|anchor| self.line_of(&anchor))
            .unwrap_or(line)
            .min(self.view_lines().saturating_sub(1))
    }

    /// How the reader has arranged the list, in terms another explorer over
    /// the same files can be given.
    pub fn arrangement(&self) -> Arrangement {
        Arrangement {
            mode: self.mode,
            groups: self
                .groups
                .iter()
                .map(|group| (group.open, group.style.closed()))
                .collect(),
        }
    }

    /// Arranges this list the way `arrangement` describes.
    ///
    /// A group the arrangement says nothing about is left as it was built,
    /// which is open: an arrangement named over one set of files is put on
    /// whatever the next set produced.
    pub fn arrange(&mut self, arrangement: &Arrangement) {
        self.set_mode(arrangement.mode);
        for (group, (open, closed)) in self.groups.iter_mut().zip(&arrangement.groups) {
            group.open = *open;
            group.style.set_closed(closed);
        }
    }

    /// The arrangement with whatever is on `line` shut if it was open and
    /// opened if it was shut, or `None` when there is nothing there to fold.
    ///
    /// [`Self::toggle`] for a list that is worked out rather than kept: the
    /// answer is what to arrange it by next.
    pub fn folded(&self, line: u32) -> Option<Arrangement> {
        let mut arrangement = self.arrangement();
        if let Some(index) = group::get_heading_line(&self.groups, line) {
            let open = &mut arrangement.groups[index].0;
            *open = !*open;
            return Some(arrangement);
        }
        let (group, line) = group::get_line_style(&self.groups, line)?;
        let id = self.groups[group].style.foldable_on(line)?;
        let closed = &mut arrangement.groups[group].1;
        match closed.iter().position(|&shut| shut == id) {
            Some(at) => {
                closed.remove(at);
            }
            None => closed.push(id),
        }
        Some(arrangement)
    }

    pub fn set_mode(&mut self, mode: ViewMode) {
        self.mode = mode;
        self.reshape();
    }

    pub fn toggle_mode(&mut self) {
        self.set_mode(match self.mode {
            ViewMode::Tree => ViewMode::List,
            ViewMode::List => ViewMode::Tree,
        });
    }

    pub fn set_stats(&mut self, shown: bool) {
        self.stats_shown = shown;
    }

    pub fn toggle_stats(&mut self) {
        self.set_stats(!self.stats_shown);
    }

    /// Hides every file whose path does not match a glob.
    ///
    /// `None` shows everything again.
    pub fn set_pattern(&mut self, pattern: Option<String>) {
        self.pattern = pattern;
        self.reshape();
    }

    /// Replaces the files, keeping how the reader had arranged the view.
    ///
    /// For a refresh: the mode, the pattern and the stats switch survive
    /// because they are the reader's choices, while the folds do not, because
    /// a node means nothing once the arrangement has been rebuilt from
    /// different files.
    pub fn refresh(&mut self, files: Vec<File>) {
        self.files = files;
        self.reshape();
    }

    /// What the reader is looking at, in terms that survive a rebuild.
    fn anchor(&self, line: u32) -> Option<Anchor> {
        let file = self.file(line)?;
        Some(Anchor {
            revs: file.revs(),
            path: file.path().as_str().to_owned(),
        })
    }

    /// Where that file is now, if it is still listed.
    fn line_of(&self, anchor: &Anchor) -> Option<u32> {
        (0..self.view_lines()).find(|&line| {
            self.file(line).is_some_and(|file| {
                file.revs() == anchor.revs && file.path().as_str() == anchor.path
            })
        })
    }

    /// Rebuilds every group, which is what any change to *which lines exist*
    /// has to come through: a fold lives inside an arrangement and does not
    /// survive it.
    fn reshape(&mut self) {
        let kept: Vec<File> = match &self.pattern {
            None => self.files.clone(),
            Some(pattern) => self
                .files
                .iter()
                .filter(|file| filter::matches(pattern, file.path().as_str()))
                .cloned()
                .collect(),
        };
        let mode = self.mode;
        self.groups = group::from_files(&kept)
            .into_iter()
            .map(|(heading, files)| group::Group {
                heading,
                style: match mode {
                    ViewMode::Tree => Style::Tree(Tree::build(&kept, &files)),
                    ViewMode::List => Style::List(List::build(&kept, &files)),
                },
                files,
                open: true,
            })
            .collect();
        self.shown = kept;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use file_types::{File, Oid, RepoPath, Rev};
    use std::path::Path;

    fn at(path: &str, revs: Revs) -> File {
        File::unchanged_path(RepoPath::new(path, Path::new("/repo")), revs)
    }

    fn unstaged() -> Revs {
        Revs::new(Rev::Index, Rev::Worktree)
    }

    fn staged() -> Revs {
        Revs::new(Rev::Commit(Oid::new("b87b24c")), Rev::Index)
    }

    #[test]
    fn a_heading_folds_in_either_arrangement() {
        // The fold belongs to the group, so it cannot depend on how the files
        // under it happen to be laid out.
        for mode in [ViewMode::Tree, ViewMode::List] {
            let mut explorer = Explorer::new(vec![at("a.rs", unstaged()), at("b.rs", unstaged())]);
            explorer.set_mode(mode);
            assert_eq!(explorer.view_lines(), 3, "{mode:?}");
            assert!(explorer.toggle(0), "{mode:?}");
            assert_eq!(explorer.view_lines(), 1, "{mode:?}: its files are hidden");
            assert!(explorer.toggle(0), "{mode:?}");
            assert_eq!(explorer.view_lines(), 3, "{mode:?}: and opening restores");
        }
    }

    #[test]
    fn folding_one_heading_leaves_the_other_alone() {
        let mut explorer = Explorer::new(vec![at("a.rs", unstaged()), at("s.rs", staged())]);
        assert_eq!(explorer.view_lines(), 4);
        assert!(explorer.toggle(0));
        assert_eq!(explorer.view_lines(), 3);
        assert!(
            explorer.file(2).is_some(),
            "the staged group still shows its file"
        );
    }

    #[test]
    fn every_line_resolves_to_a_different_place() {
        // The numbering is what stitches the groups together, and getting it
        // wrong does not fail an assertion — it makes two lines the same line.
        // Dropping the heading's own line sends every lookup to a heading, and
        // a scan for the first file then never terminates.
        for mode in [ViewMode::Tree, ViewMode::List] {
            let mut explorer = Explorer::new(vec![
                at("src/a.rs", unstaged()),
                at("b.rs", unstaged()),
                at("s.rs", staged()),
            ]);
            explorer.set_mode(mode);

            let places: Vec<(usize, Option<usize>)> = (0..explorer.view_lines())
                .map(|line| {
                    let heading = group::get_heading_line(&explorer.groups, line);
                    let within = group::get_line_style(&explorer.groups, line);
                    assert!(
                        heading.is_some() != within.is_some(),
                        "{mode:?}: line {line} is a heading or inside one, never both \
                         and never neither"
                    );
                    match heading {
                        Some(group) => (group, None),
                        None => within.map(|(g, l)| (g, Some(l))).expect("a place"),
                    }
                })
                .collect();

            let headings = places.iter().filter(|(_, line)| line.is_none()).count();
            assert_eq!(headings, 2, "{mode:?}: one line each, and no more");

            let mut seen = places.clone();
            seen.sort_unstable();
            seen.dedup();
            assert_eq!(seen.len(), places.len(), "{mode:?}: {places:?} repeats");
        }
    }

    #[test]
    fn a_line_past_the_end_is_nothing_rather_than_a_panic() {
        let explorer = Explorer::new(vec![at("a.rs", unstaged())]);
        assert!(explorer.view_line(explorer.view_lines()).is_none());
        assert!(explorer.file(99).is_none());
    }
}
