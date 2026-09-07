use std::rc::Rc;

use loom::testing::Harness;
use loom::{
    Basis, Column, ColumnProps, Layout, Node, Row, RowProps, Scope, Text, TextProps, component, rsx,
};
use ui::hooks::use_diff_viewer_navigation::use_diff_viewer_navigation;
use ui::hooks::use_horizontal_scroll::use_horizontal_scroll;
use ui::hooks::use_scroll::use_scroll;

#[component]
fn Probe(
    scope: &mut Scope,
    total: u32,
    longest_line_cells: u32,
    auto_focus: bool,
    initial_top: u32,
    initial_first_cell: u32,
) -> Node {
    let (view, vertical_handle) = use_scroll(scope, *total, *initial_top);
    let maximum_first_cell = longest_line_cells
        .saturating_add(4)
        .saturating_sub(u32::from(view.width));
    let (horizontal, horizontal_handle) =
        use_horizontal_scroll(scope, maximum_first_cell, *initial_first_cell);
    let listeners = use_diff_viewer_navigation(vertical_handle, horizontal_handle);
    let state: Rc<str> = format!("{} {}", view.top, horizontal.first_cell).into();
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

fn harness() -> Harness {
    navigation_harness(20, 40)
}

fn navigation_harness(width: u16, longest_line_cells: u32) -> Harness {
    let mut harness = Harness::new::<Probe>(
        ProbeProps {
            total: 20,
            longest_line_cells,
            auto_focus: true,
            initial_top: 0,
            initial_first_cell: 0,
        },
        width,
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
fn absolute_positions_can_be_set_on_mount() {
    let mut harness = Harness::new::<Probe>(
        ProbeProps {
            total: 20,
            longest_line_cells: 40,
            auto_focus: true,
            initial_top: 6,
            initial_first_cell: 5,
        },
        20,
        4,
    );
    harness.force_draw().force_draw();

    assert_eq!(state(&mut harness), (6, 5));
    harness.press(crokey::key!(j)).force_draw();
    harness.press(crokey::key!(l)).force_draw();
    assert_eq!(state(&mut harness), (7, 6));
}

#[test]
fn j_and_k_scroll_one_view_line() {
    let mut harness = harness();
    harness.press(crokey::key!(j)).force_draw();
    assert_eq!(state(&mut harness).0, 1);
    harness.press(crokey::key!(k)).force_draw();
    assert_eq!(state(&mut harness).0, 0);
}

#[test]
fn wheel_moves_only_the_view() {
    let mut harness = harness();
    harness.wheel(1, 1, 1).force_draw();
    assert_eq!(state(&mut harness), (3, 0));
}

#[test]
fn h_l_zero_and_dollar_move_the_horizontal_position() {
    let mut harness = harness();
    harness.press(crokey::key!(h)).force_draw();
    assert_eq!(state(&mut harness).1, 0);
    for _ in 0..3 {
        harness.press(crokey::key!(l)).force_draw();
    }
    assert_eq!(state(&mut harness).1, 3);
    harness.press(crokey::key!(h)).force_draw();
    assert_eq!(state(&mut harness).1, 2);
    harness.press(crokey::key!(0)).force_draw();
    assert_eq!(state(&mut harness).1, 0);
    harness.press(crokey::key!('$')).force_draw();
    assert_eq!(state(&mut harness).1, 24);
}

#[test]
fn repeated_horizontal_keys_compose_before_a_draw() {
    let mut harness = harness();
    harness
        .press(crokey::key!(l))
        .press(crokey::key!(l))
        .press(crokey::key!(l))
        .force_draw();

    assert_eq!(state(&mut harness).1, 3);
}

#[test]
fn repeated_vertical_keys_compose_before_a_draw() {
    let mut harness = harness();
    harness
        .press(crokey::key!(j))
        .press(crokey::key!(j))
        .press(crokey::key!(j))
        .force_draw();

    assert_eq!(state(&mut harness).0, 3);
}

#[test]
fn horizontal_position_stops_at_the_vscode_endpoint() {
    let mut harness = navigation_harness(10, 20);
    for _ in 0..20 {
        harness.press(crokey::key!(l)).force_draw();
    }
    assert_eq!(state(&mut harness).1, 14);
}

#[test]
fn a_line_narrower_than_the_viewport_does_not_scroll() {
    let mut harness = navigation_harness(20, 10);
    harness.press(crokey::key!(l)).force_draw();
    assert_eq!(state(&mut harness).1, 0);
}

#[test]
fn resizing_clamps_without_forgetting_the_requested_position() {
    let mut harness = navigation_harness(10, 20);
    for _ in 0..6 {
        harness.press(crokey::key!(l)).force_draw();
    }
    assert_eq!(state(&mut harness).1, 6);

    harness.resize(30, 4).force_draw().force_draw();
    assert_eq!(state(&mut harness).1, 0);

    harness.resize(10, 4).force_draw().force_draw();
    assert_eq!(state(&mut harness).1, 6);
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
            Probe {
                total: 20,
                longest_line_cells: 40,
                auto_focus: true,
                initial_top: 0,
                initial_first_cell: 0,
            }
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
