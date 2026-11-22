//! Tests for the CommitGraph component in both `BranchDrawing` and `BoxDrawing` modes.

#![allow(clippy::type_complexity)]

use std::cell::RefCell;
use std::rc::Rc;

use file_types::Oid;
use loom::crokey::key;
use loom::testing::Harness;
use ratatui::style::{Color, Modifier};
use ui::components::{CommitGraph, CommitGraphProps, Context, Ui};
use ui::{Commit, Theme};

fn commit(oid: &str, parents: &[&str], subject: &str) -> Commit {
    Commit {
        oid: Oid::new(oid),
        parents: parents.iter().copied().map(Oid::new).collect(),
        current: false,
        branches: Vec::new(),
        remotes: Vec::new(),
        tags: Vec::new(),
        subject: subject.to_owned(),
        author: "Yanuo Ma".to_owned(),
        date: "2026-09-10".to_owned(),
    }
}

fn decorated_commits() -> Vec<Commit> {
    let mut head = commit("fb0d76c", &["91dba92"], "release: v0.29.0 [skip ci]");
    head.current = true;
    head.branches = vec!["main".to_owned(), "develop".to_owned()];
    head.remotes = vec!["origin/main".to_owned()];
    head.tags = vec!["v0.29.0".to_owned()];
    head.author = "github-actions[bot]".to_owned();

    let merge = commit(
        "91dba92",
        &["3493f95", "f8b977f"],
        "refactor(ui): restore native scrollbind (#557)",
    );

    let mut topic = commit(
        "f8b977f",
        &["3493f95"],
        "refactor(ui): restore native scrollbind",
    );
    topic.remotes = vec!["origin/feat/scrollbind".to_owned()];

    let mut detached = commit("3493f95", &[], "initial baseline commit");
    detached.current = true;

    vec![head, merge, topic, detached]
}

fn harness(
    commits: Vec<Commit>,
    theme: Theme,
    branch_drawing: bool,
    width: u16,
    height: u16,
    on_select: Option<Rc<dyn Fn(&Commit)>>,
    on_open: Option<Rc<dyn Fn(&Commit)>>,
) -> Harness {
    let mut h = Harness::new::<CommitGraph>(
        CommitGraphProps {
            commits: Rc::new(commits),
            on_select,
            on_open,
        },
        width,
        height,
    )
    .provide::<Ui>(Context {
        theme: Rc::new(theme),
        branch_drawing,
        ..Context::default()
    });
    for _ in 0..4 {
        h.force_draw();
    }
    h
}

#[test]
fn empty_list_renders_placeholder_without_markers() {
    for branch_drawing in [false, true] {
        let mut h = harness(Vec::new(), Theme::DARK, branch_drawing, 80, 6, None, None);
        let screen = h.screen().join("\n");
        assert!(screen.contains("No commits"), "{screen}");
        assert!(!screen.contains('○'), "{screen}");
        assert!(!screen.contains('◎'), "{screen}");
        assert!(
            !screen
                .chars()
                .any(|ch| ('\u{f5d0}'..='\u{f60d}').contains(&ch))
        );
    }
}

#[test]
fn branch_drawing_mode_renders_filled_pua_circles_and_seamless_curves() {
    let mut h = harness(decorated_commits(), Theme::DARK, true, 120, 6, None, None);

    // Row 0: first commit (line down only -> filled circle \u{f5f6})
    let row0 = h.screen_line(0);
    assert!(row0.starts_with("› \u{f5f6}"), "{row0:?}");
    assert!(
        row0.contains("HEAD -> main · develop · origin/main · tag: v0.29.0"),
        "{row0:?}"
    );
    assert!(row0.contains("GitHub Actions"), "{row0:?}");
    assert!(row0.contains("09-10"), "{row0:?}");
    assert!(row0.contains("fb0d76c"), "{row0:?}");

    // Row 1: merge commit (up + down + right -> filled circle \u{f604}, horizontal \u{f5d0}, bend down-left \u{f5d7})
    let row1 = h.screen_line(1);
    assert!(row1.starts_with("  \u{f604}\u{f5d0}\u{f5d7}"), "{row1:?}");

    // Row 2: vertical \u{f5d1} on lane 0, filled commit \u{f5fa} (up + down) on lane 1
    let row2 = h.screen_line(2);
    assert!(row2.starts_with("  \u{f5d1} \u{f5fa}"), "{row2:?}");

    // Row 3: root commit with incoming merge from right (up + right -> filled circle \u{f600}, \u{f5d0}, bend up-left \u{f5d9})
    let row3 = h.screen_line(3);
    assert!(row3.starts_with("  \u{f600}\u{f5d0}\u{f5d9}"), "{row3:?}");
    assert!(row3.contains("HEAD  initial baseline commit"), "{row3:?}");
}

#[test]
fn box_drawing_fallback_renders_lazygit_commit_and_merge_glyphs() {
    let mut h = harness(decorated_commits(), Theme::DARK, false, 120, 6, None, None);

    let row0 = h.screen_line(0);
    assert!(row0.starts_with("› ○"), "{row0:?}");
    assert_eq!(
        h.screen_line(1).chars().take(5).collect::<String>(),
        "  ◎─╮"
    );
    assert_eq!(
        h.screen_line(2).chars().take(5).collect::<String>(),
        "  │ ○"
    );
    assert_eq!(
        h.screen_line(3).chars().take(5).collect::<String>(),
        "  ○─╯"
    );
}

#[test]
fn selecting_a_merge_commit_highlights_its_outgoing_pipes_in_both_modes() {
    for branch_drawing in [false, true] {
        let mut h = harness(
            decorated_commits(),
            Theme::DARK,
            branch_drawing,
            100,
            6,
            None,
            None,
        );

        assert!(!h.style_at(2, 2).add_modifier.contains(Modifier::BOLD));

        h.press(key!(j)).force_draw();
        let highlight_fg = Theme::DARK.line_number_current.fg;
        for x in [2u16, 3, 4] {
            assert_eq!(h.style_at(x, 1).fg, highlight_fg, "row 1 col {x}");
            assert!(h.style_at(x, 1).add_modifier.contains(Modifier::BOLD));
        }
        assert_eq!(h.style_at(2, 2).fg, highlight_fg);
        assert!(h.style_at(2, 2).add_modifier.contains(Modifier::BOLD));
        assert_eq!(h.style_at(2, 3).fg, highlight_fg);
        assert!(h.style_at(2, 3).add_modifier.contains(Modifier::BOLD));
    }
}

#[test]
fn ref_colours_and_lane_colours_never_emit_rgb_in_basic_themes() {
    for theme in Theme::ALL {
        for branch_drawing in [false, true] {
            let mut h = harness(
                decorated_commits(),
                theme,
                branch_drawing,
                120,
                6,
                None,
                None,
            );

            let line0 = h.screen_line(0);
            let head_col = line0[..line0.find("HEAD -> main").unwrap()].chars().count() as u16;
            let branch_col = line0[..line0.find("develop").unwrap()].chars().count() as u16;
            let tag_col = line0[..line0.find("tag: v0.29.0").unwrap()].chars().count() as u16;

            assert_eq!(
                h.style_at(head_col, 0).fg,
                Some(theme.tree.heading),
                "{}",
                theme.name
            );
            assert_eq!(
                h.style_at(branch_col, 0).fg,
                Some(theme.tree.directory),
                "{}",
                theme.name
            );
            assert_eq!(
                h.style_at(tag_col, 0).fg,
                Some(theme.code.constant),
                "{}",
                theme.name
            );

            if theme.name.starts_with("basic-") {
                let cells = h.cells();
                for y in 0..6 {
                    for x in 0..120 {
                        let style = cells.cell((x, y)).unwrap().style();
                        assert!(
                            !matches!(style.fg, Some(Color::Rgb(..))),
                            "{} leaked RGB fg at ({x}, {y}): {style:?}",
                            theme.name
                        );
                        assert!(
                            !matches!(style.bg, Some(Color::Rgb(..))),
                            "{} leaked RGB bg at ({x}, {y}): {style:?}",
                            theme.name
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn keyboard_and_mouse_navigation_update_selection_and_fire_callbacks() {
    let selected: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));
    let opened: Rc<RefCell<Vec<String>>> = Rc::new(RefCell::new(Vec::new()));

    let selected_cb = Rc::clone(&selected);
    let opened_cb = Rc::clone(&opened);
    let mut h = harness(
        decorated_commits(),
        Theme::DARK,
        true,
        100,
        6,
        Some(Rc::new(move |commit| {
            selected_cb
                .borrow_mut()
                .push(commit.oid.as_str().to_owned());
        })),
        Some(Rc::new(move |commit| {
            opened_cb.borrow_mut().push(commit.oid.as_str().to_owned());
        })),
    );

    let cursor_bg = Theme::DARK.normal.patch(Theme::DARK.cursor_line).bg;
    assert_eq!(h.style_at(0, 0).bg, cursor_bg);

    h.press(key!(j)).force_draw();
    assert_eq!(h.style_at(0, 1).bg, cursor_bg);
    assert_eq!(*selected.borrow(), vec!["91dba92"]);

    h.press(key!('$')).force_draw();
    assert_eq!(h.style_at(0, 3).bg, cursor_bg);
    assert_eq!(*selected.borrow(), vec!["91dba92", "3493f95"]);

    h.press(key!('0')).force_draw();
    assert_eq!(h.style_at(0, 0).bg, cursor_bg);
    assert_eq!(*selected.borrow(), vec!["91dba92", "3493f95", "fb0d76c"]);

    h.click(5, 2).force_draw();
    assert_eq!(h.style_at(0, 2).bg, cursor_bg);
    assert_eq!(
        *selected.borrow(),
        vec!["91dba92", "3493f95", "fb0d76c", "f8b977f"]
    );

    h.press(key!(k)).force_draw();
    assert_eq!(h.style_at(0, 1).bg, cursor_bg);

    h.press(key!(enter)).force_draw();
    assert_eq!(*opened.borrow(), vec!["91dba92"]);
}

#[test]
fn virtualized_scroll_keeps_the_cursor_visible_across_many_commits() {
    let commits: Vec<Commit> = (0..60)
        .map(|index| {
            let oid = format!("{index:07x}");
            let parent = format!("{:07x}", index + 1);
            commit(&oid, &[&parent], &format!("subject-{index:02}"))
        })
        .collect();

    let mut h = harness(commits, Theme::DARK, true, 90, 8, None, None);
    assert!(h.screen().join("\n").contains("subject-00"));
    assert!(!h.screen().join("\n").contains("subject-40"));

    h.press(key!('$')).force_draw().force_draw();
    let bottom = h.screen().join("\n");
    assert!(bottom.contains("subject-59"), "{bottom}");
    assert!(!bottom.contains("subject-00"), "{bottom}");

    h.press(key!('0')).force_draw().force_draw();
    let top = h.screen().join("\n");
    assert!(top.contains("subject-00"), "{top}");
}

#[test]
fn reloading_commits_preserves_selected_commit_by_oid_and_settles() {
    let initial = vec![
        commit("1111111", &["2222222"], "first"),
        commit("2222222", &[], "target"),
    ];
    let mut h = harness(initial, Theme::DARK, true, 80, 5, None, None);
    h.press(key!(j)).force_draw();
    let cursor_bg = Theme::DARK.normal.patch(Theme::DARK.cursor_line).bg;
    assert_eq!(h.style_at(0, 1).bg, cursor_bg);

    let reloaded = vec![
        commit("0000000", &["1111111"], "newest"),
        commit("1111111", &["2222222"], "first"),
        commit("2222222", &[], "target"),
    ];
    h.set_props::<CommitGraph>(CommitGraphProps {
        commits: Rc::new(reloaded),
        on_select: None,
        on_open: None,
    });
    for _ in 0..5 {
        h.force_draw();
    }

    assert!(!h.needs_draw());
    assert_eq!(h.style_at(0, 2).bg, cursor_bg);
    assert!(h.screen_line(2).contains("target"));
}

#[test]
fn narrow_and_cjk_widths_never_panic_or_emit_selector_ellipsis() {
    let mut cjk = commit(
        "fb0d76c",
        &["91dba92"],
        "修复宽字符提交标题截断与分支绘制对齐 🦀",
    );
    cjk.current = true;
    cjk.branches = vec!["主分支".to_owned()];
    let commits = vec![cjk, commit("91dba92", &[], "second")];

    let mut w1 = harness(commits.clone(), Theme::DARK, true, 1, 4, None, None);
    assert_eq!(w1.screen_line(0), "›");

    for width in [2u16, 4, 8, 12, 20, 32, 60] {
        let mut h = harness(commits.clone(), Theme::DARK, true, width, 4, None, None);
        assert_eq!(h.screen().len(), 4);
    }
}
