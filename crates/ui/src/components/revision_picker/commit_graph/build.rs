//! Multi-lane topological commit graph builder with `BranchDrawing` (`U+F5D0..U+F60D`)
//! and `BoxDrawing` (`○`/`◎` + `╭ ╮ ╰ ╯ │ ─`) symbol sets.
//!
//! Translated from LazyGit's `pkg/gui/presentation/graph/graph.go`, `cell.go`, and
//! `branch_drawing.go` (MIT License, Copyright (c) 2018 Jesse Duffield).

use std::cmp::Ordering;
use std::collections::HashSet;
use std::rc::Rc;

use crate::Commit;

/// Number of branch lane colours in the theme palette cycle.
pub const LANE_COLOUR_COUNT: usize = 6;
/// Fallback marker colour index (matches LazyGit's `FgDefault`).
pub const MARKER_COLOUR: u8 = LANE_COLOUR_COUNT as u8;
/// Highlight colour index for pipes originating at the selected commit.
pub const HIGHLIGHT_COLOUR: u8 = (LANE_COLOUR_COUNT + 1) as u8;

const START_HASH: &str = "START";
const BRANCH_DRAWING_HORIZONTAL: &str = "\u{f5d0}";

/// Character set used to render commit graph cells.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SymbolSet {
    #[default]
    BoxDrawing,
    BranchDrawing,
}

/// One resolved two-column cell ready to paint onto a commit entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GraphCell {
    pub first: &'static str,
    pub first_colour: u8,
    pub second: &'static str,
    pub second_colour: u8,
}

impl Default for GraphCell {
    fn default() -> Self {
        Self {
            first: " ",
            first_colour: MARKER_COLOUR,
            second: " ",
            second_colour: MARKER_COLOUR,
        }
    }
}

/// Precomputed topological pipe sets across a slice of commits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Graph {
    lane_count: usize,
    pipe_sets: Vec<Vec<GraphPipe>>,
}

impl Graph {
    /// Number of two-column graph lanes across all commits in this graph.
    pub fn lane_count(&self) -> usize {
        self.lane_count
    }

    /// Resolves glyphs and colour indices for commit `index`, padded to `lane_count`.
    pub fn cells(
        &self,
        index: usize,
        commits: &[Commit],
        selected_oid: Option<&str>,
        symbol_set: SymbolSet,
    ) -> Rc<[GraphCell]> {
        let Some(pipes) = self.pipe_sets.get(index) else {
            return Rc::from([]);
        };
        let prev_commit_oid = index
            .checked_sub(1)
            .and_then(|prev| commits.get(prev))
            .map(|commit| commit.oid.as_str());
        let routed = route_pipe_set(pipes, selected_oid, prev_commit_oid, symbol_set);
        let mut resolved = Vec::with_capacity(self.lane_count);
        for cell in &routed {
            resolved.push(resolve_cell(cell, symbol_set));
        }
        if resolved.len() < self.lane_count {
            resolved.resize(self.lane_count, GraphCell::default());
        }
        Rc::from(resolved)
    }
}

/// Builds topological pipe sets for `commits`.
pub fn graph(commits: &[Commit]) -> Graph {
    if commits.is_empty() {
        return Graph {
            lane_count: 0,
            pipe_sets: Vec::new(),
        };
    }

    let pipe_sets = get_pipe_sets(commits);
    let lane_count = pipe_sets
        .iter()
        .map(|pipes| {
            pipes
                .iter()
                .map(GraphPipe::right)
                .max()
                .map_or(0, |max_pos| max_pos + 1)
        })
        .max()
        .unwrap_or(0);

    Graph {
        lane_count,
        pipe_sets,
    }
}

/// Formats `cells` into a single string with trailing spaces trimmed.
pub fn format_cells(cells: &[GraphCell]) -> String {
    let mut text = String::with_capacity(cells.len() * 6);
    for cell in cells {
        text.push_str(cell.first);
        text.push_str(cell.second);
    }
    text.trim_end().to_owned()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum CellType {
    #[default]
    Connection,
    Commit,
    Merge,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum VerticalLine {
    #[default]
    None,
    Straight,
    ToLeft,
    ToRight,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RoutedCell {
    up: VerticalLine,
    down: VerticalLine,
    left: bool,
    right: bool,
    horizontal: bool,
    horizontal_on_top: bool,
    cell_type: CellType,
    colour: u8,
    right_colour: Option<u8>,
}

impl Default for RoutedCell {
    fn default() -> Self {
        Self {
            up: VerticalLine::None,
            down: VerticalLine::None,
            left: false,
            right: false,
            horizontal: false,
            horizontal_on_top: false,
            cell_type: CellType::Connection,
            colour: MARKER_COLOUR,
            right_colour: None,
        }
    }
}

impl RoutedCell {
    fn reset(&mut self) {
        self.up = VerticalLine::None;
        self.down = VerticalLine::None;
        self.left = false;
        self.right = false;
        self.horizontal = false;
        self.horizontal_on_top = false;
        self.right_colour = None;
    }

    fn set_up(&mut self, colour: u8, line: VerticalLine) {
        self.up = line;
        self.colour = colour;
    }

    fn set_down(&mut self, colour: u8, line: VerticalLine) {
        self.down = line;
        self.colour = colour;
    }

    fn set_left(&mut self, colour: u8) {
        self.left = true;
        if self.up == VerticalLine::None && self.down == VerticalLine::None {
            self.colour = colour;
        }
    }

    fn set_right(&mut self, colour: u8, override_right: bool) {
        self.right = true;
        if self.right_colour.is_none() || override_right {
            self.right_colour = Some(colour);
        }
    }

    fn set_horizontal(&mut self, colour: u8, override_right: bool) {
        self.horizontal = true;
        self.set_left(colour);
        self.set_right(colour, override_right);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum PipeStep {
    Terminates = 0,
    Starts = 1,
    Continues = 2,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct GraphPipe {
    from_hash: String,
    to_hash: Option<String>,
    from_pos: usize,
    to_pos: usize,
    step: PipeStep,
    colour: u8,
}

impl GraphPipe {
    fn left(&self) -> usize {
        self.from_pos.min(self.to_pos)
    }

    fn right(&self) -> usize {
        self.from_pos.max(self.to_pos)
    }
}

fn resolve_cell(cell: &RoutedCell, symbol_set: SymbolSet) -> GraphCell {
    let (first, second) = match symbol_set {
        SymbolSet::BoxDrawing => box_cell_chars(cell),
        SymbolSet::BranchDrawing => branch_cell_chars(cell),
    };
    let second_colour = if second == " " {
        MARKER_COLOUR
    } else {
        cell.right_colour.unwrap_or(cell.colour)
    };
    GraphCell {
        first,
        first_colour: cell.colour,
        second,
        second_colour,
    }
}

fn get_pipe_sets(commits: &[Commit]) -> Vec<Vec<GraphPipe>> {
    if commits.is_empty() {
        return Vec::new();
    }

    let mut next_colour = 0usize;
    let mut allocate_colour = || {
        let colour = (next_colour % LANE_COLOUR_COUNT) as u8;
        next_colour += 1;
        colour
    };

    let mut pipes = vec![GraphPipe {
        from_hash: START_HASH.to_owned(),
        to_hash: Some(commits[0].oid.as_str().to_owned()),
        from_pos: 0,
        to_pos: 0,
        step: PipeStep::Starts,
        colour: allocate_colour(),
    }];

    commits
        .iter()
        .map(|commit| {
            let next = get_next_pipes(&pipes, commit, &mut allocate_colour);
            pipes = next.clone();
            next
        })
        .collect()
}

fn get_next_pipes(
    prev_pipes: &[GraphPipe],
    commit: &Commit,
    allocate_colour: &mut impl FnMut() -> u8,
) -> Vec<GraphPipe> {
    let commit_hash = commit.oid.as_str();

    let current_pipes: Vec<&GraphPipe> = prev_pipes
        .iter()
        .filter(|pipe| pipe.step != PipeStep::Terminates && pipe.to_hash.is_some())
        .collect();

    let mut max_pos: isize = -1;
    for pipe in &current_pipes {
        max_pos = max_pos.max(pipe.to_pos as isize);
    }

    let incoming = current_pipes
        .iter()
        .copied()
        .find(|pipe| pipe.to_hash.as_deref() == Some(commit_hash));
    let pos = incoming.map_or((max_pos + 1) as usize, |pipe| pipe.to_pos);
    let commit_colour = incoming.map_or_else(&mut *allocate_colour, |pipe| pipe.colour);

    let mut taken_spots = HashSet::new();
    let mut traversed_spots = HashSet::new();

    let first_parent = commit.parents.first().map(|oid| oid.as_str().to_owned());
    let mut new_pipes = Vec::with_capacity(current_pipes.len() + commit.parents.len().max(1));
    new_pipes.push(GraphPipe {
        from_hash: commit_hash.to_owned(),
        to_hash: first_parent,
        from_pos: pos,
        to_pos: pos,
        step: PipeStep::Starts,
        colour: commit_colour,
    });

    let mut traversed_for_continuing = HashSet::new();
    for pipe in &current_pipes {
        if pipe.to_hash.as_deref() != Some(commit_hash) {
            traversed_for_continuing.insert(pipe.to_pos);
        }
    }

    let traverse =
        |from: usize, to: usize, traversed: &mut HashSet<usize>, taken: &mut HashSet<usize>| {
            let left = from.min(to);
            let right = from.max(to);
            for spot in left..=right {
                traversed.insert(spot);
            }
            taken.insert(to);
        };

    for pipe in &current_pipes {
        if pipe.to_hash.as_deref() == Some(commit_hash) {
            new_pipes.push(GraphPipe {
                from_hash: pipe.from_hash.clone(),
                to_hash: pipe.to_hash.clone(),
                from_pos: pipe.to_pos,
                to_pos: pos,
                step: PipeStep::Terminates,
                colour: pipe.colour,
            });
            traverse(pipe.to_pos, pos, &mut traversed_spots, &mut taken_spots);
        } else if pipe.to_pos < pos {
            let mut available = 0usize;
            while traversed_spots.contains(&available) {
                available += 1;
            }
            new_pipes.push(GraphPipe {
                from_hash: pipe.from_hash.clone(),
                to_hash: pipe.to_hash.clone(),
                from_pos: pipe.to_pos,
                to_pos: available,
                step: PipeStep::Continues,
                colour: pipe.colour,
            });
            traverse(
                pipe.to_pos,
                available,
                &mut traversed_spots,
                &mut taken_spots,
            );
        }
    }

    if commit.parents.len() > 1 {
        for parent in &commit.parents[1..] {
            let mut available = 0usize;
            while taken_spots.contains(&available) || traversed_for_continuing.contains(&available)
            {
                available += 1;
            }
            new_pipes.push(GraphPipe {
                from_hash: commit_hash.to_owned(),
                to_hash: Some(parent.as_str().to_owned()),
                from_pos: pos,
                to_pos: available,
                step: PipeStep::Starts,
                colour: allocate_colour(),
            });
            taken_spots.insert(available);
        }
    }

    for pipe in &current_pipes {
        if pipe.to_hash.as_deref() != Some(commit_hash) && pipe.to_pos > pos {
            let mut last = pipe.to_pos;
            let mut candidate = pipe.to_pos;
            while candidate > pos {
                if taken_spots.contains(&candidate) || traversed_spots.contains(&candidate) {
                    break;
                }
                last = candidate;
                candidate -= 1;
            }
            new_pipes.push(GraphPipe {
                from_hash: pipe.from_hash.clone(),
                to_hash: pipe.to_hash.clone(),
                from_pos: pipe.to_pos,
                to_pos: last,
                step: PipeStep::Continues,
                colour: pipe.colour,
            });
            traverse(pipe.to_pos, last, &mut traversed_spots, &mut taken_spots);
        }
    }

    new_pipes.sort_by(|left, right| {
        left.to_pos
            .cmp(&right.to_pos)
            .then_with(|| left.step.cmp(&right.step))
    });

    new_pipes
}

fn route_pipe_set(
    pipes: &[GraphPipe],
    selected_oid: Option<&str>,
    prev_commit_oid: Option<&str>,
    symbol_set: SymbolSet,
) -> Vec<RoutedCell> {
    let mut max_pos = 0usize;
    let mut commit_pos = 0usize;
    let mut start_count = 0usize;
    let mut commit_colour = None;

    for pipe in pipes {
        if pipe.step == PipeStep::Starts {
            start_count += 1;
            commit_pos = pipe.from_pos;
            if pipe.from_pos == pipe.to_pos {
                commit_colour = Some(pipe.colour);
            } else {
                commit_colour.get_or_insert(pipe.colour);
            }
        } else if pipe.step == PipeStep::Terminates {
            commit_pos = pipe.to_pos;
        }
        max_pos = max_pos.max(pipe.right());
    }

    let is_merge = start_count > 1;
    let mut cells = vec![RoutedCell::default(); max_pos + 1];

    let highlight = if prev_commit_oid.is_some() && prev_commit_oid == selected_oid {
        pipes.iter().any(|pipe| {
            Some(pipe.from_hash.as_str()) == selected_oid
                && (pipe.step != PipeStep::Terminates || pipe.from_pos != pipe.to_pos)
        })
    } else {
        true
    };

    let mut selected_pipes = Vec::new();
    let mut non_selected_pipes = Vec::new();
    for pipe in pipes {
        if highlight && Some(pipe.from_hash.as_str()) == selected_oid {
            selected_pipes.push(pipe);
        } else {
            non_selected_pipes.push(pipe);
        }
    }

    // Pass 1: non-selected starting pipes. Keep the commit marker on its own
    // lane colour even if a secondary merge parent starts to the left (`to_pos < commit_pos`).
    let mut drew_non_selected_start = false;
    for pipe in &non_selected_pipes {
        if pipe.step == PipeStep::Starts {
            render_pipe(&mut cells, pipe, pipe.colour, true);
            drew_non_selected_start = true;
        }
    }
    if drew_non_selected_start && let Some(colour) = commit_colour {
        cells[commit_pos].colour = colour;
    }

    // Pass 2: non-selected continuing and terminating pipes.
    for pipe in &non_selected_pipes {
        if pipe.step == PipeStep::Starts {
            continue;
        }
        if pipe.step == PipeStep::Terminates
            && pipe.from_pos == commit_pos
            && pipe.to_pos == commit_pos
        {
            if pipe.from_hash != START_HASH {
                cells[commit_pos].up = VerticalLine::Straight;
            }
            continue;
        }
        render_pipe(&mut cells, pipe, pipe.colour, false);
    }

    // Pass 3: selected commit's pipes.
    if symbol_set == SymbolSet::BoxDrawing {
        for pipe in &selected_pipes {
            for cell in &mut cells[pipe.left()..=pipe.right()] {
                cell.reset();
            }
        }
    }
    for pipe in &selected_pipes {
        render_pipe(&mut cells, pipe, HIGHLIGHT_COLOUR, true);
        if pipe.to_pos == commit_pos {
            cells[pipe.to_pos].colour = HIGHLIGHT_COLOUR;
        }
    }

    // Root commits carry no downward line below their marker.
    for pipe in pipes {
        if pipe.step == PipeStep::Starts && pipe.to_hash.is_none() {
            cells[pipe.to_pos].down = VerticalLine::None;
        }
    }

    // In BranchDrawing mode, draw horizontal selected pipes on top of unrelated
    // straight vertical pipes they cross so the selected path reads as one line.
    if symbol_set == SymbolSet::BranchDrawing {
        for pipe in &selected_pipes {
            let left = pipe.left();
            let right = pipe.right();
            if left + 1 < right {
                for cell in &mut cells[(left + 1)..right] {
                    if cell.up == VerticalLine::Straight
                        && cell.down == VerticalLine::Straight
                        && cell.colour != HIGHLIGHT_COLOUR
                    {
                        cell.horizontal_on_top = true;
                        cell.colour = HIGHLIGHT_COLOUR;
                    }
                }
            }
        }
    }

    cells[commit_pos].cell_type = if is_merge {
        CellType::Merge
    } else {
        CellType::Commit
    };

    cells
}

fn render_pipe(cells: &mut [RoutedCell], pipe: &GraphPipe, colour: u8, override_right: bool) {
    let left = pipe.left();
    let right = pipe.right();

    if left != right {
        for cell in &mut cells[(left + 1)..right] {
            cell.set_horizontal(colour, override_right);
        }
        cells[left].set_right(colour, override_right);
        cells[right].set_left(colour);
    }

    if matches!(pipe.step, PipeStep::Starts | PipeStep::Continues) {
        cells[pipe.to_pos].set_down(colour, line_towards(pipe.to_pos, pipe.from_pos));
    }
    if matches!(pipe.step, PipeStep::Terminates | PipeStep::Continues) {
        cells[pipe.from_pos].set_up(colour, line_towards(pipe.from_pos, pipe.to_pos));
    }
}

fn line_towards(pos: usize, other_pos: usize) -> VerticalLine {
    match other_pos.cmp(&pos) {
        Ordering::Less => VerticalLine::ToLeft,
        Ordering::Greater => VerticalLine::ToRight,
        Ordering::Equal => VerticalLine::Straight,
    }
}

fn box_cell_chars(cell: &RoutedCell) -> (&'static str, &'static str) {
    let (first, second) = box_drawing_chars(
        cell.up != VerticalLine::None,
        cell.down != VerticalLine::None,
        cell.left,
        cell.right,
    );
    match cell.cell_type {
        CellType::Commit => ("○", second),
        CellType::Merge => ("◎", second),
        CellType::Connection => (first, second),
    }
}

fn branch_cell_chars(cell: &RoutedCell) -> (&'static str, &'static str) {
    let second = if cell.right {
        BRANCH_DRAWING_HORIZONTAL
    } else {
        " "
    };

    match cell.cell_type {
        CellType::Commit | CellType::Merge => (
            filled_branch_commit_symbol(
                cell.up != VerticalLine::None,
                cell.down != VerticalLine::None,
                cell.left,
                cell.right,
            ),
            second,
        ),
        CellType::Connection => {
            if cell.horizontal_on_top {
                return (BRANCH_DRAWING_HORIZONTAL, second);
            }
            if let Some(first) = branch_connection_symbol(cell.up, cell.down, cell.horizontal) {
                return (first, second);
            }
            let (first, _) = box_drawing_chars(
                cell.up != VerticalLine::None,
                cell.down != VerticalLine::None,
                cell.left,
                cell.right,
            );
            (first, second)
        }
    }
}

const fn filled_branch_commit_symbol(
    up: bool,
    down: bool,
    left: bool,
    right: bool,
) -> &'static str {
    match (up, down, left, right) {
        (false, false, false, false) => "\u{f5ee}",
        (false, false, false, true) => "\u{f5f0}",
        (false, false, true, false) => "\u{f5f2}",
        (false, false, true, true) => "\u{f5f4}",
        (false, true, false, false) => "\u{f5f6}",
        (true, false, false, false) => "\u{f5f8}",
        (true, true, false, false) => "\u{f5fa}",
        (false, true, false, true) => "\u{f5fc}",
        (false, true, true, false) => "\u{f5fe}",
        (true, false, false, true) => "\u{f600}",
        (true, false, true, false) => "\u{f602}",
        (true, true, false, true) => "\u{f604}",
        (true, true, true, false) => "\u{f606}",
        (false, true, true, true) => "\u{f608}",
        (true, false, true, true) => "\u{f60a}",
        (true, true, true, true) => "\u{f60c}",
    }
}

const fn branch_connection_symbol(
    up: VerticalLine,
    down: VerticalLine,
    horizontal: bool,
) -> Option<&'static str> {
    use VerticalLine::{None as No, Straight, ToLeft, ToRight};
    match (up, down, horizontal) {
        (No, No, false) => Some(" "),
        (No, No, true) => Some(BRANCH_DRAWING_HORIZONTAL),
        (Straight, Straight, false) | (Straight, Straight, true) => Some("\u{f5d1}"),
        (ToLeft, No, false) => Some("\u{f5d9}"),
        (ToRight, No, false) => Some("\u{f5d8}"),
        (No, ToLeft, false) => Some("\u{f5d7}"),
        (No, ToRight, false) => Some("\u{f5d6}"),
        (ToLeft, No, true) => Some("\u{f5e3}"),
        (ToRight, No, true) => Some("\u{f5e4}"),
        (No, ToLeft, true) => Some("\u{f5e0}"),
        (No, ToRight, true) => Some("\u{f5e1}"),
        (ToLeft, ToLeft, false) => Some("\u{f5df}"),
        (ToRight, ToRight, false) => Some("\u{f5dc}"),
        (ToLeft, ToLeft, true) => Some("\u{f5e8}"),
        (ToRight, ToRight, true) => Some("\u{f5e9}"),
        (ToLeft, ToRight, true) => Some("\u{f5ec}"),
        (ToRight, ToLeft, true) => Some("\u{f5ed}"),
        _ => Option::None,
    }
}

const fn box_drawing_chars(
    up: bool,
    down: bool,
    left: bool,
    right: bool,
) -> (&'static str, &'static str) {
    match (up, down, left, right) {
        (true, true, true, true) => ("│", "─"),
        (true, true, true, false) => ("│", " "),
        (true, true, false, true) => ("│", "─"),
        (true, true, false, false) => ("│", " "),
        (true, false, true, true) => ("┴", "─"),
        (true, false, true, false) => ("╯", " "),
        (true, false, false, true) => ("╰", "─"),
        (true, false, false, false) => ("╵", " "),
        (false, true, true, true) => ("┬", "─"),
        (false, true, true, false) => ("╮", " "),
        (false, true, false, true) => ("╭", "─"),
        (false, true, false, false) => ("╷", " "),
        (false, false, true, true) => ("─", "─"),
        (false, false, true, false) => ("─", " "),
        (false, false, false, true) => ("╶", "─"),
        (false, false, false, false) => (" ", " "),
    }
}

#[cfg(test)]
mod tests {
    use file_types::Oid;

    use super::*;

    fn commit(oid: &str, parents: &[&str]) -> Commit {
        Commit {
            oid: Oid::new(oid),
            parents: parents.iter().copied().map(Oid::new).collect(),
            current: false,
            branches: Vec::new(),
            remotes: Vec::new(),
            tags: Vec::new(),
            subject: format!("commit {oid}"),
            author: "Test".to_owned(),
            date: "2026-09-10".to_owned(),
        }
    }

    fn branch_drawing_look_alike(ch: char, is_merge: bool) -> char {
        match ch {
            '\u{f5d0}' => '─',
            '\u{f5d1}' => '│',
            '\u{f5d6}' => '╭',
            '\u{f5d7}' => '╮',
            '\u{f5d8}' => '╰',
            '\u{f5d9}' => '╯',
            '\u{f5e0}' => '┬',
            '\u{f5e3}' => '┴',
            '\u{f5df}' => '┤',
            '\u{f5e8}' => '┼',
            '\u{f5ee}'..='\u{f60d}' => {
                if is_merge {
                    '◎'
                } else {
                    '○'
                }
            }
            other => other,
        }
    }

    fn assert_lazygit_box_graph(
        name: &str,
        specs: &[(&str, &[&str])],
        selected_oid: Option<&str>,
        expected_block: &str,
    ) {
        let commits: Vec<Commit> = specs
            .iter()
            .map(|(oid, parents)| commit(oid, parents))
            .collect();
        let built = graph(&commits);
        let actual: Vec<String> = specs
            .iter()
            .enumerate()
            .map(|(index, (oid, _))| {
                let cells = built.cells(index, &commits, selected_oid, SymbolSet::BoxDrawing);
                format!("{oid} {}", format_cells(&cells)).trim().to_owned()
            })
            .collect();
        let expected: Vec<String> = expected_block
            .trim()
            .lines()
            .map(|line| line.trim().to_owned())
            .collect();
        assert_eq!(actual, expected, "box-drawing case {name:?} mismatched");
    }

    fn assert_lazygit_branch_graph(
        name: &str,
        specs: &[(&str, &[&str])],
        selected_oid: Option<&str>,
        expected_block: &str,
    ) {
        let commits: Vec<Commit> = specs
            .iter()
            .map(|(oid, parents)| commit(oid, parents))
            .collect();
        let built = graph(&commits);
        let actual: Vec<String> = specs
            .iter()
            .enumerate()
            .map(|(index, (oid, parents))| {
                let is_merge = parents.len() > 1;
                let cells = built.cells(index, &commits, selected_oid, SymbolSet::BranchDrawing);
                let mapped: String = format_cells(&cells)
                    .chars()
                    .map(|ch| branch_drawing_look_alike(ch, is_merge))
                    .collect();
                format!("{oid} {mapped}").trim().to_owned()
            })
            .collect();
        let expected: Vec<String> = expected_block
            .trim()
            .lines()
            .map(|line| line.trim().to_owned())
            .collect();
        assert_eq!(actual, expected, "branch-drawing case {name:?} mismatched");
    }

    #[test]
    fn empty_history_produces_zero_lanes_and_empty_cells() {
        let built = graph(&[]);
        assert_eq!(built.lane_count(), 0);
        assert!(built.cells(0, &[], None, SymbolSet::BoxDrawing).is_empty());
    }

    #[test]
    fn lazygit_box_drawing_all_eleven_cases() {
        assert_lazygit_box_graph(
            "with some merges",
            &[
                ("1", &["2"]),
                ("2", &["3"]),
                ("3", &["4"]),
                ("4", &["5", "7"]),
                ("7", &["5"]),
                ("5", &["8"]),
                ("8", &["9"]),
                ("9", &["A", "B"]),
                ("B", &["D"]),
                ("D", &["D"]),
                ("A", &["E"]),
                ("E", &["F"]),
                ("F", &["D"]),
                ("D", &["G"]),
            ],
            Some("blah"),
            "
            1 ○
            2 ○
            3 ○
            4 ◎─╮
            7 │ ○
            5 ○─╯
            8 ○
            9 ◎─╮
            B │ ○
            D │ ○
            A ○ │
            E ○ │
            F ○ │
            D ○─╯
            ",
        );

        assert_lazygit_box_graph(
            "with a path that has room to move to the left",
            &[
                ("1", &["2"]),
                ("2", &["3", "4"]),
                ("4", &["3", "5"]),
                ("3", &["5"]),
                ("5", &["6"]),
                ("6", &["7"]),
            ],
            Some("blah"),
            "
            1 ○
            2 ◎─╮
            4 │ ◎─╮
            3 ○─╯ │
            5 ○───╯
            6 ○
            ",
        );

        assert_lazygit_box_graph(
            "with a new commit",
            &[
                ("1", &["2"]),
                ("2", &["3", "4"]),
                ("4", &["3", "5"]),
                ("Z", &["Z"]),
                ("3", &["5"]),
                ("5", &["6"]),
                ("6", &["7"]),
            ],
            Some("blah"),
            "
            1 ○
            2 ◎─╮
            4 │ ◎─╮
            Z │ │ │ ○
            3 ○─╯ │ │
            5 ○───╯ │
            6 ○ ╭───╯
            ",
        );

        assert_lazygit_box_graph(
            "with a root commit followed by an unrelated history",
            &[("1", &["2"]), ("2", &[]), ("A", &["B"]), ("B", &[])],
            Some("blah"),
            "
            1 ○
            2 ○
            A ○
            B ○
            ",
        );

        assert_lazygit_box_graph(
            "with a merge of an unrelated history",
            &[("1", &["2", "A"]), ("2", &["3"]), ("A", &[]), ("3", &[])],
            Some("blah"),
            "
            1 ◎─╮
            2 ○ │
            A │ ○
            3 ○
            ",
        );

        assert_lazygit_box_graph(
            "room to move left and continues 1",
            &[
                ("1", &["2"]),
                ("2", &["3", "4"]),
                ("3", &["5", "4"]),
                ("5", &["7", "8"]),
                ("4", &["7"]),
                ("7", &["11"]),
            ],
            Some("blah"),
            "
            1 ○
            2 ◎─╮
            3 ◎─│─╮
            5 ◎─│─│─╮
            4 │ ○─╯ │
            7 ○─╯ ╭─╯
            ",
        );

        assert_lazygit_box_graph(
            "room to move left and continues 2",
            &[
                ("1", &["2"]),
                ("2", &["3", "4"]),
                ("3", &["5", "4"]),
                ("5", &["7", "8"]),
                ("7", &["4", "A"]),
                ("4", &["B"]),
                ("B", &["C"]),
            ],
            Some("blah"),
            "
            1 ○
            2 ◎─╮
            3 ◎─│─╮
            5 ◎─│─│─╮
            7 ◎─│─│─│─╮
            4 ○─┴─╯ │ │
            B ○ ╭───╯ │
            ",
        );

        assert_lazygit_box_graph(
            "room to move left and continues 3",
            &[
                ("1", &["2", "3"]),
                ("3", &["2"]),
                ("2", &["4", "5"]),
                ("4", &["6", "7"]),
                ("6", &["8"]),
            ],
            Some("blah"),
            "
            1 ◎─╮
            3 │ ○
            2 ◎─│
            4 ◎─│─╮
            6 ○ │ │
            ",
        );

        assert_lazygit_box_graph(
            "new merge path fills gap before continuing path on right",
            &[
                ("1", &["2", "3", "4", "5"]),
                ("4", &["2"]),
                ("2", &["A"]),
                ("A", &["6", "B"]),
                ("B", &["C"]),
            ],
            Some("blah"),
            "
            1 ◎─┬─┬─╮
            4 │ │ ○ │
            2 ○─│─╯ │
            A ◎─│─╮ │
            B │ │ ○ │
            ",
        );

        assert_lazygit_box_graph(
            "room to move left and continues 4",
            &[
                ("1", &["2"]),
                ("2", &["3", "4"]),
                ("3", &["5", "4"]),
                ("5", &["7", "8"]),
                ("7", &["4", "A"]),
                ("4", &["B"]),
                ("B", &["C"]),
                ("C", &["D"]),
            ],
            Some("blah"),
            "
            1 ○
            2 ◎─╮
            3 ◎─│─╮
            5 ◎─│─│─╮
            7 ◎─│─│─│─╮
            4 ○─┴─╯ │ │
            B ○ ╭───╯ │
            C ○ │ ╭───╯
            ",
        );

        assert_lazygit_box_graph(
            "room to move left and continues 5",
            &[
                ("1", &["2"]),
                ("2", &["3", "4"]),
                ("3", &["5", "4"]),
                ("5", &["7", "G"]),
                ("7", &["8", "A"]),
                ("8", &["4", "E"]),
                ("4", &["B"]),
                ("B", &["C"]),
                ("C", &["D"]),
                ("D", &["F"]),
            ],
            Some("blah"),
            "
            1 ○
            2 ◎─╮
            3 ◎─│─╮
            5 ◎─│─│─╮
            7 ◎─│─│─│─╮
            8 ◎─│─│─│─│─╮
            4 ○─┴─╯ │ │ │
            B ○ ╭───╯ │ │
            C ○ │ ╭───╯ │
            D ○ │ │ ╭───╯
            ",
        );
    }

    #[test]
    fn lazygit_branch_drawing_all_five_cases() {
        assert_lazygit_branch_graph(
            "branch forked off a merge commit",
            &[
                ("1", &["2", "3"]),
                ("3", &["5"]),
                ("2", &["5", "4"]),
                ("4", &["5"]),
                ("5", &["6", "7"]),
                ("7", &["6"]),
                ("6", &["8"]),
            ],
            Some(""),
            "
            1 ◎─╮
            3 │ ○
            2 ◎─│─╮
            4 │ │ ○
            5 ◎─┼─╯
            7 │ ○
            6 ○─╯
            ",
        );

        assert_lazygit_branch_graph(
            "branch forked off a merge commit, with the merge commit selected",
            &[
                ("1", &["2", "3"]),
                ("3", &["5"]),
                ("2", &["5", "4"]),
                ("4", &["5"]),
                ("5", &["6", "7"]),
                ("7", &["6"]),
                ("6", &["8"]),
            ],
            Some("5"),
            "
            1 ◎─╮
            3 │ ○
            2 ◎─│─╮
            4 │ │ ○
            5 ◎─┼─╯
            7 │ ○
            6 ○─╯
            ",
        );

        assert_lazygit_branch_graph(
            "branch forked off a merge commit, with the merge commit above it selected",
            &[
                ("1", &["2", "3"]),
                ("3", &["5"]),
                ("2", &["5", "4"]),
                ("4", &["5"]),
                ("5", &["6", "7"]),
                ("7", &["6"]),
                ("6", &["8"]),
            ],
            Some("2"),
            "
            1 ◎─╮
            3 │ ○
            2 ◎───╮
            4 │ │ ○
            5 ◎─┼─╯
            7 │ ○
            6 ○─╯
            ",
        );

        assert_lazygit_branch_graph(
            "branch forked off a merge commit, with no line passing through",
            &[
                ("1", &["2", "3"]),
                ("3", &["2"]),
                ("2", &["4", "5"]),
                ("4", &["6", "7"]),
                ("6", &["8"]),
            ],
            Some(""),
            "
            1 ◎─╮
            3 │ ○
            2 ◎─┤
            4 ◎─│─╮
            6 ○ │ │
            ",
        );

        assert_lazygit_branch_graph(
            "several lines ending in a commit",
            &[
                ("1", &["2"]),
                ("2", &["3", "4"]),
                ("3", &["5", "4"]),
                ("5", &["7", "8"]),
                ("7", &["4", "A"]),
                ("4", &["B"]),
                ("B", &["C"]),
            ],
            Some(""),
            "
            1 ○
            2 ◎─╮
            3 ◎─│─╮
            5 ◎─│─│─╮
            7 ◎─│─│─│─╮
            4 ○─┴─╯ │ │
            B ○ ╭───╯ │
            ",
        );
    }

    #[test]
    fn lazygit_render_pipe_set_with_branch_drawing_symbols() {
        let cyan = 0u8;
        let red = 1u8;
        let green = 2u8;
        let yellow = 3u8;
        let magenta = 4u8;

        let check = |name: &str,
                     pipes: Vec<GraphPipe>,
                     prev: Option<&str>,
                     expected_chars: &str,
                     expected_styles: &[u8]| {
            let routed = route_pipe_set(&pipes, Some("selected"), prev, SymbolSet::BranchDrawing);
            let cells: Vec<GraphCell> = routed
                .iter()
                .map(|cell| resolve_cell(cell, SymbolSet::BranchDrawing))
                .collect();
            assert_eq!(format_cells(&cells), expected_chars, "{name}: chars");
            assert_eq!(cell_styles(&cells), expected_styles, "{name}: styles");
        };

        // 1. commit with lines above and below (filled circle \u{f5fa})
        check(
            "commit with lines above and below",
            vec![
                GraphPipe {
                    from_pos: 0,
                    to_pos: 0,
                    from_hash: "a".to_owned(),
                    to_hash: Some("b".to_owned()),
                    step: PipeStep::Terminates,
                    colour: cyan,
                },
                GraphPipe {
                    from_pos: 0,
                    to_pos: 0,
                    from_hash: "b".to_owned(),
                    to_hash: Some("c".to_owned()),
                    step: PipeStep::Starts,
                    colour: green,
                },
            ],
            Some("a"),
            "\u{f5fa}",
            &[green],
        );

        // 2. first commit (filled circle \u{f5f6} with line down only)
        check(
            "first commit",
            vec![
                GraphPipe {
                    from_pos: 0,
                    to_pos: 0,
                    from_hash: START_HASH.to_owned(),
                    to_hash: Some("b".to_owned()),
                    step: PipeStep::Terminates,
                    colour: cyan,
                },
                GraphPipe {
                    from_pos: 0,
                    to_pos: 0,
                    from_hash: "b".to_owned(),
                    to_hash: Some("c".to_owned()),
                    step: PipeStep::Starts,
                    colour: green,
                },
            ],
            None,
            "\u{f5f6}",
            &[green],
        );

        // 3. root commit (filled circle \u{f5f8} with line up only)
        check(
            "root commit",
            vec![
                GraphPipe {
                    from_pos: 0,
                    to_pos: 0,
                    from_hash: "a".to_owned(),
                    to_hash: Some("b".to_owned()),
                    step: PipeStep::Terminates,
                    colour: cyan,
                },
                GraphPipe {
                    from_pos: 0,
                    to_pos: 0,
                    from_hash: "b".to_owned(),
                    to_hash: None,
                    step: PipeStep::Starts,
                    colour: green,
                },
            ],
            Some("a"),
            "\u{f5f8}",
            &[green],
        );

        // 4. selected root commit
        check(
            "selected root commit",
            vec![
                GraphPipe {
                    from_pos: 0,
                    to_pos: 0,
                    from_hash: "a".to_owned(),
                    to_hash: Some("selected".to_owned()),
                    step: PipeStep::Terminates,
                    colour: cyan,
                },
                GraphPipe {
                    from_pos: 0,
                    to_pos: 0,
                    from_hash: "selected".to_owned(),
                    to_hash: None,
                    step: PipeStep::Starts,
                    colour: green,
                },
            ],
            Some("a"),
            "\u{f5f8}",
            &[HIGHLIGHT_COLOUR],
        );

        // 5. line of the selected commit crossing another line (`horizontal_on_top`)
        check(
            "line of the selected commit crossing another line",
            vec![
                GraphPipe {
                    from_pos: 0,
                    to_pos: 0,
                    from_hash: "a1".to_owned(),
                    to_hash: Some("selected".to_owned()),
                    step: PipeStep::Terminates,
                    colour: red,
                },
                GraphPipe {
                    from_pos: 0,
                    to_pos: 0,
                    from_hash: "selected".to_owned(),
                    to_hash: Some("a3".to_owned()),
                    step: PipeStep::Starts,
                    colour: yellow,
                },
                GraphPipe {
                    from_pos: 1,
                    to_pos: 1,
                    from_hash: "b1".to_owned(),
                    to_hash: Some("b2".to_owned()),
                    step: PipeStep::Continues,
                    colour: magenta,
                },
                GraphPipe {
                    from_pos: 3,
                    to_pos: 0,
                    from_hash: "e1".to_owned(),
                    to_hash: Some("selected".to_owned()),
                    step: PipeStep::Terminates,
                    colour: green,
                },
                GraphPipe {
                    from_pos: 0,
                    to_pos: 2,
                    from_hash: "selected".to_owned(),
                    to_hash: Some("c3".to_owned()),
                    step: PipeStep::Starts,
                    colour: yellow,
                },
            ],
            Some("a1"),
            "\u{f604}\u{f5d0}\u{f5d0}\u{f5d0}\u{f5e0}\u{f5d0}\u{f5d9}",
            &[
                HIGHLIGHT_COLOUR,
                HIGHLIGHT_COLOUR,
                HIGHLIGHT_COLOUR,
                HIGHLIGHT_COLOUR,
                HIGHLIGHT_COLOUR,
                green,
                green,
            ],
        );

        // 6. line of the selected commit passing through a double-bend cell (\u{f5e8})
        check(
            "line of the selected commit passing through a cell in which other lines bend",
            vec![
                GraphPipe {
                    from_pos: 0,
                    to_pos: 0,
                    from_hash: "a1".to_owned(),
                    to_hash: Some("a2".to_owned()),
                    step: PipeStep::Terminates,
                    colour: red,
                },
                GraphPipe {
                    from_pos: 1,
                    to_pos: 0,
                    from_hash: "b1".to_owned(),
                    to_hash: Some("a2".to_owned()),
                    step: PipeStep::Terminates,
                    colour: magenta,
                },
                GraphPipe {
                    from_pos: 2,
                    to_pos: 0,
                    from_hash: "selected".to_owned(),
                    to_hash: Some("a2".to_owned()),
                    step: PipeStep::Terminates,
                    colour: cyan,
                },
                GraphPipe {
                    from_pos: 0,
                    to_pos: 0,
                    from_hash: "a2".to_owned(),
                    to_hash: Some("a3".to_owned()),
                    step: PipeStep::Starts,
                    colour: green,
                },
                GraphPipe {
                    from_pos: 0,
                    to_pos: 1,
                    from_hash: "a2".to_owned(),
                    to_hash: Some("b3".to_owned()),
                    step: PipeStep::Starts,
                    colour: green,
                },
            ],
            Some("a1"),
            "\u{f604}\u{f5d0}\u{f5e8}\u{f5d0}\u{f5d9}",
            &[
                HIGHLIGHT_COLOUR,
                HIGHLIGHT_COLOUR,
                magenta,
                HIGHLIGHT_COLOUR,
                HIGHLIGHT_COLOUR,
            ],
        );
    }

    #[test]
    fn codediff_press_lane_colours_keep_main_blue_and_branch_pipe_continuous() {
        let commits = vec![
            commit("head", &["merge"]),
            commit("merge", &["base", "topic"]),
            commit("topic", &["base"]),
            commit("base", &[]),
        ];
        let built = graph(&commits);
        let row_styles =
            |index| cell_styles(&built.cells(index, &commits, None, SymbolSet::BranchDrawing));

        assert_eq!(built.lane_count(), 2);
        assert_eq!(row_styles(0), vec![0]);
        assert_eq!(row_styles(1), vec![0, 1, 1]);
        assert_eq!(row_styles(2), vec![0, MARKER_COLOUR, 1]);
        assert_eq!(row_styles(3), vec![0, 1, 1]);
    }

    fn cell_styles(cells: &[GraphCell]) -> Vec<u8> {
        let char_len = format_cells(cells).chars().count();
        let mut styles = Vec::with_capacity(cells.len() * 2);
        for cell in cells {
            styles.push(cell.first_colour);
            styles.push(cell.second_colour);
        }
        styles.truncate(char_len);
        styles
    }
}
