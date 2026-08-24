//! The flex pass: hypothetical sizes, free space, growing, shrinking,
//! freezing, and the rectangles that come out.

use ratatui::layout::Rect;

use super::{Axis, Basis, Layout};

/// One child, as the flex pass sees it: what it asked for, and what it
/// measured when it asked for `Auto`.
pub(crate) struct Item {
    pub layout: Layout,
    /// What the measure pass found, on the main axis.
    pub measured: u16,
}

/// What the pass decided.
pub(crate) struct Assigned {
    /// One rectangle per child, in child order. Hidden children get
    /// `Rect::ZERO`.
    pub areas: Vec<Rect>,
    /// A child was left below its minimum, so the container cannot show its
    /// children at this size.
    pub too_small: bool,
}

/// Lays `children` out inside `inner`.
pub(crate) fn assign(axis: Axis, inner: Rect, gap: u16, children: &[Item]) -> Assigned {
    if axis == Axis::Over {
        return over(inner, children);
    }

    let main_room = if axis == Axis::Across { inner.width } else { inner.height };
    let cross_room = if axis == Axis::Across { inner.height } else { inner.width };

    let shown: Vec<usize> = (0..children.len()).filter(|&i| !children[i].layout.hidden).collect();
    let gaps = gap.saturating_mul(shown.len().saturating_sub(1) as u16);
    let room = main_room.saturating_sub(gaps);

    let main = resolve(axis, room, children, &shown);

    // R5.5.1 — the cross axis is the container's, clamped by the child's own
    // bounds. `align-items: stretch`, and the only alignment there is.
    let mut areas = vec![Rect::ZERO; children.len()];
    let mut at = if axis == Axis::Across { inner.x } else { inner.y };
    let mut left = main_room;
    let mut too_small = false;

    for &i in &shown {
        let layout = children[i].layout;
        // A child never reaches past the container. Where CSS overflows, this
        // cuts, and the cut is what R5.6.1 sees.
        let size = main[i].min(left);
        let cross = clamp(cross_room, min_on(layout, cross_axis(axis)), max_on(layout, cross_axis(axis)));

        areas[i] = if axis == Axis::Across {
            Rect { x: at, y: inner.y, width: size, height: cross }
        } else {
            Rect { x: inner.x, y: at, width: cross, height: size }
        };

        // R5.6.1 — short of its minimum because shrinking ran out of room.
        if areas[i].width < layout.min_width || areas[i].height < layout.min_height {
            too_small = true;
        }

        at = at.saturating_add(size).saturating_add(gap);
        left = left.saturating_sub(size).saturating_sub(gap);
    }

    Assigned { areas, too_small }
}

/// R5.5.4 — `Stack` gives every child the whole inner rectangle.
fn over(inner: Rect, children: &[Item]) -> Assigned {
    let mut areas = vec![Rect::ZERO; children.len()];
    let mut too_small = false;
    for (i, child) in children.iter().enumerate() {
        if child.layout.hidden {
            continue;
        }
        areas[i] = inner;
        if inner.width < child.layout.min_width || inner.height < child.layout.min_height {
            too_small = true;
        }
    }
    Assigned { areas, too_small }
}

/// §5.4 — CSS's *resolve flexible lengths*, in `u16`, one line, no wrap.
fn resolve(axis: Axis, room: u16, children: &[Item], shown: &[usize]) -> Vec<u16> {
    let mut size = vec![0u16; children.len()];
    let mut frozen = vec![false; children.len()];

    // R5.4.1 — the hypothetical size, clamped to the child's own bounds.
    for &i in shown {
        let layout = children[i].layout;
        let want = match layout.basis {
            Basis::Auto => children[i].measured,
            Basis::Length(n) => n,
            // What a percentage is a share of is the inner main size.
            Basis::Percent(n) => (u32::from(room) * u32::from(n) / 100) as u16,
        };
        size[i] = clamp(want, min_on(layout, axis), max_on(layout, axis));
    }

    // R5.4.5 — each round freezes at least one child, so this ends.
    for _ in 0..=shown.len() {
        let taken: u32 = shown.iter().map(|&i| u32::from(size[i])).sum();
        let free = i64::from(room) - i64::from(taken);

        if free == 0 {
            break;
        }

        let movable: Vec<usize> = shown
            .iter()
            .copied()
            .filter(|&i| {
                !frozen[i]
                    && if free > 0 { children[i].layout.grow > 0 } else { children[i].layout.shrink > 0 }
            })
            .collect();

        if movable.is_empty() {
            break;
        }

        let shares: Vec<u32> = movable
            .iter()
            .map(|&i| {
                if free > 0 {
                    // R5.4.3 — in proportion to `grow`.
                    u32::from(children[i].layout.grow)
                } else {
                    // R5.4.4 — CSS's scaled shrink factor, so a wide child
                    // gives up more than a narrow one at the same `shrink`.
                    u32::from(children[i].layout.shrink) * u32::from(size[i])
                }
            })
            .collect();

        let total: u32 = shares.iter().sum();
        if total == 0 {
            break;
        }

        let moving = free.unsigned_abs() as u32;
        let mut given = vec![0u32; movable.len()];
        let mut remainders = Vec::with_capacity(movable.len());
        let mut handed = 0u32;

        for (n, (&i, &share)) in movable.iter().zip(&shares).enumerate() {
            let exact = u64::from(moving) * u64::from(share);
            let whole = (exact / u64::from(total)) as u32;
            given[n] = whole;
            handed += whole;
            remainders.push((exact % u64::from(total), i, n));
            let _ = i;
        }

        // R5.4.7 — the remainder goes a cell at a time to the largest
        // fractional parts, ties to the earlier child, so the same split comes
        // out every frame.
        remainders.sort_by(|a, b| b.0.cmp(&a.0).then(a.2.cmp(&b.2)));
        let mut left = moving.saturating_sub(handed);
        for &(_, _, n) in &remainders {
            if left == 0 {
                break;
            }
            given[n] += 1;
            left -= 1;
        }

        let mut froze = false;
        for (n, &i) in movable.iter().enumerate() {
            let layout = children[i].layout;
            let want = if free > 0 {
                u32::from(size[i]).saturating_add(given[n])
            } else {
                u32::from(size[i]).saturating_sub(given[n])
            };
            let want = want.min(u32::from(u16::MAX)) as u16;
            let clamped = clamp(want, min_on(layout, axis), max_on(layout, axis));
            if clamped != want {
                frozen[i] = true;
                froze = true;
            }
            size[i] = clamped;
        }

        // Nothing hit a bound, so the space is spent and the pass is done.
        if !froze {
            break;
        }
    }

    size
}

fn cross_axis(axis: Axis) -> Axis {
    match axis {
        Axis::Across => Axis::Down,
        Axis::Down => Axis::Across,
        Axis::Over => Axis::Over,
    }
}

fn min_on(layout: Layout, axis: Axis) -> u16 {
    match axis {
        Axis::Across => layout.min_width,
        Axis::Down => layout.min_height,
        Axis::Over => 0,
    }
}

fn max_on(layout: Layout, axis: Axis) -> Option<u16> {
    match axis {
        Axis::Across => layout.max_width,
        Axis::Down => layout.max_height,
        Axis::Over => None,
    }
}

fn clamp(n: u16, min: u16, max: Option<u16>) -> u16 {
    let n = n.max(min);
    match max {
        Some(m) => n.min(m.max(min)),
        None => n,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item(layout: Layout, measured: u16) -> Item {
        Item { layout, measured }
    }

    fn across(width: u16, gap: u16, children: &[Item]) -> Vec<u16> {
        assign(Axis::Across, Rect { x: 0, y: 0, width, height: 10 }, gap, children)
            .areas
            .iter()
            .map(|r| r.width)
            .collect()
    }

    #[test]
    fn a_fixed_child_measures_as_its_size_not_its_content() {
        let got = across(
            50,
            0,
            &[item(Layout { basis: Basis::Length(20), ..Default::default() }, 99)],
        );
        assert_eq!(got, vec![20]);
    }

    #[test]
    fn a_percent_child_is_a_share_of_the_inner_size() {
        let got = across(
            80,
            0,
            &[item(Layout { basis: Basis::Percent(25), shrink: 0, ..Default::default() }, 0)],
        );
        assert_eq!(got, vec![20]);
    }

    #[test]
    fn free_space_counts_the_gaps() {
        let got = across(
            10,
            2,
            &[
                item(Layout { grow: 1, basis: Basis::Length(0), ..Default::default() }, 0),
                item(Layout { grow: 1, basis: Basis::Length(0), ..Default::default() }, 0),
            ],
        );
        assert_eq!(got, vec![4, 4], "10 cells less one 2-cell gap, split two ways");
    }

    #[test]
    fn two_growing_children_split_what_the_fixed_one_left() {
        let got = across(
            100,
            0,
            &[
                item(Layout { basis: Basis::Length(40), grow: 0, shrink: 0, ..Default::default() }, 0),
                item(Layout { grow: 1, basis: Basis::Length(0), ..Default::default() }, 0),
                item(Layout { grow: 1, basis: Basis::Length(0), ..Default::default() }, 0),
            ],
        );
        assert_eq!(got, vec![40, 30, 30]);
    }

    #[test]
    fn a_wide_child_gives_up_more_than_a_narrow_one() {
        let got = across(
            60,
            0,
            &[
                item(Layout { basis: Basis::Length(80), shrink: 1, ..Default::default() }, 0),
                item(Layout { basis: Basis::Length(20), shrink: 1, ..Default::default() }, 0),
            ],
        );
        assert_eq!(got.iter().sum::<u16>(), 60);
        assert!(80 - got[0] > 20 - got[1], "the wide one gave up more: {got:?}");
    }

    #[test]
    fn an_odd_split_is_the_same_two_frames_running() {
        let three = || {
            across(
                100,
                0,
                &[
                    item(Layout { grow: 1, basis: Basis::Length(0), ..Default::default() }, 0),
                    item(Layout { grow: 1, basis: Basis::Length(0), ..Default::default() }, 0),
                    item(Layout { grow: 1, basis: Basis::Length(0), ..Default::default() }, 0),
                ],
            )
        };
        assert_eq!(three(), vec![34, 33, 33]);
        assert_eq!(three(), three());
    }

    /// The four widths `render::layout::split` answers today.
    #[test]
    fn the_flex_pass_reproduces_the_split_it_replaces() {
        let split = |width| {
            across(
                width,
                0,
                &[
                    item(Layout { basis: Basis::Length(40), shrink: 1, min_width: 8, ..Default::default() }, 0),
                    item(Layout { basis: Basis::Length(1), shrink: 0, grow: 0, ..Default::default() }, 0),
                    item(Layout { grow: 1, basis: Basis::Length(0), shrink: 0, min_width: 20, ..Default::default() }, 0),
                ],
            )
        };
        assert_eq!(split(80), vec![40, 1, 39]);
        assert_eq!(split(50), vec![29, 1, 20]);
        assert_eq!(split(29), vec![8, 1, 20]);
    }

    #[test]
    fn a_child_below_its_minimum_makes_its_parent_too_small() {
        let out = assign(
            Axis::Across,
            Rect { x: 0, y: 0, width: 10, height: 4 },
            0,
            &[item(Layout { min_width: 20, ..Default::default() }, 0)],
        );
        assert!(out.too_small);
    }

    #[test]
    fn a_child_of_a_row_is_as_tall_as_the_row() {
        let out = assign(
            Axis::Across,
            Rect { x: 0, y: 0, width: 10, height: 7 },
            0,
            &[item(Layout::default(), 3)],
        );
        assert_eq!(out.areas[0].height, 7);
    }

    #[test]
    fn a_stack_gives_every_child_the_same_rectangle() {
        let area = Rect { x: 2, y: 1, width: 10, height: 4 };
        let out = assign(Axis::Over, area, 0, &[item(Layout::default(), 0), item(Layout::default(), 0)]);
        assert_eq!(out.areas, vec![area, area]);
    }

    #[test]
    fn a_wider_screen_never_shows_less_than_a_narrower_one() {
        let fits = |width| {
            !assign(
                Axis::Across,
                Rect { x: 0, y: 0, width, height: 4 },
                0,
                &[
                    item(Layout { basis: Basis::Length(40), shrink: 1, min_width: 8, ..Default::default() }, 0),
                    item(Layout { grow: 1, basis: Basis::Length(0), min_width: 20, ..Default::default() }, 0),
                ],
            )
            .too_small
        };
        for width in 0..200u16 {
            if fits(width) {
                assert!(fits(width + 1), "fits at {width} but not at {}", width + 1);
            }
        }
    }
}
