use std::path::Path;
use std::rc::Rc;

use file_types::{DiffType, File, Oid, RepoPath, Revs, Stats};
use loom::testing::Harness;
use loom::{Node, Scope, component, crokey, rsx};
use pipeline::diff::{self, DiffContent};
use ui::Theme;
use ui::components::{Context, DiffCard, DiffCardProps, DiffCards, DiffCardsProps, Ui};

fn file_at(path: &str) -> File {
    File::unchanged_path(
        RepoPath::new(path, Path::new("/repo")),
        Revs::worktree_against(Oid::new("card")),
    )
    .set_stats(Stats::new(4, 2))
}

fn diff_at(path: &str, original: &[&str], modified: &[&str]) -> DiffContent {
    let changes = diff::compute(original, modified, diff::Settings::default()).unwrap();
    let alignment = diff::align(changes, original, modified).unwrap();
    DiffContent::Diff(diff::Diff {
        file: file_at(path),
        alignment,
    })
}

fn diff(original: &[&str], modified: &[&str]) -> DiffContent {
    diff_at("src/card.rs", original, modified)
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
            height: None,
            auto_focus: true,
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

#[component]
fn CardsHost(_scope: &mut Scope, contents: Rc<Vec<Rc<DiffContent>>>) -> Node {
    rsx! {
        DiffCards {
            contents: Rc::clone(contents),
            view_layout: DiffType::Inline,
            wrap: false,
        }
    }
}

fn cards_harness(contents: Rc<Vec<Rc<DiffContent>>>, width: u16, height: u16) -> Harness {
    Harness::new::<CardsHost>(CardsHostProps { contents }, width, height).provide::<Ui>(Context {
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

#[test]
fn cards_scroll_vertically_as_one_collection() {
    let first_original = [
        "first old one",
        "first old two",
        "first old three",
        "first old four",
        "first old five",
        "first old six",
    ];
    let first_modified = [
        "first new one",
        "first new two",
        "first new three",
        "first new four",
        "first new five",
        "first new six",
    ];
    let second_original = [
        "second old one",
        "second old two",
        "second old three",
        "second old four",
        "second old five",
        "second old six",
    ];
    let second_modified = [
        "second new one",
        "second new two",
        "second new three",
        "second new four",
        "second new five",
        "second new six",
    ];
    let contents = Rc::new(vec![
        Rc::new(diff_at("src/first.rs", &first_original, &first_modified)),
        Rc::new(diff_at("src/second.rs", &second_original, &second_modified)),
    ]);
    let mut harness = cards_harness(contents, 50, 8);
    harness.force_draw().force_draw();
    let start = harness.screen().join("\n");
    assert!(start.contains("first.rs"), "{start}");
    assert!(!start.contains("second.rs"), "{start}");

    harness.wheel(25, 6, 1).force_draw();
    let top = harness.screen_line(0);
    assert!(top.starts_with('│'), "{top:?}");
    assert!(top.ends_with('│'), "{top:?}");
    assert!(!top.contains('╭') && !top.contains('╮'), "{top:?}");

    harness.wheel(25, 6, -1).force_draw();
    for _ in 0..15 {
        harness.press(crokey::key!(j));
    }
    harness.force_draw();
    let scrolled = harness.screen().join("\n");
    assert!(scrolled.contains("second.rs"), "{scrolled}");
    assert_ne!(scrolled, start);
}
