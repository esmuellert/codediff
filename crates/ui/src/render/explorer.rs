//! Fitting one explorer row into the width it has.
//!
//! A brick: it is handed regions and a rectangle, and knows nothing about
//! trees, folds or files. That is what lets "does a narrow pane keep the file
//! name" be asked of a list of strings rather than of a screenshot.
//!
//! Two rules, in this order. Whole regions are dropped by
//! [`priority`](explorer::priority), lowest first and a whole priority level
//! at a time — so a count never loses its closing bracket while keeping its
//! opening one. Only when nothing is left to drop is a region cut, and the
//! widest one is chosen, because cutting the longest is what removes the most
//! for the least loss.

use explorer::Region;

/// The one column that always separates the two sides.
const GAP: usize = 1;

/// What a row will actually show at a given width.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fitted {
    pub left: Vec<Region>,
    pub right: Vec<Region>,
    /// Columns between the two sides. At least [`GAP`], and everything spare
    /// when there is room.
    pub gap: usize,
}

/// Chooses what survives at `width` columns.
pub fn fit(left: &[Region], right: &[Region], width: usize) -> Fitted {
    let mut left = left.to_vec();
    let mut right = right.to_vec();

    while total(&left, &right) > width {
        let Some(level) = lowest_priority(&left, &right) else {
            break;
        };
        left.retain(|region| region.priority != Some(level));
        right.retain(|region| region.priority != Some(level));
    }

    // A loop, not one pass: dropping the widest region can leave the row still
    // too wide, and returning without re-checking broke the promise this
    // function's name makes. It was invisible only because the cell writer
    // clips.
    while total(&left, &right) > width && !(left.is_empty() && right.is_empty()) {
        let over = total(&left, &right) - width;
        if !truncate_widest(&mut left, &mut right, over) {
            break;
        }
    }
    debug_assert!(
        total(&left, &right) <= width || (left.is_empty() && right.is_empty()),
        "a row of {} columns was fitted into {width}",
        total(&left, &right)
    );

    let spare = width.saturating_sub(sum(&left) + sum(&right));
    // No gap when there is nothing on the other side of it: a heading would
    // otherwise be followed by a column of trailing space that widens the row
    // past the pane.
    let gap = if left.is_empty() || right.is_empty() {
        0
    } else {
        spare.max(GAP)
    };
    Fitted { left, right, gap }
}

fn sum(regions: &[Region]) -> usize {
    regions.iter().map(Region::width).sum()
}

/// What the row needs, including the space between the sides.
fn total(left: &[Region], right: &[Region]) -> usize {
    let gap = if left.is_empty() || right.is_empty() {
        0
    } else {
        GAP
    };
    sum(left) + gap + sum(right)
}

fn lowest_priority(left: &[Region], right: &[Region]) -> Option<u8> {
    left.iter()
        .chain(right)
        .filter_map(|region| region.priority)
        .min()
}

/// Cuts `over` columns from the widest region, ending it with an ellipsis.
///
/// The ellipsis costs a column of its own, so a region cut to nothing is
/// dropped rather than left as a lone `…` saying nothing about what was there.
///
/// Returns whether anything changed, so the caller's loop cannot spin on a row
/// it can no longer make narrower.
fn truncate_widest(left: &mut Vec<Region>, right: &mut Vec<Region>, over: usize) -> bool {
    let widest = left
        .iter_mut()
        .chain(right.iter_mut())
        .max_by_key(|region| region.width());
    let Some(region) = widest else {
        return false;
    };
    if region.width() == 0 {
        return false;
    }
    let keep = region.width().saturating_sub(over + 1);
    if keep == 0 {
        region.text.clear();
        left.retain(|region| !region.text.is_empty());
        right.retain(|region| !region.text.is_empty());
        return true;
    }
    // Cut in cells, not characters: a wide glyph is two columns, and cutting
    // by character count would leave the row wider than the pane.
    region.cut(keep);
    region.text.push('…');
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use explorer::RegionType;

    fn fixed(text: &str) -> Region {
        Region::fixed(text, RegionType::Name)
    }

    fn droppable(text: &str, priority: u8) -> Region {
        Region::droppable(text, RegionType::Added, priority)
    }

    fn shown(fitted: &Fitted) -> String {
        let mut out = String::new();
        for region in &fitted.left {
            out.push_str(&region.text);
        }
        out.push_str(&" ".repeat(fitted.gap));
        for region in &fitted.right {
            out.push_str(&region.text);
        }
        out
    }

    #[test]
    fn a_wide_pane_pushes_the_two_sides_apart() {
        let fitted = fit(&[fixed("a.rs")], &[fixed("M")], 20);
        assert_eq!(shown(&fitted), "a.rs               M");
        assert_eq!(shown(&fitted).chars().count(), 20);
    }

    #[test]
    fn the_lowest_priority_goes_first() {
        let left = [fixed("a.rs"), droppable(" ← old.rs", 0)];
        let right = [droppable("+4", 1), fixed("M")];
        assert_eq!(shown(&fit(&left, &right, 20)), "a.rs ← old.rs    +4M");
        assert_eq!(shown(&fit(&left, &right, 12)), "a.rs     +4M");
        assert_eq!(shown(&fit(&left, &right, 6)), "a.rs M");
    }

    #[test]
    fn a_whole_priority_level_goes_at_once() {
        // The failure this prevents: `(3 · ` kept while `)` is dropped,
        // leaving a bracket that never closes.
        let left = [
            fixed("Changes"),
            droppable(" (3 · ", 2),
            droppable("+9", 2),
            droppable(")", 2),
        ];
        let fitted = fit(&left, &[], 10);
        assert_eq!(shown(&fitted), "Changes");
    }

    #[test]
    fn what_cannot_be_dropped_is_cut_and_says_so() {
        let left = [fixed("a-very-long-file-name.rs")];
        let fitted = fit(&left, &[fixed("M")], 12);
        assert_eq!(shown(&fitted), "a-very-lo… M");
        assert_eq!(fitted.left[0].width(), 10, "the ellipsis is one of them");
    }

    #[test]
    fn the_widest_is_the_one_that_is_cut() {
        // Cutting the indent guides instead of the name would save the same
        // columns and cost the reader the thing they were looking for.
        let left = [fixed("│ │ "), fixed("a-long-name.rs")];
        let fitted = fit(&left, &[], 10);
        assert_eq!(shown(&fitted), "│ │ a-lon…");
    }

    #[test]
    fn a_row_never_comes_out_wider_than_the_pane() {
        // The assertion used to be `<= width.max(2)`, which at widths 0 and 1
        // permitted two columns and so asserted nothing at all. It really only
        // checked that nothing panicked, which its name half admitted.
        let rows: [(&[Region], &[Region]); 4] = [
            (&[fixed("a.rs")], &[fixed("M")]),
            (
                &[
                    fixed("│ │ "),
                    fixed("a-long-name.rs"),
                    droppable(" ← was.rs", 0),
                ],
                &[droppable("+12 -3", 1), fixed("M")],
            ),
            (&[fixed("ファイル.txt")], &[fixed("??")]),
            (&[], &[]),
        ];
        for width in 0..30 {
            for (left, right) in rows {
                let fitted = fit(left, right, width);
                let drawn = shown(&fitted).chars().count();
                assert!(drawn <= width, "{drawn} columns drawn into {width}");
            }
        }
    }
}
