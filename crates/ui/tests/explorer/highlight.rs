use std::path::Path;
use std::rc::Rc;

use loom::testing::Harness;
use loom::{Layout, Node, Scope, component, rsx};
use ui::Theme;
use ui::components::border::{Border, BorderProps};
use ui::components::{Context, Explorer, Ui};

use super::common::{fake_pipeline, file, harness as explorer_harness, wait};

#[component]
fn BorderedExplorer(scope: &mut Scope) -> Node {
    let _ = scope;
    rsx! {
        Border {
            layout: Layout { grow: 1, ..Layout::default() },
            Explorer {}
        }
    }
}

#[test]
fn selection_background_fills_an_unbordered_explorer() {
    let mut harness = explorer_harness(vec![file("src/a.rs"), file("src/b.rs")], 40, 6);
    harness.press(crokey::key!(j));
    harness.force_draw();

    let selected_background = harness.style_at(0, 1).bg;
    let unselected: Vec<u16> = (0..40)
        .filter(|&x| harness.style_at(x, 1).bg != selected_background)
        .collect();

    assert!(unselected.is_empty(), "unselected cells: {unselected:?}");
}

#[test]
fn selection_background_fills_every_cell_inside_the_border() {
    let (pipeline, redraws) =
        fake_pipeline(vec![vec![file("src/a.rs"), file("src/b.rs")]], Vec::new());
    let mut harness = Harness::new::<BorderedExplorer>(BorderedExplorerProps {}, 44, 8)
        .provide::<Ui>(Context {
            theme: Rc::new(Theme::DARK),
            repo: Rc::from(Path::new("/repo")),
            diff_service: Some(Rc::clone(&pipeline)),
            ..Context::default()
        });
    harness.force_draw();
    wait(&pipeline, &redraws, &mut harness);
    harness.force_draw();
    harness.press(crokey::key!(j));
    harness.force_draw();

    let selected_background = harness.style_at(2, 2).bg;
    let unselected: Vec<u16> = (1..43)
        .filter(|&x| harness.style_at(x, 2).bg != selected_background)
        .collect();

    assert!(
        unselected.is_empty(),
        "cells inside the border without the selection background: {unselected:?}"
    );
}
