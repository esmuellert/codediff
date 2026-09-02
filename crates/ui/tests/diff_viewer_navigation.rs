use std::rc::Rc;

use loom::testing::Harness;
use loom::{
    Basis, Column, ColumnProps, Layout, Node, Row, RowProps, Scope, Text, TextProps, component, rsx,
};
use ui::hooks::use_diff_viewer_navigation::use_diff_viewer_navigation;

#[component]
fn Probe(scope: &mut Scope, file_key: Rc<str>, total: u32, auto_focus: bool) -> Node {
    let (view, listeners) = use_diff_viewer_navigation(scope, Some(file_key), *total);
    let state: Rc<str> = format!("{} {}", view.cursor, view.top).into();
    rsx! {
        Column {
            ref: Some(view.node_ref),
            focusable: true,
            auto_focus: *auto_focus,
            listeners: listeners,
            layout: Layout { grow: 1, ..Default::default() },
            ..,
            Text { text: state, .. }
        }
    }
}

fn harness(key: &str) -> Harness {
    let mut harness = Harness::new::<Probe>(
        ProbeProps {
            file_key: key.into(),
            total: 20,
            auto_focus: true,
        },
        20,
        4,
    );
    harness.force_draw().force_draw();
    harness
}

fn state(harness: &mut Harness) -> (u32, u32) {
    let row = harness.screen_row(0);
    let mut values = row.split_whitespace().map(|value| value.parse().unwrap());
    (values.next().unwrap(), values.next().unwrap())
}

#[test]
fn j_and_k_move_the_current_row() {
    let mut harness = harness("a.rs");
    harness.press(crokey::key!(j)).force_draw();
    assert_eq!(state(&mut harness).0, 1);
    harness.press(crokey::key!(k)).force_draw();
    assert_eq!(state(&mut harness).0, 0);
}

#[test]
fn wheel_moves_only_the_view() {
    let mut harness = harness("a.rs");
    harness.wheel(1, 1, 1).force_draw();
    assert_eq!(state(&mut harness), (0, 3));
}

#[test]
fn click_moves_the_current_row() {
    let mut harness = harness("a.rs");
    harness.click(1, 2).force_draw();
    assert_eq!(state(&mut harness).0, 2);
}

#[test]
fn changing_files_restores_each_position() {
    let mut harness = harness("a.rs");
    for _ in 0..8 {
        harness.press(crokey::key!(j)).force_draw();
    }
    let saved = state(&mut harness);
    assert_ne!(saved, (0, 0));

    harness.set_props::<Probe>(ProbeProps {
        file_key: "b.rs".into(),
        total: 20,
        auto_focus: true,
    });
    harness.force_draw().force_draw();
    assert_eq!(state(&mut harness), (0, 0));

    harness.set_props::<Probe>(ProbeProps {
        file_key: "a.rs".into(),
        total: 20,
        auto_focus: true,
    });
    harness.force_draw().force_draw();
    assert_eq!(state(&mut harness), saved);
}

#[component]
fn Previous(scope: &mut Scope) -> Node {
    let _ = scope;
    rsx! {
        Column {
            focusable: true,
            layout: Layout { basis: Basis::Length(1), ..Default::default() },
            ..,
            Text { text: "previous".into(), .. }
        }
    }
}

#[component]
fn FocusPair(scope: &mut Scope) -> Node {
    let _ = scope;
    rsx! {
        Row {
            layout: Layout { grow: 1, ..Default::default() },
            ..,
            Previous {}
            Probe { file_key: "a.rs".into(), total: 20, auto_focus: true }
        }
    }
}

#[test]
fn left_focuses_the_previous_view() {
    let mut harness = Harness::new::<FocusPair>(FocusPairProps {}, 20, 4);
    harness.force_draw().force_draw();
    assert_eq!(harness.focused_name(), Some("Probe"));

    harness.press(crokey::key!(left)).force_draw();

    assert_eq!(harness.focused_name(), Some("Previous"));
}
