use std::path::Path;
use std::rc::Rc;

use file_types::{DiffType, File, Oid, RepoPath, Revs, Stats};
use loom::testing::Harness;
use loom::{Node, Scope, component, crokey, rsx};
use pipeline::diff::{self, DiffContent};
use ui::Theme;
use ui::components::{Context, DiffCard, DiffCardProps, Ui};

fn file() -> File {
    File::unchanged_path(
        RepoPath::new("src/card.rs", Path::new("/repo")),
        Revs::worktree_against(Oid::new("card")),
    )
    .set_stats(Stats::new(4, 2))
}

fn diff(original: &[&str], modified: &[&str]) -> DiffContent {
    let changes = diff::compute(original, modified, diff::Settings::default()).unwrap();
    let alignment = diff::align(changes, original, modified).unwrap();
    DiffContent::Diff(diff::Diff {
        file: file(),
        alignment,
    })
}

#[component]
fn CardHost(
    _scope: &mut Scope,
    content: Rc<DiffContent>,
    view_layout: DiffType,
    wrap: bool,
) -> Node {
    rsx! {
        DiffCard {
            content: Rc::clone(content),
            view_layout: *view_layout,
            wrap: *wrap,
        }
    }
}

fn harness(content: DiffContent, width: u16, height: u16) -> Harness {
    Harness::new::<CardHost>(
        CardHostProps {
            content: Rc::new(content),
            view_layout: DiffType::Inline,
            wrap: false,
        },
        width,
        height,
    )
    .provide::<Ui>(Context {
        theme: Rc::new(Theme::DARK),
        ..Context::default()
    })
}

#[test]
fn card_draws_its_header_and_compact_diff() {
    let mut harness = harness(diff(&["before"], &["after"]), 60, 8);
    harness.force_draw().force_draw();

    let screen = harness.screen().join("\n");
    assert!(screen.contains("src/card.rs"), "{screen}");
    assert!(screen.contains("before"), "{screen}");
    assert!(screen.contains("after"), "{screen}");
    assert!(screen.contains('╭'), "{screen}");

    let header = harness.screen_line(1);
    let plus = header.find('+').expect("added count");
    let minus = header.find('-').expect("removed count");
    assert_eq!(
        harness.style_at(plus as u16, 1).fg,
        Some(Theme::DARK.change.gained)
    );
    assert_eq!(
        harness.style_at(minus as u16, 1).fg,
        Some(Theme::DARK.change.lost)
    );
}

#[test]
fn card_scrolls_horizontally_without_vertical_scrolling() {
    let original = "CARD_ORIGINAL_".to_owned() + &"0123456789".repeat(12);
    let modified = "CARD_MODIFIED_".to_owned() + &"fedcba9876".repeat(12);
    let original = [original.as_str()];
    let modified = [modified.as_str()];
    let mut harness = harness(diff(&original, &modified), 40, 8);
    harness.force_draw().force_draw();
    let start = harness.screen().join("\n");

    for _ in 0..20 {
        harness.press(crokey::key!(l));
    }
    harness.force_draw();
    let horizontal = harness.screen().join("\n");
    assert_ne!(horizontal, start);
    assert!(horizontal.contains("fedcba"), "{horizontal}");

    harness.press(crokey::key!(j)).force_draw();
    assert_eq!(harness.screen().join("\n"), horizontal);
}
