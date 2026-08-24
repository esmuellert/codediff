//! The invariants of §13, each written by breaking the code on purpose and
//! watching it fail.

use std::cell::RefCell;
use std::rc::Rc;

use loom::testing::Harness;
use loom::{
    Basis, Bubble, Canvas, CanvasProps, Column, ColumnProps, Layout, Listeners, Node, Row, RowProps,
    Scope, Text, TextProps, component, rsx, use_effect, use_memo, use_ref, use_state,
};

#[component]
fn Hello(scope: &mut Scope) -> Node {
    let _ = scope;
    rsx! { "hello" }
}

#[test]
fn a_component_paints_its_text() {
    let mut screen = Harness::new::<Hello>(HelloProps {}, 10, 1);
    assert_eq!(screen.screen_row(0), "hello");
}

#[component]
fn Two(scope: &mut Scope) -> Node {
    let _ = scope;
    rsx! {
        Row {
            Text { text: "ab".into(), layout: Layout { basis: Basis::Length(2), ..Default::default() }, .. }
            Text { text: "cd".into(), layout: Layout { basis: Basis::Length(2), ..Default::default() }, .. }
        }
    }
}

#[test]
fn a_row_places_its_children_across() {
    let mut screen = Harness::new::<Two>(TwoProps {}, 10, 1);
    assert_eq!(screen.screen_row(0), "abcd");
}

#[component]
fn Stacked(scope: &mut Scope) -> Node {
    let _ = scope;
    rsx! {
        Column {
            "top"
            "bottom"
        }
    }
}

#[test]
fn a_column_places_its_children_down() {
    let mut screen = Harness::new::<Stacked>(StackedProps {}, 10, 2);
    assert_eq!(screen.screen(), vec!["top", "bottom"]);
}

/// A component that counts up when a key arrives, so a test can drive it.
/// The canvas asks for one row, because a canvas measures as nothing.
#[component]
fn Counter(scope: &mut Scope) -> Node {
    let (n, set) = use_state(scope, || 0u32);
    let text: Rc<str> = format!("n={n}").into();
    rsx! {
        Canvas {
            layout: Layout { basis: Basis::Length(1), ..Default::default() },
            focusable: true,
            listeners: Listeners::new().on_key(move |_| {
                set(&|n| n + 1);
                Bubble::Stop
            }),
            paint: {
                let text = text.clone();
                Rc::new(move |brush: &mut loom::Paint<'_>| {
                    let area = brush.area();
                    brush.write(area.x, area.y, &text, Default::default());
                })
            },
            ..
        }
    }
}

#[test]
fn a_component_reads_its_own_state() {
    let mut screen = Harness::new::<Counter>(CounterProps {}, 10, 1);
    assert_eq!(screen.screen_row(0), "n=0");
}

/// I1 — the screen is a function of state.
#[test]
fn two_draws_with_nothing_between_them_agree() {
    let mut screen = Harness::new::<Counter>(CounterProps {}, 10, 1);
    let first = screen.screen();
    let second = screen.force_draw().screen();
    assert_eq!(first, second);
}

#[test]
fn a_key_reaches_the_focused_node_and_state_moves() {
    let mut screen = Harness::new::<Counter>(CounterProps {}, 10, 1);
    screen.draw();
    screen.click(0, 0);
    screen.press(crokey::key!(a));
    assert_eq!(screen.screen_row(0), "n=1");
    screen.press(crokey::key!(a));
    assert_eq!(screen.screen_row(0), "n=2");
}

/// I2 — state survives a re-render of the parent.
#[component]
fn Parent(scope: &mut Scope, tag: u32) -> Node {
    let _ = scope;
    let text: Rc<str> = format!("tag={tag}").into();
    rsx! {
        Column {
            Text { text: text, .. }
            Counter {}
        }
    }
}

#[test]
fn a_component_at_the_same_place_keeps_its_state() {
    let mut screen = Harness::new::<Parent>(ParentProps { tag: 1 }, 12, 2);
    screen.draw();
    screen.click(0, 1);
    screen.press(crokey::key!(a));
    assert_eq!(screen.screen_row(1), "n=1");

    // The parent renders again with new props; the child's state stands.
    screen.set_props::<Parent>(ParentProps { tag: 2 });
    assert_eq!(screen.screen_row(0), "tag=2");
    assert_eq!(screen.screen_row(1), "n=1");
}

/// I3 — a different component at the same place starts fresh.
#[component]
fn Swap(scope: &mut Scope, other: bool) -> Node {
    let _ = scope;
    rsx! {
        if *other {
            Hello {}
        } else {
            Counter {}
        }
    }
}

#[test]
fn a_different_component_at_the_same_place_starts_fresh() {
    let mut screen = Harness::new::<Swap>(SwapProps { other: false }, 10, 1);
    screen.draw();
    screen.click(0, 0);
    screen.press(crokey::key!(a));
    assert_eq!(screen.screen_row(0), "n=1");

    screen.set_props::<Swap>(SwapProps { other: true });
    assert_eq!(screen.screen_row(0), "hello");

    // Back again: a fresh Counter, because Hello stood in its place.
    screen.set_props::<Swap>(SwapProps { other: false });
    assert_eq!(screen.screen_row(0), "n=0");
}

/// I11 — an effect's cleanup runs before its next setup.
#[component]
fn Effecting(scope: &mut Scope, tag: u32, log: Rc<RefCell<Vec<String>>>) -> Node {
    let tag = *tag;
    let log = log.clone();
    use_effect(scope, tag, move || {
        log.borrow_mut().push(format!("setup {tag}"));
        move || log.borrow_mut().push(format!("cleanup {tag}"))
    });
    rsx! { "x" }
}

#[test]
fn a_cleanup_runs_before_the_next_setup() {
    let log = Rc::new(RefCell::new(Vec::new()));
    let mut screen =
        Harness::new::<Effecting>(EffectingProps { tag: 1, log: log.clone() }, 4, 1);
    screen.draw();
    assert_eq!(&*log.borrow(), &["setup 1"]);

    screen.set_props::<Effecting>(EffectingProps { tag: 2, log: log.clone() });
    screen.draw();
    assert_eq!(&*log.borrow(), &["setup 1", "cleanup 1", "setup 2"]);
}

/// An effect with `()` deps runs once, however many frames pass.
#[component]
fn Once(scope: &mut Scope, log: Rc<RefCell<Vec<String>>>) -> Node {
    let log = log.clone();
    use_effect(scope, (), move || log.borrow_mut().push("once".into()));
    rsx! { "x" }
}

#[test]
fn an_effect_with_no_deps_runs_once() {
    let log = Rc::new(RefCell::new(Vec::new()));
    let mut screen = Harness::new::<Once>(OnceProps { log: log.clone() }, 4, 1);
    screen.draw();
    screen.force_draw();
    screen.force_draw();
    assert_eq!(log.borrow().len(), 1);
}

/// A memo hands back the same `Rc` while its deps hold still.
#[component]
fn Memoing(scope: &mut Scope, tag: u32, seen: Rc<RefCell<Vec<usize>>>) -> Node {
    let value = use_memo(scope, *tag, || format!("computed {tag}"));
    seen.borrow_mut().push(Rc::as_ptr(&value) as usize);
    rsx! { "x" }
}

#[test]
fn a_memo_keeps_its_value_while_its_deps_hold_still() {
    let seen = Rc::new(RefCell::new(Vec::new()));
    let mut screen = Harness::new::<Memoing>(MemoingProps { tag: 1, seen: seen.clone() }, 4, 1);
    screen.draw();
    screen.set_props::<Memoing>(MemoingProps { tag: 1, seen: seen.clone() });
    screen.draw();
    let seen = seen.borrow();
    assert_eq!(seen.len(), 2);
    assert_eq!(seen[0], seen[1], "the same deps gave back the same Rc");
}

#[test]
fn a_memo_recomputes_when_its_deps_change() {
    let seen = Rc::new(RefCell::new(Vec::new()));
    let mut screen = Harness::new::<Memoing>(MemoingProps { tag: 1, seen: seen.clone() }, 4, 1);
    screen.draw();
    screen.set_props::<Memoing>(MemoingProps { tag: 2, seen: seen.clone() });
    screen.draw();
    let seen = seen.borrow();
    assert_ne!(seen[0], seen[1], "new deps gave back a new Rc");
}

/// A ref survives a render and does not cause one.
#[component]
fn Reffing(scope: &mut Scope, log: Rc<RefCell<Vec<u32>>>) -> Node {
    let held = use_ref(scope, || 0u32);
    *held.current() += 1;
    log.borrow_mut().push(*held.current());
    rsx! { "x" }
}

#[test]
fn a_ref_survives_a_render() {
    let log = Rc::new(RefCell::new(Vec::new()));
    let mut screen = Harness::new::<Reffing>(ReffingProps { log: log.clone() }, 4, 1);
    screen.draw();
    screen.set_props::<Reffing>(ReffingProps { log: log.clone() });
    screen.draw();
    assert_eq!(&*log.borrow(), &[1, 2]);
}

/// I5 — every rectangle handed to a child lies inside its parent's.
#[component]
fn Overflowing(scope: &mut Scope) -> Node {
    let _ = scope;
    rsx! {
        Column {
            layout: Layout { pad: loom::Edges::all(1), ..Default::default() },
            ..,
            Row {
                // A minimum wider than the column has room for.
                layout: Layout { min_width: 40, ..Default::default() },
                ..,
                Text { text: "inner".into(), .. }
            }
        }
    }
}

#[test]
fn every_child_rectangle_is_inside_its_parent() {
    let mut screen = Harness::new::<Overflowing>(OverflowingProps {}, 20, 5);
    screen.draw();
    let text = screen.tree_text();
    // 20 wide less one cell of padding each side, whatever the row asked for.
    assert!(text.contains("Row 18x1+1+1"), "{text}");
}

/// The same thing seen from the padded side: a child never starts before its
/// parent's inner edge.
#[component]
fn Nested(scope: &mut Scope) -> Node {
    let _ = scope;
    rsx! {
        Column {
            layout: Layout { pad: loom::Edges::all(1), ..Default::default() },
            ..,
            Row {
                Text { text: "inner".into(), .. }
            }
        }
    }
}

#[test]
fn padding_comes_off_before_the_children() {
    let mut screen = Harness::new::<Nested>(NestedProps {}, 20, 5);
    screen.draw();
    let text = screen.tree_text();
    assert!(text.contains("Row 18x1+1+1"), "{text}");
}

/// I7 — children tile the container in order.
#[test]
fn children_tile_the_container_in_order() {
    let mut screen = Harness::new::<Two>(TwoProps {}, 10, 1);
    assert_eq!(screen.screen_row(0), "abcd");
}

/// A key names a child wherever it moved to.
#[component]
fn Keyed(scope: &mut Scope, order: Vec<u32>) -> Node {
    let _ = scope;
    rsx! {
        Column {
            for n in order.clone() {
                Tagged { key: n, tag: n }
            }
        }
    }
}

#[component]
fn Tagged(scope: &mut Scope, tag: u32) -> Node {
    let (start, _) = use_state(scope, || *tag);
    let text: Rc<str> = format!("{tag}:{start}").into();
    rsx! { Text { text: text, .. } }
}

#[test]
fn a_keyed_child_keeps_its_state_when_the_list_reorders() {
    let mut screen = Harness::new::<Keyed>(KeyedProps { order: vec![1, 2, 3] }, 10, 3);
    assert_eq!(screen.screen(), vec!["1:1", "2:2", "3:3"]);

    screen.set_props::<Keyed>(KeyedProps { order: vec![3, 1, 2] });
    // Each row kept the state it mounted with, so the pairs still match.
    assert_eq!(screen.screen(), vec!["3:3", "1:1", "2:2"]);
}

/// A child that writes state is reached even when everything above it is
/// clean. Without marking the path to the root, the root hands back last
/// frame's subtree and the write never reaches the screen.
#[component]
fn Quiet(scope: &mut Scope) -> Node {
    let _ = scope;
    rsx! {
        Column {
            Deep {}
        }
    }
}

#[component]
fn Deep(scope: &mut Scope) -> Node {
    let (n, set) = use_state(scope, || 0u32);
    // The first frame's effect writes state; the second must show it.
    use_effect(scope, (), move || set(&|_| 42));
    let text: Rc<str> = format!("n={n}").into();
    rsx! { Text { text: text, .. } }
}

#[test]
fn a_write_below_a_clean_parent_still_reaches_the_screen() {
    let mut screen = Harness::new::<Quiet>(QuietProps {}, 10, 1);
    assert_eq!(screen.screen_row(0), "n=0");
    // The effect ran after that paint; the next draw shows what it wrote.
    screen.draw();
    assert_eq!(screen.screen_row(0), "n=42");
}
