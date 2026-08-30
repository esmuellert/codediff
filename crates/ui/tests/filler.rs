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

#[test]
fn a_filler_beside_a_gutter_fills_the_remaining_width() {
    use loom::{Node, Scope, component, rsx, Row, RowProps, Layout};
    use ui::components::gutter::{Gutter, GutterProps};

    #[component]
    fn GutterAndFiller(scope: &mut Scope) -> Node {
        let theme = loom::use_context::<Ui>(scope).theme;
        let gutter_style = theme.normal.patch(theme.line_number);
        rsx! {
            Row {
                layout: Layout { grow: 1, ..Default::default() },
                ..,
                Gutter {
                    key: 0u32,
                    number: None,
                    style: gutter_style,
                    blank: gutter_style,
                    width: 4,
                }
                Filler { key: 1u32 }
            }
        }
    }

    let mut h = Harness::new::<GutterAndFiller>(
        GutterAndFillerProps {},
        20, 1,
    )
    .provide::<Ui>(Context {
        theme: Rc::new(Theme::DARK),
        ..Context::default()
    });
    for _ in 0..3 { h.force_draw(); }
    let row = h.screen_row(0);
    let hatches = row.chars().filter(|&c| c == '╱').count();
    assert!(
        hatches > 1,
        "the filler should fill the space after the gutter, got {} hatches in {:?}",
        hatches, row
    );
}
