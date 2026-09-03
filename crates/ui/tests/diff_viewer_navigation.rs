use std::rc::Rc;

use loom::testing::Harness;
use loom::{
    Basis, Column, ColumnProps, Layout, Node, Row, RowProps, Scope, Text, TextProps, component, rsx,
};
use ui::hooks::use_diff_viewer_navigation::use_diff_viewer_navigation;
use ui::hooks::use_horizontal_scroll::HorizontalDimensions;

#[component]
fn Probe(
    scope: &mut Scope,
    file_key: Rc<str>,
    total: u32,
    longest_line_cells: u32,
    auto_focus: bool,
) -> Node {
    let (view, horizontal, listeners) = use_diff_viewer_navigation(
        scope,
        Some(file_key),
        *total,
        HorizontalDimensions::Single {
            longest_line_cells: *longest_line_cells,
            gutter_cells: 0,
        },
    );
    let state: Rc<str> = format!(
        "{} {} {}",
        view.cursor, view.top, horizontal.requested_first_cell
    )
    .into();
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
    navigation_harness(key, 20, 40)
}

fn navigation_harness(key: &str, width: u16, longest_line_cells: u32) -> Harness {
    let mut harness = Harness::new::<Probe>(
        ProbeProps {
            file_key: key.into(),
            total: 20,
            longest_line_cells,
            auto_focus: true,
        },
        width,
        4,
    );
    harness.force_draw().force_draw();
    harness
}

fn state(harness: &mut Harness) -> (u32, u32, u32) {
    let row = harness.screen_row(0);
    let mut values = row.split_whitespace().map(|value| value.parse().unwrap());
    (
        values.next().unwrap(),
        values.next().unwrap(),
        values.next().unwrap(),
    )
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
    assert_eq!(state(&mut harness), (0, 3, 0));
}

#[test]
fn click_moves_the_current_row() {
    let mut harness = harness("a.rs");
    harness.click(1, 2).force_draw();
    assert_eq!(state(&mut harness).0, 2);
}

#[test]
fn h_l_zero_and_dollar_move_the_horizontal_position() {
    let mut harness = harness("a.rs");
    harness.press(crokey::key!(h)).force_draw();
    assert_eq!(state(&mut harness).2, 0);
    for _ in 0..3 {
        harness.press(crokey::key!(l)).force_draw();
    }
    assert_eq!(state(&mut harness).2, 3);
    harness.press(crokey::key!(h)).force_draw();
    assert_eq!(state(&mut harness).2, 2);
    harness.press(crokey::key!(0)).force_draw();
    assert_eq!(state(&mut harness).2, 0);
    harness.press(crokey::key!('$')).force_draw();
    assert_eq!(state(&mut harness).2, 24);
}

#[test]
fn repeated_horizontal_keys_compose_before_a_draw() {
    let mut harness = harness("a.rs");
    harness
        .press(crokey::key!(l))
        .press(crokey::key!(l))
        .press(crokey::key!(l))
        .force_draw();

    assert_eq!(state(&mut harness).2, 3);
}

#[test]
fn repeated_vertical_keys_compose_before_a_draw() {
    let mut harness = harness("a.rs");
    harness
        .press(crokey::key!(j))
        .press(crokey::key!(j))
        .press(crokey::key!(j))
        .force_draw();

    assert_eq!(state(&mut harness).0, 3);
}

#[test]
fn horizontal_position_stops_at_the_vscode_endpoint() {
    let mut harness = navigation_harness("a.rs", 10, 20);
    for _ in 0..20 {
        harness.press(crokey::key!(l)).force_draw();
    }
    assert_eq!(state(&mut harness).2, 14);
}

#[test]
fn a_line_narrower_than_the_viewport_does_not_scroll() {
    let mut harness = navigation_harness("a.rs", 20, 10);
    harness.press(crokey::key!(l)).force_draw();
    assert_eq!(state(&mut harness).2, 0);
}

#[test]
fn resizing_clamps_without_forgetting_the_requested_position() {
    let mut harness = navigation_harness("a.rs", 10, 20);
    for _ in 0..6 {
        harness.press(crokey::key!(l)).force_draw();
    }
    assert_eq!(state(&mut harness).2, 6);

    harness.resize(30, 4).force_draw().force_draw();
    assert_eq!(state(&mut harness).2, 0);

    harness.resize(10, 4).force_draw().force_draw();
    assert_eq!(state(&mut harness).2, 6);
}

#[test]
fn changing_files_restores_each_position() {
    let mut harness = harness("a.rs");
    for _ in 0..8 {
        harness.press(crokey::key!(j)).force_draw();
    }
    for _ in 0..5 {
        harness.press(crokey::key!(l)).force_draw();
    }
    let saved = state(&mut harness);
    assert_ne!(saved, (0, 0, 0));

    harness.set_props::<Probe>(ProbeProps {
        file_key: "b.rs".into(),
        total: 20,
        longest_line_cells: 40,
        auto_focus: true,
    });
    harness.force_draw().force_draw();
    assert_eq!(state(&mut harness), (0, 0, 0));
    for _ in 0..2 {
        harness.press(crokey::key!(l)).force_draw();
    }
    assert_eq!(state(&mut harness).2, 2);

    harness.set_props::<Probe>(ProbeProps {
        file_key: "a.rs".into(),
        total: 20,
        longest_line_cells: 40,
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
            Probe { file_key: "a.rs".into(), total: 20, longest_line_cells: 40, auto_focus: true }
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
