//! Tests for the Filler component.

use std::rc::Rc;

use loom::testing::Harness;
use ui::Theme;
use ui::components::filler::{Filler, FillerProps};
use ui::components::{Context, Ui};

fn filler(width: u16) -> Harness {
    Harness::new::<Filler>(FillerProps {}, width, 1)
        .provide::<Ui>(Context {
            theme: Rc::new(Theme::DARK),
            ..Context::default()
        })
}

#[test]
fn every_cell_is_the_hatch_character() {
    let mut h = filler(10);
    h.draw();
    let row = h.screen_row(0);
    for ch in row.chars() {
        assert_eq!(ch, '╱', "every cell is the hatch: {:?}", row);
    }
}

#[test]
fn a_filler_is_one_row_tall() {
    let mut h = filler(10);
    h.draw();
    let row0 = h.screen_row(0);
    assert!(!row0.is_empty(), "the filler drew something");
}

#[test]
fn the_filler_uses_the_filler_style() {
    let mut h = filler(10);
    h.draw();
    let style = h.style_at(0, 0);
    let theme = Theme::DARK;
    let expected = theme.normal.patch(theme.filler);
    assert_eq!(style.bg, expected.bg, "the filler background matches the theme");
}
