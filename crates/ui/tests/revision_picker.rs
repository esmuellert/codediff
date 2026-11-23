//! Tests for the `RevisionPicker` component.

#![allow(clippy::type_complexity)]

use std::cell::RefCell;
use std::rc::Rc;

use file_types::{Oid, Rev};
use loom::crokey::key;
use loom::testing::Harness;
use ratatui::style::{Color, Modifier};
use ui::components::{Context, RevisionPicker, RevisionPickerProps, Ui};
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
    head.branches = vec!["main".to_owned()];
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
        "feat: quick jump keys j k q in viewer",
    );
    topic.remotes = vec!["origin/feat/scrollbind".to_owned()];

    let mut tagged = commit("3493f95", &["89480e6"], "release: v0.28.4 [skip ci]");
    tagged.tags = vec!["v0.28.4".to_owned()];
    tagged.author = "github-actions[bot]".to_owned();

    let mut older_tag = commit("89480e6", &[], "release: v0.28.3 [skip ci]");
    older_tag.tags = vec!["v0.28.3".to_owned()];
    older_tag.author = "github-actions[bot]".to_owned();

    vec![head, merge, topic, tagged, older_tag]
}

fn harness_with_theme(
    props: RevisionPickerProps,
    theme: Theme,
    branch_drawing: bool,
    width: u16,
    height: u16,
) -> Harness {
    let mut h = Harness::new::<RevisionPicker>(props, width, height).provide::<Ui>(Context {
        theme: Rc::new(theme),
        branch_drawing,
        ..Context::default()
    });
    for _ in 0..4 {
        h.force_draw();
    }
    h
}

fn harness_with_props(
    props: RevisionPickerProps,
    branch_drawing: bool,
    width: u16,
    height: u16,
) -> Harness {
    harness_with_theme(props, Theme::DARK, branch_drawing, width, height)
}

fn harness(
    commits: Vec<Commit>,
    worktree_summary: Option<&str>,
    branch_drawing: bool,
    width: u16,
    height: u16,
    on_select: Option<Rc<dyn Fn(Rev)>>,
    on_open: Option<Rc<dyn Fn(Rev)>>,
) -> Harness {
    harness_with_props(
        RevisionPickerProps {
            commits: Rc::new(commits),
            worktree_summary: worktree_summary.map(Rc::from),
            initial: None,
            on_select,
            on_open,
            on_close: None,
        },
        branch_drawing,
        width,
        height,
    )
}

fn press_text(h: &mut Harness, text: &str) {
    for ch in text.chars() {
        let combo =
            loom::crokey::KeyCombination::from(loom::crokey::crossterm::event::KeyCode::Char(ch));
        h.press(combo).force_draw();
    }
}

fn selected_row_containing(h: &mut Harness, needle: &str) -> Option<u16> {
    let cursor_bg = Theme::DARK.normal.patch(Theme::DARK.cursor_line).bg;
    let lines = h.screen();
    (0..lines.len() as u16)
        .find(|&y| lines[y as usize].contains(needle) && h.style_at(0, y).bg == cursor_bg)
}

#[test]
fn browse_mode_renders_header_working_tree_separator_and_skips_separator_on_j_k() {
    let selected: Rc<RefCell<Vec<Rev>>> = Rc::new(RefCell::new(Vec::new()));
    let selected_cb = Rc::clone(&selected);

    let mut h = harness(
        decorated_commits(),
        Some("changed · 6 files"),
        true,
        110,
        10,
        Some(Rc::new(move |rev| {
            selected_cb.borrow_mut().push(rev);
        })),
        None,
    );

    assert!(!h.screen().join("\n").contains('›'));

    let header = h.screen_line(0);
    assert!(header.starts_with("git log"), "{header:?}");
    assert!(header.ends_with("[/] search"), "{header:?}");

    let row_wt = h.screen_line(1);
    assert!(
        row_wt.contains("Working tree  changed · 6 files"),
        "{row_wt:?}"
    );

    let sep = h.screen_line(2);
    assert_eq!(sep.chars().count(), 110);
    assert!(sep.chars().all(|ch| ch == '─'), "{sep:?}");

    let commit0 = h.screen_line(3);
    assert!(commit0.starts_with(" \u{f5f6}"), "{commit0:?}");
    assert!(commit0.contains("HEAD -> main"), "{commit0:?}");

    // Working tree label aligns with commit text column (1 margin + 4 graph + 1 gap = col 6).
    assert_eq!(
        row_wt[..row_wt.find("Working tree").unwrap()]
            .chars()
            .count(),
        commit0[..commit0.find("HEAD -> main").unwrap()]
            .chars()
            .count(),
    );

    let commit1 = h.screen_line(4);
    assert!(
        commit1.starts_with(" \u{f604}\u{f5d0}\u{f5d7}"),
        "{commit1:?}"
    );

    // `j` steps straight over the separator from Working tree (row 1) to commit 0 (row 3).
    let cursor_bg = Theme::DARK.normal.patch(Theme::DARK.cursor_line).bg;
    assert_eq!(h.style_at(0, 1).bg, cursor_bg);

    h.press(key!(j)).force_draw();
    assert_eq!(h.style_at(0, 3).bg, cursor_bg);
    assert_ne!(h.style_at(0, 1).bg, cursor_bg);
    assert_ne!(h.style_at(0, 2).bg, cursor_bg);
    assert_eq!(*selected.borrow(), vec![Rev::Commit(Oid::new("fb0d76c"))]);

    // `k` steps straight back across the separator to Working tree (row 1).
    h.press(key!(k)).force_draw();
    assert_eq!(h.style_at(0, 1).bg, cursor_bg);
    assert_eq!(
        *selected.borrow(),
        vec![Rev::Commit(Oid::new("fb0d76c")), Rev::Worktree]
    );
}

#[test]
fn search_filters_into_compact_flat_list_without_pipes_and_treats_j_k_q_as_query_text() {
    let mut h = harness(
        decorated_commits(),
        Some("changed · 6 files"),
        false,
        110,
        10,
        None,
        None,
    );

    // Click `[/] search` on the right side of header row 0 to open search.
    h.click(105, 0).force_draw();
    assert!(h.screen_line(0).ends_with("[Esc] close"));
    assert!(h.screen_line(1).contains("> Search revisions…"));
    // Empty query keeps Browse Mode continuous graph visible below search bar.
    assert!(h.screen_line(2).contains("Working tree"));
    assert!(h.screen_line(3).chars().all(|ch| ch == '─'));
    assert!(h.screen_line(5).contains("◎─╮"));

    // Type "v0.28" — distant commits #3 and #4 pack into rows 2 and 3 at 1-col left margin!
    press_text(&mut h, "v0.28");
    assert_eq!(h.screen_line(1), " > v0.28");
    let first_match = h.screen_line(2);
    let second_match = h.screen_line(3);
    assert!(first_match.starts_with(" tag: v0.28.4"), "{first_match:?}");
    assert!(
        second_match.starts_with(" tag: v0.28.3"),
        "{second_match:?}"
    );
    let cursor_bg = Theme::DARK.normal.patch(Theme::DARK.cursor_line).bg;
    assert_eq!(h.style_at(0, 2).bg, cursor_bg);
    assert_ne!(h.style_at(0, 3).bg, cursor_bg);
    assert!(!h.screen().join("\n").contains('›'));
    assert!(!h.screen().join("\n").contains("Working tree"));
    assert!(!h.screen().join("\n").contains("◎─╮"));

    // Clear via Backspace and type a query containing 'j', 'k', 'q' — must NOT quit or navigate!
    for _ in 0..5 {
        h.press(key!(backspace)).force_draw();
    }
    press_text(&mut h, "j k q");
    assert!(!h.exiting());
    assert_eq!(h.screen_line(1), " > j k q");
    assert!(h.screen_line(2).contains("quick jump keys j k q in viewer"));
    assert!(h.screen_line(3).is_empty());
}

#[test]
fn exiting_search_via_esc_or_backspace_restores_exact_browse_selection_and_scroll_offset() {
    let commits: Vec<Commit> = (0..40)
        .map(|index| {
            let oid = format!("{index:07x}");
            let parent = format!("{:07x}", index + 1);
            let mut c = commit(&oid, &[&parent], &format!("subject-{index:02}"));
            if index == 2 || index == 30 {
                c.tags = vec![format!("match-tag-{index:02}")];
            }
            c
        })
        .collect();

    let mut h = harness(
        commits,
        Some("changed · 6 files"),
        true,
        100,
        10,
        None,
        None,
    );

    // Move down to commit #15 (candidate 16: Working tree is 0, commit #0..#15 are 1..=16).
    for _ in 0..16 {
        h.press(key!(j)).force_draw();
    }
    h.force_draw();

    let pre_search_row = selected_row_containing(&mut h, "subject-15")
        .expect("expected commit #15 selected before search");
    let pre_search_screen = h.screen();
    assert!(
        !pre_search_screen
            .iter()
            .any(|line| line.contains("Working tree")),
        "expected viewport scrolled past Working tree:\n{}",
        pre_search_screen.join("\n")
    );

    // Open `/` search, type "match-tag", move down in filtered list, then press Esc.
    h.press(key!('/')).force_draw();
    press_text(&mut h, "match-tag");
    h.force_draw();
    assert!(h.screen_line(2).contains("match-tag-02"));
    assert!(h.screen_line(3).contains("match-tag-30"));

    let cursor_bg = Theme::DARK.normal.patch(Theme::DARK.cursor_line).bg;
    h.press(key!(down)).force_draw();
    assert_eq!(h.style_at(0, 3).bg, cursor_bg);

    h.press(key!(esc)).force_draw().force_draw();
    assert_eq!(
        h.screen(),
        pre_search_screen,
        "Esc must restore exact viewport scroll offset"
    );
    assert_eq!(h.style_at(0, pre_search_row).bg, cursor_bg);

    // Now repeat and clear query via Backspace back to empty, then close search:
    h.press(key!('/')).force_draw();
    press_text(&mut h, "30");
    h.force_draw();
    assert!(h.screen_line(2).contains("subject-30"));

    // Backspace back to empty while search bar is still open -> Browse position already restored!
    h.press(key!(backspace)).force_draw();
    h.press(key!(backspace)).force_draw().force_draw();
    assert!(
        selected_row_containing(&mut h, "subject-15").is_some(),
        "Backspace-to-empty must restore commit #15 in Browse mode:\n{}",
        h.screen().join("\n")
    );

    // Clicking `[Esc] close` closes the bar and matches pre_search_screen + cursor_line!
    h.click(95, 0).force_draw().force_draw();
    assert_eq!(h.screen(), pre_search_screen);
    assert_eq!(h.style_at(0, pre_search_row).bg, cursor_bg);
}

#[test]
fn cjk_query_backspace_enter_on_open_and_empty_no_matches_state() {
    let mut commits = decorated_commits();
    let mut cjk = commit("c7c7c7c", &[], "修复修订选择器搜索过滤 🦀");
    cjk.branches = vec!["功能/搜索".to_owned()];
    commits.push(cjk);

    let opened: Rc<RefCell<Vec<Rev>>> = Rc::new(RefCell::new(Vec::new()));
    let opened_cb = Rc::clone(&opened);
    let mut h = harness(
        commits,
        Some("changed · 2 files"),
        true,
        100,
        10,
        None,
        Some(Rc::new(move |rev| {
            opened_cb.borrow_mut().push(rev);
        })),
    );

    // Enter on initial row opens Rev::Worktree.
    h.press(key!(enter)).force_draw();
    assert_eq!(*opened.borrow(), vec![Rev::Worktree]);

    // Search with CJK + emoji, Backspace emoji + one CJK char safely on char boundaries, then Enter.
    h.press(key!('/')).force_draw();
    press_text(&mut h, "搜索过滤🦀");
    assert_eq!(h.screen_line(1), " > 搜索过滤🦀");
    assert_eq!(h.screen_line(2), " No revisions match.");

    // Backspace deletes '🦀' (4-byte UTF-8), then '滤' (3-byte UTF-8) -> "搜索过" matches!
    h.press(key!(backspace)).force_draw();
    assert_eq!(h.screen_line(1), " > 搜索过滤");
    assert!(h.screen_line(2).contains("修复修订选择器搜索过滤"));

    h.press(key!(backspace)).force_draw();
    assert_eq!(h.screen_line(1), " > 搜索过");
    h.press(key!(enter)).force_draw();
    assert_eq!(
        *opened.borrow(),
        vec![Rev::Worktree, Rev::Commit(Oid::new("c7c7c7c"))]
    );

    // Non-matching query renders " No revisions match." and Enter is a safe no-op.
    press_text(&mut h, "zzzz");
    assert_eq!(h.screen_line(2), " No revisions match.");
    h.press(key!(enter)).force_draw();
    assert_eq!(opened.borrow().len(), 2);
}

#[test]
fn matching_working_tree_and_commits_together_keeps_separator_between_them() {
    let mut commits = decorated_commits();
    commits[2].subject = "feat: working tree status badge".to_owned();

    let mut h = harness(
        commits,
        Some("changed · 6 files"),
        false,
        100,
        10,
        None,
        None,
    );

    h.press(key!('/')).force_draw();
    press_text(&mut h, "working tree");

    let cursor_bg = Theme::DARK.normal.patch(Theme::DARK.cursor_line).bg;
    assert!(h.screen_line(2).starts_with(" Working tree"));
    assert_eq!(h.style_at(0, 2).bg, cursor_bg);
    assert!(h.screen_line(3).chars().all(|ch| ch == '─'));
    assert!(
        h.screen_line(4)
            .starts_with(" origin/feat/scrollbind  feat: working tree status badge"),
        "{:?}",
        h.screen_line(4)
    );
    assert_ne!(h.style_at(0, 4).bg, cursor_bg);

    // Clicking the separator (row 3: y=3 in component, local.y=1 in Scroll) does not change selection.
    h.click(10, 3).force_draw();
    assert_eq!(h.style_at(0, 2).bg, cursor_bg);

    // Clicking the matching commit row (y=4) selects it.
    h.click(10, 4).force_draw();
    assert_eq!(h.style_at(0, 4).bg, cursor_bg);
    assert_ne!(h.style_at(0, 2).bg, cursor_bg);
}

#[test]
fn multi_key_bursts_in_a_single_frame_preserve_every_keystroke_and_navigation() {
    let selected: Rc<RefCell<Vec<Rev>>> = Rc::new(RefCell::new(Vec::new()));
    let opened: Rc<RefCell<Vec<Rev>>> = Rc::new(RefCell::new(Vec::new()));
    let selected_cb = Rc::clone(&selected);
    let opened_cb = Rc::clone(&opened);

    let mut h = harness(
        decorated_commits(),
        Some("changed · 6 files"),
        false,
        110,
        10,
        Some(Rc::new(move |rev| {
            selected_cb.borrow_mut().push(rev);
        })),
        Some(Rc::new(move |rev| {
            opened_cb.borrow_mut().push(rev);
        })),
    );

    let cursor_bg = Theme::DARK.normal.patch(Theme::DARK.cursor_line).bg;

    // Burst 1: rapid `j`, `j`, `j` in a single frame without intermediate draws.
    h.press(key!(j)).press(key!(j)).press(key!(j));
    h.force_draw().force_draw();
    assert_eq!(
        h.style_at(0, 5).bg,
        cursor_bg,
        "expected commit #2 selected after 3 rapid j presses:\n{}",
        h.screen().join("\n")
    );
    assert_eq!(
        *selected.borrow(),
        vec![
            Rev::Commit(Oid::new("fb0d76c")),
            Rev::Commit(Oid::new("91dba92")),
            Rev::Commit(Oid::new("f8b977f")),
        ]
    );

    // Burst 2: pasting `"/v0.28"` in one crossterm poll batch without intermediate draws!
    for ch in "/v0.28".chars() {
        let combo =
            loom::crokey::KeyCombination::from(loom::crokey::crossterm::event::KeyCode::Char(ch));
        h.press(combo);
    }
    h.force_draw().force_draw();
    assert_eq!(h.screen_line(1), " > v0.28");
    assert!(h.screen_line(2).starts_with(" tag: v0.28.4"));
    assert!(h.screen_line(3).starts_with(" tag: v0.28.3"));
    assert_eq!(h.style_at(0, 2).bg, cursor_bg);

    // Burst 3: appending `".3"` + `Enter` in one batch selects/opens 89480e6 (v0.28.3)!
    for ch in ".3".chars() {
        let combo =
            loom::crokey::KeyCombination::from(loom::crokey::crossterm::event::KeyCode::Char(ch));
        h.press(combo);
    }
    h.press(key!(enter));
    h.force_draw().force_draw();
    assert_eq!(h.screen_line(1), " > v0.28.3");
    assert_eq!(*opened.borrow(), vec![Rev::Commit(Oid::new("89480e6"))]);

    // Burst 4: multiple Backspaces in a single batch (`".3"` removed -> back to `"v0.28"`),
    // followed by `Down` + `Enter` in the same batch!
    h.press(key!(backspace))
        .press(key!(backspace))
        .press(key!(down))
        .press(key!(enter));
    h.force_draw().force_draw();
    assert_eq!(h.screen_line(1), " > v0.28");
    assert_eq!(h.style_at(0, 3).bg, cursor_bg);
    assert_eq!(
        *opened.borrow(),
        vec![
            Rev::Commit(Oid::new("89480e6")),
            Rev::Commit(Oid::new("89480e6")),
        ]
    );

    // Burst 5: `Esc` + `q` in one batch closes search (restoring commit #2) and quits!
    h.press(key!(esc)).press(key!(q));
    h.force_draw();
    assert!(h.exiting());
}

#[test]
fn initial_commit_scrolls_into_view_both_on_mount_and_after_async_load_and_restores_after_search() {
    let commits: Vec<Commit> = (0..40)
        .map(|index| {
            let oid = format!("{index:07x}");
            let parent = format!("{:07x}", index + 1);
            let mut c = commit(&oid, &[&parent], &format!("subject-{index:02}"));
            if index == 2 {
                c.tags = vec!["early-tag".to_owned()];
            }
            c
        })
        .collect();
    let target_oid = Oid::new("0000019"); // commit #25 (`subject-25`)
    let cursor_bg = Theme::DARK.normal.patch(Theme::DARK.cursor_line).bg;

    // Case 1: `commits` already populated at mount.
    let mut sync_h = harness_with_props(
        RevisionPickerProps {
            commits: Rc::new(commits.clone()),
            worktree_summary: Some(Rc::from("changed · 6 files")),
            initial: Some(Rev::Commit(target_oid.clone())),
            on_select: None,
            on_open: None,
            on_close: None,
        },
        true,
        100,
        10,
    );

    let sync_row = selected_row_containing(&mut sync_h, "subject-25")
        .expect("expected commit #25 selected and visible at mount");
    let sync_screen = sync_h.screen();
    assert!(
        !sync_screen.iter().any(|line| line.contains("Working tree")),
        "expected viewport scrolled down to commit #25:\n{}",
        sync_screen.join("\n")
    );

    // Searching and pressing `Esc` restores back to commit #25 and its scroll offset.
    sync_h.press(key!('/')).force_draw();
    press_text(&mut sync_h, "early-tag");
    sync_h.force_draw();
    assert!(sync_h.screen_line(2).contains("subject-02"));
    sync_h.press(key!(esc)).force_draw().force_draw();
    assert_eq!(sync_h.screen(), sync_screen);
    assert_eq!(sync_h.style_at(0, sync_row).bg, cursor_bg);

    // Case 2: `commits` starts empty on first mount, then populates asynchronously on rerender.
    let mut async_h = harness_with_props(
        RevisionPickerProps {
            commits: Rc::new(Vec::new()),
            worktree_summary: Some(Rc::from("changed · 6 files")),
            initial: Some(Rev::Commit(target_oid.clone())),
            on_select: None,
            on_open: None,
            on_close: None,
        },
        true,
        100,
        10,
    );
    assert!(async_h.screen_line(1).starts_with(" Working tree"));
    assert_eq!(async_h.style_at(0, 1).bg, cursor_bg);

    async_h.set_props::<RevisionPicker>(RevisionPickerProps {
        commits: Rc::new(commits),
        worktree_summary: Some(Rc::from("changed · 6 files")),
        initial: Some(Rev::Commit(target_oid)),
        on_select: None,
        on_open: None,
        on_close: None,
    });
    for _ in 0..4 {
        async_h.force_draw();
    }

    let async_row = selected_row_containing(&mut async_h, "subject-25")
        .expect("expected async-loaded commit #25 selected and scrolled into view");
    let async_screen = async_h.screen();
    assert!(
        !async_screen
            .iter()
            .any(|line| line.contains("Working tree")),
        "expected async viewport scrolled past Working tree:\n{}",
        async_screen.join("\n")
    );

    // Searching and pressing `Esc` in the async-loaded picker also restores commit #25 + scroll!
    async_h.press(key!('/')).force_draw();
    press_text(&mut async_h, "early-tag");
    async_h.force_draw();
    assert!(async_h.screen_line(2).contains("subject-02"));
    async_h.press(key!(esc)).force_draw().force_draw();
    assert_eq!(async_h.screen(), async_screen);
    assert_eq!(async_h.style_at(0, async_row).bg, cursor_bg);
}

#[test]
fn two_step_esc_exits_search_before_invoking_on_close_and_q_invokes_on_close_without_exiting() {
    let close_count = Rc::new(RefCell::new(0usize));
    let close_cb = Rc::clone(&close_count);

    let mut h = harness_with_props(
        RevisionPickerProps {
            commits: Rc::new(decorated_commits()),
            worktree_summary: Some(Rc::from("changed · 6 files")),
            initial: None,
            on_select: None,
            on_open: None,
            on_close: Some(Rc::new(move || {
                *close_cb.borrow_mut() += 1;
            })),
        },
        false,
        110,
        10,
    );

    // Open search and type a query.
    h.press(key!('/')).force_draw();
    press_text(&mut h, "v0.28");
    assert_eq!(h.screen_line(1), " > v0.28");
    assert_eq!(*close_count.borrow(), 0);

    // First `Esc`: closes search and restores browse mode; does NOT call `on_close`.
    h.press(key!(esc)).force_draw();
    assert!(h.screen_line(0).ends_with("[/] search"));
    assert!(h.screen_line(1).contains("Working tree"));
    assert_eq!(*close_count.borrow(), 0);
    assert!(!h.exiting());

    // Second `Esc` in browse mode: calls `on_close` once without exiting the app.
    h.press(key!(esc)).force_draw();
    assert_eq!(*close_count.borrow(), 1);
    assert!(!h.exiting());

    // Pressing `q` in browse mode with `on_close: Some(...)` also calls `on_close` instead of `exit()`.
    h.press(key!(q)).force_draw();
    assert_eq!(*close_count.borrow(), 2);
    assert!(!h.exiting());
}

fn graph_fixture_commits() -> Vec<Commit> {
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

#[test]
fn empty_list_without_worktree_renders_placeholder_without_markers() {
    for branch_drawing in [false, true] {
        let mut h = harness(Vec::new(), None, branch_drawing, 80, 6, None, None);
        let screen = h.screen().join("\n");
        assert!(screen.contains("No commits"), "{screen}");
        assert!(!screen.contains('›'), "{screen}");
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
fn branch_drawing_and_box_drawing_modes_render_dag_curves_and_merge_glyphs() {
    let cursor_bg = Theme::DARK.normal.patch(Theme::DARK.cursor_line).bg;

    // BranchDrawing mode (row 0 is `git log`, rows 1..4 are commits when worktree_summary is None).
    let mut branch_h = harness(graph_fixture_commits(), None, true, 120, 7, None, None);
    let row1 = branch_h.screen_line(1);
    assert!(row1.starts_with(" \u{f5f6}"), "{row1:?}");
    assert_eq!(branch_h.style_at(0, 1).bg, cursor_bg);
    assert!(
        row1.contains("HEAD -> main · develop · origin/main · tag: v0.29.0"),
        "{row1:?}"
    );
    assert!(row1.contains("GitHub Actions"), "{row1:?}");
    assert!(row1.contains("09-10"), "{row1:?}");
    assert!(row1.contains("fb0d76c"), "{row1:?}");
    assert!(
        branch_h
            .screen_line(2)
            .starts_with(" \u{f604}\u{f5d0}\u{f5d7}")
    );
    assert!(branch_h.screen_line(3).starts_with(" \u{f5d1} \u{f5fa}"));
    let row4 = branch_h.screen_line(4);
    assert!(row4.starts_with(" \u{f600}\u{f5d0}\u{f5d9}"), "{row4:?}");
    assert!(row4.contains("HEAD  initial baseline commit"), "{row4:?}");

    // BoxDrawing fallback mode.
    let mut box_h = harness(graph_fixture_commits(), None, false, 120, 7, None, None);
    assert!(box_h.screen_line(1).starts_with(" ○"));
    assert_eq!(box_h.style_at(0, 1).bg, cursor_bg);
    assert_eq!(
        box_h.screen_line(2).chars().take(4).collect::<String>(),
        " ◎─╮"
    );
    assert_eq!(
        box_h.screen_line(3).chars().take(4).collect::<String>(),
        " │ ○"
    );
    assert_eq!(
        box_h.screen_line(4).chars().take(4).collect::<String>(),
        " ○─╯"
    );
}

#[test]
fn selecting_a_merge_commit_highlights_its_outgoing_pipes_in_both_modes() {
    for branch_drawing in [false, true] {
        let mut h = harness(
            graph_fixture_commits(),
            None,
            branch_drawing,
            100,
            7,
            None,
            None,
        );

        // Row 0 is header; row 1 = head, row 2 = merge, row 3 = topic, row 4 = root.
        // Pipe lane 0 starts at x = 1 (1-column left margin).
        assert!(!h.style_at(1, 3).add_modifier.contains(Modifier::BOLD));

        h.press(key!(j)).force_draw();
        let highlight_fg = Theme::DARK.line_number_current.fg;
        for x in [1u16, 2, 3] {
            assert_eq!(h.style_at(x, 2).fg, highlight_fg, "row 2 col {x}");
            assert!(h.style_at(x, 2).add_modifier.contains(Modifier::BOLD));
        }
        assert_eq!(h.style_at(1, 3).fg, highlight_fg);
        assert!(h.style_at(1, 3).add_modifier.contains(Modifier::BOLD));
        assert_eq!(h.style_at(1, 4).fg, highlight_fg);
        assert!(h.style_at(1, 4).add_modifier.contains(Modifier::BOLD));
    }
}

#[test]
fn ref_colours_and_lane_colours_never_emit_rgb_in_basic_themes() {
    for theme in Theme::ALL {
        for branch_drawing in [false, true] {
            let mut h = harness_with_theme(
                RevisionPickerProps {
                    commits: Rc::new(graph_fixture_commits()),
                    worktree_summary: Some(Rc::from("changed · 6 files")),
                    initial: None,
                    on_select: None,
                    on_open: None,
                    on_close: None,
                },
                theme,
                branch_drawing,
                120,
                8,
            );

            // Row 0: header, Row 1: Working tree, Row 2: separator, Row 3: first commit.
            let line3 = h.screen_line(3);
            let head_col = line3[..line3.find("HEAD -> main").unwrap()].chars().count() as u16;
            let branch_col = line3[..line3.find("develop").unwrap()].chars().count() as u16;
            let tag_col = line3[..line3.find("tag: v0.29.0").unwrap()].chars().count() as u16;

            assert_eq!(
                h.style_at(head_col, 3).fg,
                Some(theme.tree.heading),
                "{}",
                theme.name
            );
            assert_eq!(
                h.style_at(branch_col, 3).fg,
                Some(theme.tree.directory),
                "{}",
                theme.name
            );
            assert_eq!(
                h.style_at(tag_col, 3).fg,
                Some(theme.code.constant),
                "{}",
                theme.name
            );

            if theme.name.starts_with("basic-") {
                let cells = h.cells();
                for y in 0..8 {
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
fn start_and_end_keys_jump_across_eighty_virtualized_commits() {
    let commits: Vec<Commit> = (0..80)
        .map(|index| {
            let oid = format!("{index:07x}");
            let parent = format!("{:07x}", index + 1);
            commit(&oid, &[&parent], &format!("subject-{index:02}"))
        })
        .collect();

    let mut h = harness(commits, Some("changed · 6 files"), true, 90, 10, None, None);
    assert!(h.screen().join("\n").contains("Working tree"));
    assert!(h.screen().join("\n").contains("subject-00"));
    assert!(!h.screen().join("\n").contains("subject-40"));

    h.press(key!('$')).force_draw().force_draw();
    let bottom = h.screen().join("\n");
    assert!(bottom.contains("subject-79"), "{bottom}");
    assert!(!bottom.contains("subject-00"), "{bottom}");

    h.press(key!('0')).force_draw().force_draw();
    let top = h.screen().join("\n");
    assert!(top.contains("Working tree"), "{top}");
    assert!(top.contains("subject-00"), "{top}");
}

#[test]
fn reloading_commits_preserves_selected_commit_by_oid_and_settles() {
    let initial = vec![
        commit("1111111", &["2222222"], "first"),
        commit("2222222", &[], "target"),
    ];
    let mut h = harness(initial, Some("changed · 6 files"), true, 80, 8, None, None);
    // Row 0 = header, Row 1 = Working tree, Row 2 = separator, Row 3 = first, Row 4 = target.
    h.press(key!(j)).press(key!(j)).force_draw();
    let cursor_bg = Theme::DARK.normal.patch(Theme::DARK.cursor_line).bg;
    assert_eq!(h.style_at(0, 4).bg, cursor_bg);
    assert!(h.screen_line(4).contains("target"));

    let reloaded = vec![
        commit("0000000", &["1111111"], "newest"),
        commit("1111111", &["2222222"], "first"),
        commit("2222222", &[], "target"),
    ];
    h.set_props::<RevisionPicker>(RevisionPickerProps {
        commits: Rc::new(reloaded),
        worktree_summary: Some(Rc::from("changed · 6 files")),
        initial: None,
        on_select: None,
        on_open: None,
        on_close: None,
    });
    for _ in 0..5 {
        h.force_draw();
    }

    assert!(!h.needs_draw());
    assert_eq!(h.style_at(0, 5).bg, cursor_bg);
    assert!(h.screen_line(5).contains("target"));
}

#[test]
fn narrow_and_cjk_widths_never_panic_in_browse_or_search_modes() {
    let mut cjk = commit(
        "fb0d76c",
        &["91dba92"],
        "修复宽字符提交标题截断与分支绘制对齐 🦀",
    );
    cjk.current = true;
    cjk.branches = vec!["主分支".to_owned()];
    let commits = vec![cjk, commit("91dba92", &[], "second")];

    for width in [1u16, 2, 4, 8, 12, 20, 32, 60] {
        let mut h = harness(
            commits.clone(),
            Some("changed · 6 files"),
            true,
            width,
            6,
            None,
            None,
        );
        assert_eq!(h.screen().len(), 6);
        assert!(!h.screen().join("\n").contains('›'));
        h.press(key!('/')).force_draw();
        press_text(&mut h, "主分支");
        assert_eq!(h.screen().len(), 6);
    }
}
