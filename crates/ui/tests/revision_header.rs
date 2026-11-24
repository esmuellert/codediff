use std::path::Path;
use std::rc::Rc;

use diff_service::files::RevisionRange;
use file_types::Oid;
use loom::testing::Harness;
use loom::{Node, Scope, component, rsx, use_state};
use ui::components::{Context, RevisionHeader, RevisionSide, Ui, UiProps};
use ui::ratatui::style::Modifier;
use ui::{Commit, Theme};

fn byte_to_col(line: &str, byte_offset: usize) -> u16 {
    line_index::LineIndex::new(&line[..byte_offset], 1)
        .width()
        .0 as u16
}

fn col_of(line: &str, label: &str) -> u16 {
    let byte_offset = line
        .find(label)
        .unwrap_or_else(|| panic!("{label:?} missing from {line:?}"));
    byte_to_col(line, byte_offset)
}

fn sample_commits() -> Rc<Vec<Commit>> {
    Rc::new(vec![
        Commit {
            oid: Oid::new("6152138abcdef0123456789"),
            parents: vec![Oid::new("fb0d76c1111111111111111")],
            current: true,
            branches: vec!["main".to_owned()],
            remotes: Vec::new(),
            tags: Vec::new(),
            subject: "feat: revision header".to_owned(),
            author: "Alex".to_owned(),
            date: "2h ago".to_owned(),
        },
        Commit {
            oid: Oid::new("fb0d76c1111111111111111"),
            parents: vec![Oid::new("deadbee2222222222222222")],
            current: false,
            branches: Vec::new(),
            remotes: Vec::new(),
            tags: vec!["v0.29.0".to_owned()],
            subject: "release: v0.29.0".to_owned(),
            author: "Alex".to_owned(),
            date: "1d ago".to_owned(),
        },
        Commit {
            oid: Oid::new("deadbee2222222222222222"),
            parents: Vec::new(),
            current: false,
            branches: Vec::new(),
            remotes: Vec::new(),
            tags: Vec::new(),
            subject: "chore: initial".to_owned(),
            author: "Alex".to_owned(),
            date: "2d ago".to_owned(),
        },
    ])
}

#[component]
fn HeaderHost(
    scope: &mut Scope,
    initial_range: RevisionRange,
    initial_picking: Option<RevisionSide>,
    commits: Rc<Vec<Commit>>,
) -> Node {
    let init_range = initial_range.clone();
    let init_picking = *initial_picking;
    let (revision_range, set_revision_range) = use_state(scope, move || init_range);
    let (picking, set_picking) = use_state(scope, move || init_picking);
    let context = Context {
        theme: Rc::new(Theme::DARK),
        repo: Rc::from(Path::new("/repos/codediff")),
        revision_range: revision_range.clone(),
        set_revision_range: Some(set_revision_range),
        picking,
        set_picking: Some(set_picking),
        commits: Rc::clone(commits),
        ..Context::default()
    };
    rsx! {
        Ui {
            value: context,
            RevisionHeader {}
        }
    }
}

fn mount(
    initial_range: RevisionRange,
    initial_picking: Option<RevisionSide>,
    width: u16,
) -> Harness {
    let mut harness = Harness::new::<HeaderHost>(
        HeaderHostProps {
            initial_range,
            initial_picking,
            commits: sample_commits(),
        },
        width,
        1,
    );
    harness.force_draw();
    harness.force_draw();
    harness
}

#[test]
fn worktree_renders_from_to_semantic_icons_repo_branch_and_hides_swap() {
    let mut harness = mount(RevisionRange::Worktree, None, 110);
    let line = harness.screen_line(0);
    assert!(line.contains(" From "), "{line:?}");
    assert!(line.contains(" \u{f418} main ▾ "), "{line:?}");
    assert!(
        !line.contains("6152138"),
        "named branch should not append oid: {line:?}"
    );
    assert!(line.contains("  to  "), "{line:?}");
    assert!(line.contains(" \u{e5fe} Working tree ▾ "), "{line:?}");
    assert!(!line.contains("⇄"), "worktree must hide swap: {line:?}");
    assert!(line.contains("codediff  \u{f418} main"), "{line:?}");
    assert!(
        col_of(&line, "codediff") > col_of(&line, "Working tree") + 10,
        "repo/branch should be right-aligned: {line:?}"
    );

    let pill_bg = Theme::DARK.normal.patch(Theme::DARK.cursor_line).bg;
    let icon_style = harness.style_at(col_of(&line, "\u{f418}"), 0);
    let label_style = harness.style_at(col_of(&line, "main"), 0);
    assert_eq!(harness.style_at(0, 0).bg, Theme::DARK.normal.bg);
    assert_eq!(icon_style.bg, pill_bg);
    assert_eq!(icon_style.fg, Some(Theme::DARK.tree.directory));
    assert_eq!(label_style.bg, pill_bg);
    assert_eq!(label_style.fg, Some(Theme::DARK.tree.name));
}

#[test]
fn between_and_merge_base_render_tag_and_commit_icons_and_swap() {
    let mut harness = mount(
        RevisionRange::Between("fb0d76c".to_owned(), "deadbee".to_owned()),
        None,
        110,
    );
    let before = harness.screen_line(0);
    assert!(before.contains(" \u{f412} v0.29.0 ▾ "), "{before:?}");
    assert!(before.contains("  to  "), "{before:?}");
    assert!(before.contains(" \u{f417} deadbee ▾ "), "{before:?}");
    assert!(before.contains(" ⇄ "), "{before:?}");

    harness.click(col_of(&before, "⇄"), 0);
    let after = harness.screen_line(0);
    assert!(
        col_of(&after, "\u{f417} deadbee") < col_of(&after, "\u{f412} v0.29.0"),
        "clicking swap should swap endpoints: {after:?}"
    );

    let mut merge_base = mount(
        RevisionRange::MergeBase("fb0d76c".to_owned(), "6152138".to_owned()),
        None,
        110,
    );
    let mb_line = merge_base.screen_line(0);
    assert!(mb_line.contains(" ... "), "{mb_line:?}");
    assert!(mb_line.contains(" \u{f412} v0.29.0 ▾ "), "{mb_line:?}");
    assert!(mb_line.contains(" \u{f418} main ▾ "), "{mb_line:?}");
    assert!(mb_line.contains(" ⇄ "), "{mb_line:?}");
}

#[test]
fn clicking_pills_toggles_active_highlight() {
    let mut harness = mount(RevisionRange::Worktree, None, 100);
    let line = harness.screen_line(0);
    let left_col = col_of(&line, "main");
    let right_col = col_of(&line, "Working tree");
    let accent = Theme::DARK
        .border_focused
        .fg
        .unwrap_or(Theme::DARK.tree.name);

    harness.click(left_col, 0);
    assert!(
        harness
            .style_at(left_col, 0)
            .add_modifier
            .contains(Modifier::UNDERLINED)
    );
    assert_eq!(harness.style_at(left_col, 0).fg, Some(accent));
    assert!(
        !harness
            .style_at(right_col, 0)
            .add_modifier
            .contains(Modifier::UNDERLINED)
    );

    harness.click(right_col, 0);
    assert!(
        !harness
            .style_at(left_col, 0)
            .add_modifier
            .contains(Modifier::UNDERLINED)
    );
    assert!(
        harness
            .style_at(right_col, 0)
            .add_modifier
            .contains(Modifier::UNDERLINED)
    );
    assert_eq!(harness.style_at(right_col, 0).fg, Some(accent));
}

#[test]
fn narrow_width_hides_repo_and_branch_cleanly_without_clipping_endpoints() {
    let mut harness = mount(RevisionRange::Worktree, None, 48);
    let line = harness.screen_line(0);
    assert!(line.contains(" \u{f418} main ▾ "), "{line:?}");
    assert!(line.contains(" \u{e5fe} Working tree ▾"), "{line:?}");
    assert!(!line.contains("codediff"), "{line:?}");
}
