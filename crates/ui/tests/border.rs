//! Tests for the Border component.

use std::rc::Rc;

use loom::testing::Harness;
use loom::{
    Column, ColumnProps, Layout, Node, NodeHandle, Scope, Text, TextProps, component, rsx,
    use_layout_effect, use_ref,
};
use ui::Theme;
use ui::components::border::{Border, BorderProps};
use ui::components::{Context, Ui};

#[component]
fn Inner(scope: &mut Scope) -> Node {
    let _ = scope;
    rsx! { Text { text: "hi".into(), .. } }
}

/// A child that takes focus on mount.
#[component]
fn Focusable(scope: &mut Scope) -> Node {
    let self_ref = use_ref(scope, || None::<NodeHandle>);
    use_layout_effect(scope, (), move || {
        if let Some(node) = self_ref.current().as_ref() {
            node.focus();
        }
    });
    rsx! {
        Column {
            ref: Some(self_ref),
            focusable: true,
            layout: Layout { grow: 1, ..Default::default() },
            ..,
            Text { text: "hi".into(), .. }
        }
    }
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
            layout: Layout {
                grow: 1,
                ..Default::default()
            },
            children: vec![rsx! { Inner {} }],
        },
        10,
        5,
    )
    .provide::<Ui>(ctx());
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
    // A border with a focusable child that takes focus on mount.
    let mut focused = Harness::new::<Border>(
        BorderProps {
            layout: Layout {
                grow: 1,
                ..Default::default()
            },
            children: vec![rsx! { Focusable {} }],
        },
        10,
        5,
    )
    .provide::<Ui>(ctx());
    for _ in 0..4 {
        focused.force_draw();
    }

    // A border with no focusable child.
    let mut unfocused = Harness::new::<Border>(
        BorderProps {
            layout: Layout {
                grow: 1,
                ..Default::default()
            },
            children: vec![rsx! { Inner {} }],
        },
        10,
        5,
    )
    .provide::<Ui>(ctx());
    unfocused.draw();

    let focused_corner = focused.style_at(0, 0);
    let unfocused_corner = unfocused.style_at(0, 0);
    assert_ne!(
        focused_corner.fg, unfocused_corner.fg,
        "focused and unfocused borders have different colours"
    );
}
