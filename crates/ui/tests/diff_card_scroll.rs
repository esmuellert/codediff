use std::path::Path;
use std::rc::Rc;

use file_types::{DiffType, File, Oid, RepoPath, Revs};
use loom::testing::Harness;
use loom::{Node, Scope, component, rsx};
use pipeline::diff::{self, DiffContent};
use ui::Theme;
use ui::components::{Context, DiffCards, DiffCardsProps, Ui};

#[component]
fn Cards(
    _scope: &mut Scope,
    contents: Rc<Vec<Rc<DiffContent>>>,
    view_layout: DiffType,
    wrap: bool,
) -> Node {
    rsx! {
        DiffCards {
            contents: Rc::clone(contents),
            view_layout: *view_layout,
            wrap: *wrap,
        }
    }
}

fn contents() -> Rc<Vec<Rc<DiffContent>>> {
    Rc::new(
        (0..3)
            .map(|file| {
                let original: Vec<_> = (1..=36)
                    .map(|line| {
                        format!("OLD_{file}_{line:03} a_long_identifier_with_a_wrapped_tail")
                    })
                    .collect();
                let modified: Vec<_> = (1..=36)
                    .map(|line| {
                        format!("NEW_{file}_{line:03} another_identifier_with_a_wrapped_tail")
                    })
                    .collect();
                let original: Vec<_> = original.iter().map(String::as_str).collect();
                let modified: Vec<_> = modified.iter().map(String::as_str).collect();
                let changes =
                    diff::compute(&original, &modified, diff::Settings::default()).unwrap();
                let alignment = diff::align(changes, &original, &modified).unwrap();
                Rc::new(DiffContent::Diff(diff::Diff {
                    file: File::unchanged_path(
                        RepoPath::new(format!("src/card_{file}.rs"), Path::new("/repo")),
                        Revs::worktree_against(Oid::new("base")),
                    ),
                    alignment,
                }))
            })
            .collect(),
    )
}

fn harness(
    contents: &Rc<Vec<Rc<DiffContent>>>,
    layout: DiffType,
    wrap: bool,
    height: u16,
) -> Harness {
    Harness::new::<Cards>(
        CardsProps {
            contents: Rc::clone(contents),
            view_layout: layout,
            wrap,
        },
        72,
        height,
    )
    .provide::<Ui>(Context {
        theme: Rc::new(Theme::DARK),
        ..Context::default()
    })
}

fn settle(h: &mut Harness) {
    h.force_draw();
    for _ in 0..8 {
        if !h.needs_draw() {
            return;
        }
        h.force_draw();
    }
    assert!(!h.needs_draw(), "cards did not settle");
}

#[test]
fn every_three_line_scroll_matches_the_complete_card_content() {
    let contents = contents();
    for layout in [DiffType::Inline, DiffType::SideBySide] {
        for wrap in [false, true] {
            let mut full = harness(&contents, layout, wrap, 2000);
            settle(&mut full);
            let reference = full.screen();
            let content_height = reference
                .iter()
                .rposition(|row| !row.trim().is_empty())
                .unwrap()
                + 1;
            let expected_cells = full.cells().clone();
            let mut h = harness(&contents, layout, wrap, 12);
            settle(&mut h);
            let max_top = content_height - 12;
            let mut top = 0;
            for direction in [1, -1] {
                loop {
                    if top == 15 {
                        h.resize(72, 18);
                        settle(&mut h);
                        assert_eq!(
                            h.screen(),
                            reference[top..top + 18],
                            "resized while scrolling: layout={layout:?} wrap={wrap}"
                        );
                        h.resize(72, 12);
                        settle(&mut h);
                    }
                    let actual = h.screen();
                    assert_eq!(
                        actual,
                        reference[top..top + 12],
                        "layout={layout:?} wrap={wrap} top={top} direction={direction}"
                    );
                    for y in 0..12 {
                        for x in 0..72 {
                            assert_eq!(
                                h.cells().cell((x, y)),
                                expected_cells.cell((x, y + top as u16)),
                                "style at {x},{y}: layout={layout:?} wrap={wrap} top={top}"
                            );
                        }
                    }
                    if (direction > 0 && top == max_top) || (direction < 0 && top == 0) {
                        break;
                    }
                    top = if direction > 0 {
                        (top + 3).min(max_top)
                    } else {
                        top.saturating_sub(3)
                    };
                    h.wheel(50, 6, direction);
                    settle(&mut h);
                }
            }
        }
    }
}
