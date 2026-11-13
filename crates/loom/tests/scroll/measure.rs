use std::cell::Cell;
use std::rc::Rc;

use loom::testing::Harness;
use loom::{
    Basis, Column, ColumnProps, Layout, Node, Scope, Scroll, ScrollOffset, ScrollProps, Size,
    component, rsx, use_layout_effect, use_measure, use_scroll,
};

#[component]
fn MeasuredContent(scope: &mut Scope, measured: Rc<Cell<Size>>) -> Node {
    let (node_ref, size) = use_measure(scope);
    let measured = Rc::clone(measured);
    use_layout_effect(scope, size, move || measured.set(size));
    rsx! {
        Column {
            ref: Some(node_ref),
            layout: Layout { basis: Basis::Length(20), min_width: 30, shrink: 0, ..Default::default() },
            ..,
            "measured content"
        }
    }
}

#[component]
fn MeasuringScroll(scope: &mut Scope, measured: Rc<Cell<Size>>) -> Node {
    let (view, handle) = use_scroll(scope, || ScrollOffset::ZERO);
    rsx! {
        Scroll {
            view,
            handle: Some(handle),
            wheel_step: 3,
            layout: Layout { grow: 1, ..Default::default() },
            ..,
            MeasuredContent { measured: Rc::clone(measured) }
        }
    }
}

#[test]
fn scrolling_does_not_change_a_childs_measured_size() {
    let measured = Rc::new(Cell::new(Size::default()));
    let mut h = Harness::new::<MeasuringScroll>(
        MeasuringScrollProps {
            measured: Rc::clone(&measured),
        },
        10,
        4,
    );
    h.force_draw().force_draw();
    let expected = Size {
        width: 30,
        height: 20,
    };
    assert_eq!(measured.get(), expected);

    for _ in 0..6 {
        h.wheel(1, 1, 1).force_draw();
        h.wheel_horizontal(1, 1, 1).force_draw();
        assert_eq!(measured.get(), expected, "scrolling changed layout size");
    }
    h.resize(12, 6).force_draw();
    assert_eq!(
        measured.get(),
        expected,
        "resizing changed fixed content size"
    );
    h.resize(40, 6).force_draw();
    assert_eq!(
        measured.get(),
        Size {
            width: 40,
            height: 20
        }
    );
    h.resize(10, 4).force_draw();
    assert_eq!(measured.get(), expected);
    for _ in 0..6 {
        h.wheel(1, 1, -1).force_draw();
        h.wheel_horizontal(1, 1, -1).force_draw();
        assert_eq!(measured.get(), expected);
    }
}
