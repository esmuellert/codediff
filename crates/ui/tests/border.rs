//! Tests for the Border component.

use std::rc::Rc;

use loom::testing::Harness;
use loom::{Layout, Node, Scope, Text, TextProps, component, rsx};
use ui::Theme;
use ui::components::border::{Border, BorderProps};
use ui::components::{Context, Ui};

#[component]
fn Inner(scope: &mut Scope) -> Node {
    let _ = scope;
    rsx! { Text { text: "hi".into(), .. } }
}

fn ctx() -> Context {
    Context {
        theme: Rc::new(Theme::DARK),
        ..Context::default()
    }
}

#[test]
fn a_border_draws_rounded_corners() {
    let mut h = Harness::new::<Border>(
        BorderProps {
            focused: false,
            layout: Layout { grow: 1, ..Default::default() },
            children: vec![rsx! { Inner {} }],
        },
        10, 5,
    ).provide::<Ui>(ctx());
    h.draw();
    let top = h.screen_row(0);
    let bottom = h.screen_row(4);
    assert!(top.contains('╭'), "top left corner: {:?}", top);
    assert!(top.contains('╮'), "top right corner: {:?}", top);
    assert!(bottom.contains('╰'), "bottom left corner: {:?}", bottom);
    assert!(bottom.contains('╯'), "bottom right corner: {:?}", bottom);
}

#[test]
fn a_focused_border_has_a_different_colour() {
    let mut focused = Harness::new::<Border>(
        BorderProps {
            focused: true,
            layout: Layout { grow: 1, ..Default::default() },
            children: vec![rsx! { Inner {} }],
        },
        10, 5,
    ).provide::<Ui>(ctx());
    focused.draw();

    let mut unfocused = Harness::new::<Border>(
        BorderProps {
            focused: false,
            layout: Layout { grow: 1, ..Default::default() },
            children: vec![rsx! { Inner {} }],
        },
        10, 5,
    ).provide::<Ui>(ctx());
    unfocused.draw();

    let focused_corner = focused.style_at(0, 0);
    let unfocused_corner = unfocused.style_at(0, 0);
    assert_ne!(focused_corner.fg, unfocused_corner.fg,
        "focused and unfocused borders have different colours");
}
