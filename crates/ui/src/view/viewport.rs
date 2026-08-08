//! Where a pane is looking: one `top`, one `cursor`, one `left`.
//!
//! One position for the whole pane. A side-by-side diff's two columns share
//! it, so they cannot drift apart — no scroll synchronisation needed.
//!
//! Everything here is arithmetic over a row count the buffer supplies.
//! Nothing here knows what a row contains.

use crate::input::{Motion, SCROLL_STEP};

/// Where one pane is looking: scroll position and cursor.
#[derive(Debug, Clone)]
pub struct Viewport {
    /// First visible view line, 0-based.
    top: u32,
    /// The view line the cursor is on.
    cursor: u32,
    /// Horizontal scroll in cells. Shared by every column, for the same
    /// reason as `top`.
    left: u32,
    /// Text height of the last frame, needed by page motions and to keep the
    /// cursor visible. Zero until the first draw.
    height: u32,
}

/// ViewLines kept between the cursor and the edge while scrolling.
const SCROLLOFF: u32 = 3;

impl Default for Viewport {
    fn default() -> Self {
        Self::new()
    }
}

impl Viewport {
    pub fn new() -> Self {
        Self {
            top: 0,
            cursor: 0,
            left: 0,
            height: 0,
        }
    }

    pub fn top(&self) -> u32 {
        self.top
    }

    pub fn cursor(&self) -> u32 {
        self.cursor
    }

    pub fn left(&self) -> u32 {
        self.left
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    /// Records the text height of the frame about to be drawn.
    ///
    /// Called by the renderer rather than set by the caller, so a terminal
    /// resize needs no event of its own: the next frame simply has a different
    /// height, and page motions immediately agree with what is on screen.
    pub fn set_height(&mut self, height: u32, view_lines: u32) {
        self.height = height;
        self.clamp(view_lines);
    }

    /// The half-open range of view lines the next frame will show.
    pub fn visible(&self, view_lines: u32) -> std::ops::Range<u32> {
        self.top..(self.top + self.height).min(view_lines)
    }

    /// Applies a generic motion.
    ///
    /// The count means what it means in vim, which is not the same thing for
    /// every motion: `5j` is five downs, while `5G` is view line five.
    pub fn motion(&mut self, motion: Motion, count: u32, view_lines: u32) {
        let page = self.height.saturating_sub(2).max(1);
        // A count naming a row is 1-based on screen and 0-based here. `count`
        // is 1 when none was typed, which is why `Top` and `Bottom` check.
        let named = (count > 1).then(|| count - 1);

        match motion {
            Motion::Down => self.cursor = self.cursor.saturating_add(count),
            Motion::Up => self.cursor = self.cursor.saturating_sub(count),
            Motion::PageDown => {
                self.cursor = self.cursor.saturating_add(page.saturating_mul(count));
            }
            Motion::PageUp => {
                self.cursor = self.cursor.saturating_sub(page.saturating_mul(count));
            }
            Motion::Top => self.cursor = named.unwrap_or(0),
            Motion::Bottom => self.cursor = named.unwrap_or(view_lines.saturating_sub(1)),
            Motion::ScrollRight => self.left = self.left.saturating_add(count * SCROLL_STEP),
            Motion::ScrollLeft => self.left = self.left.saturating_sub(count * SCROLL_STEP),
        }
        self.clamp(view_lines);
    }

    /// Puts the cursor on a given row and centres on it.
    ///
    /// For a jump whose target was worked out elsewhere — today, the row a
    /// line moved to when the layout changed. Centring rather than
    /// preserving the offset because after a change of layout the rows around
    /// the cursor are not the rows that were there before, so there is no
    /// offset worth preserving.
    pub fn jump(&mut self, view_line: u32, view_lines: u32) {
        self.cursor = view_line;
        self.centre();
        self.clamp(view_lines);
    }

    /// Moves `count` steps, asking `next` where each one lands.
    ///
    /// How a buffer contributes a motion whose targets only it knows —
    /// stepping through changed blocks now, and search matches or review marks
    /// later. The viewport still does the moving.
    ///
    /// Reports whether it moved at all, so a buffer that has run out of
    /// targets can say so rather than leaving the reader unsure whether the
    /// key was even bound. A partial step still counts as moving: `5]c` with
    /// two changes left takes both, which is what vim does.
    pub fn step(&mut self, count: u32, view_lines: u32, next: impl Fn(u32) -> Option<u32>) -> bool {
        let mut moved = false;
        for _ in 0..count {
            match next(self.cursor) {
                Some(line) => {
                    self.cursor = line;
                    moved = true;
                }
                None => break,
            }
        }
        if moved {
            self.centre();
        }
        self.clamp(view_lines);
        moved
    }

    /// Puts the cursor row in the middle of the screen.
    ///
    /// What the plugin does after change navigation: landing on a change with
    /// no keymap_type above it reads as though the file starts there.
    fn centre(&mut self) {
        self.top = self.cursor.saturating_sub(self.height / 2);
    }

    /// Brings every field back into range, and the cursor back into view.
    ///
    /// One place, called after every change, rather than each motion having to
    /// remember its own bounds.
    fn clamp(&mut self, view_lines: u32) {
        self.cursor = self.cursor.min(view_lines.saturating_sub(1));

        if self.height == 0 {
            self.top = 0;
            return;
        }

        // Never scroll past the end, unless the document is shorter than the
        // screen, in which case the top is zero and there is nothing to do.
        let last_top = view_lines.saturating_sub(self.height);

        // A margin, shrinking on a short screen so it cannot exceed half of it
        // and fight itself.
        let margin = SCROLLOFF.min(self.height.saturating_sub(1) / 2);

        if self.cursor < self.top + margin {
            self.top = self.cursor.saturating_sub(margin);
        }
        if self.cursor + margin >= self.top + self.height {
            self.top = (self.cursor + margin + 1).saturating_sub(self.height);
        }
        self.top = self.top.min(last_top);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn view(height: u32) -> Viewport {
        let mut v = Viewport::new();
        v.set_height(height, 100);
        v
    }

    #[test]
    fn one_position_serves_every_column() {
        // Not a test of behaviour so much as a test that there is only one
        // thing to test. A second scroll field would make `top()` stop being
        // the whole answer, and this would stop compiling as written.
        let mut v = view(10);
        v.motion(Motion::Down, 20, 100);
        assert_eq!(v.top(), 14);
        assert_eq!(v.cursor(), 20);
    }

    #[test]
    fn the_cursor_cannot_leave_the_document() {
        let mut v = view(5);
        v.motion(Motion::Down, 1000, 10);
        assert_eq!(v.cursor(), 9);
        v.motion(Motion::Up, 1000, 10);
        assert_eq!(v.cursor(), 0);
    }

    #[test]
    fn a_document_shorter_than_the_screen_never_scrolls() {
        let mut v = view(40);
        v.motion(Motion::Bottom, 1, 3);
        assert_eq!(v.top(), 0);
        assert_eq!(v.visible(3), 0..3);
    }

    #[test]
    fn the_end_of_the_document_sits_at_the_bottom() {
        let mut v = view(10);
        v.motion(Motion::Bottom, 1, 100);
        assert_eq!(v.cursor(), 99);
        assert_eq!(v.visible(100), 90..100);
    }

    #[test]
    fn a_count_on_g_names_a_row_rather_than_repeating() {
        // vim's rule, and the reason the count is interpreted per motion
        // rather than folded in by the resolver: `5G` is row five, not five
        // bottoms.
        let mut v = view(10);
        v.motion(Motion::Bottom, 5, 100);
        assert_eq!(v.cursor(), 4, "1-based on screen, 0-based here");
        v.motion(Motion::Top, 40, 100);
        assert_eq!(v.cursor(), 39);
    }

    #[test]
    fn a_margin_is_kept_below_the_cursor() {
        let mut v = view(10);
        v.motion(Motion::Down, 7, 100);
        assert_eq!(v.cursor(), 7);
        assert_eq!(v.top(), 1, "cursor is 3 rows off the bottom");
    }

    #[test]
    fn the_margin_shrinks_rather_than_fighting_a_tiny_screen() {
        let mut v = view(2);
        v.motion(Motion::Down, 50, 100);
        assert!(v.visible(100).contains(&v.cursor()));
    }

    #[test]
    fn a_page_is_the_screen_less_two_rows_of_overlap() {
        let mut v = view(20);
        v.motion(Motion::PageDown, 1, 1000);
        assert_eq!(v.cursor(), 18);
        v.motion(Motion::PageDown, 3, 1000);
        assert_eq!(v.cursor(), 18 + 54);
    }

    #[test]
    fn horizontal_scrolling_moves_by_whole_steps() {
        let mut v = view(10);
        v.motion(Motion::ScrollRight, 1, 10);
        assert_eq!(v.left(), SCROLL_STEP);
        v.motion(Motion::ScrollRight, 5, 10);
        assert_eq!(v.left(), SCROLL_STEP * 6);
        v.motion(Motion::ScrollLeft, 1, 10);
        assert_eq!(v.left(), SCROLL_STEP * 5);
    }

    #[test]
    fn stepping_asks_the_buffer_where_to_go_and_centres_there() {
        let starts = [40u32, 70];
        let mut v = view(20);
        v.step(1, 100, |from| starts.iter().copied().find(|&r| r > from));
        assert_eq!(v.cursor(), 40);
        assert_eq!(v.top(), 30, "centred");
    }

    #[test]
    fn a_count_steps_that_many_times() {
        let starts = [10u32, 40, 70];
        let mut v = view(20);
        v.step(2, 100, |from| starts.iter().copied().find(|&r| r > from));
        assert_eq!(v.cursor(), 40);
    }

    #[test]
    fn stepping_past_the_last_target_stays_put() {
        let starts = [40u32];
        let mut v = view(20);
        for _ in 0..2 {
            v.step(1, 100, |from| starts.iter().copied().find(|&r| r > from));
        }
        assert_eq!(v.cursor(), 40);
    }

    #[test]
    fn a_resize_re_examines_the_scroll_position() {
        let mut v = view(40);
        v.motion(Motion::Bottom, 1, 100);
        assert_eq!(v.top(), 60);
        v.set_height(10, 100);
        assert_eq!(v.top(), 90, "still showing the end");
        assert!(v.visible(100).contains(&v.cursor()));
    }
}
