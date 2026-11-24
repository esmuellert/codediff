//! Tests for the `RevisionPicker` component.

#![allow(clippy::type_complexity)]

use std::cell::RefCell;
use std::rc::Rc;

use std::path::Path;

use file_types::{CommitHash, File, RepoPath, Revs};
use loom::crokey::key;
use loom::testing::Harness;
use loom::{Node, Scope, component, rsx, use_memo, use_state};
use ratatui::style::{Color, Modifier};
use ui::RevisionRange;
use ui::components::{Context, RevisionPicker, RevisionSide, Ui, UiProps};
use ui::services::VcsService;
use ui::{Commit, Theme};

fn commit(hash: &str, parents: &[&str], subject: &str) -> Commit {
    Commit {
        hash: CommitHash::new(hash),
        parents: parents.iter().copied().map(CommitHash::new).collect(),
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

fn fake_worktree_files(count: usize) -> Vec<File> {
    (1..=count)
        .map(|i| {
            File::unchanged_path(
                RepoPath::new(format!("src/file_{i}.rs"), Path::new("/repo")),
                Revs::worktree_against(CommitHash::new("base")),
            )
        })
        .collect()
}

#[component]
fn PickerHost(
    scope: &mut Scope,
    commits: Rc<Vec<Commit>>,
    worktree_files: Option<usize>,
    initial_range: RevisionRange,
    initial_picking: Option<RevisionSide>,
    theme: Theme,
    branch_drawing: bool,
    range_log: Option<Rc<RefCell<RevisionRange>>>,
    picking_log: Option<Rc<RefCell<Option<RevisionSide>>>>,
) -> Node {
    let init_range = initial_range.clone();
    let init_picking = *initial_picking;
    let (revision_range, set_revision_range) = use_state(scope, move || init_range);
    let (picking, set_picking) = use_state(scope, move || init_picking);
    if let Some(log) = range_log.as_ref() {
        *log.borrow_mut() = revision_range.clone();
    }
    if let Some(log) = picking_log.as_ref() {
        *log.borrow_mut() = picking;
    }
    let base = use_memo(scope, (), Context::default);
    let theme_val = *theme;
    let theme_rc = use_memo(scope, theme_val.name, move || theme_val);
    let files_count = *worktree_files;
    let vcs_service = use_memo(scope, files_count, move || {
        let files = files_count.map(fake_worktree_files).unwrap_or_default();
        VcsService::new(|| {}, move |_, _| Ok(files.clone()), |_, _| Ok(Vec::new()))
    });
    let mut ctx = (*base).clone();
    ctx.theme = theme_rc;
    ctx.branch_drawing = *branch_drawing;
    ctx.revision_range = revision_range;
    ctx.set_revision_range = Some(set_revision_range);
    ctx.picking = picking;
    ctx.set_picking = picking_log.is_some().then_some(set_picking);
    ctx.commits = Rc::clone(commits);
    ctx.vcs_service = files_count.map(|_| Rc::clone(&vcs_service));
    rsx! {
        Ui {
            value: ctx,
            RevisionPicker {}
        }
    }
}

fn mount_picker(props: PickerHostProps, width: u16, height: u16) -> Harness {
    let has_service = props.worktree_files.is_some();
    let mut h = Harness::new::<PickerHost>(props, width, height);
    for _ in 0..6 {
        h.force_draw();
    }
    if has_service {
        for _ in 0..50 {
            if h.screen()
                .iter()
                .any(|l| l.contains("changed ·") || l.contains("clean"))
            {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
            h.redraw_all();
            h.force_draw();
        }
        for _ in 0..4 {
            h.force_draw();
        }
    }
    h
}

fn harness(
    commits: Vec<Commit>,
    worktree_summary: Option<&str>,
    branch_drawing: bool,
    width: u16,
    height: u16,
) -> Harness {
    let (worktree_files, initial_picking) = match worktree_summary {
        Some("changed · 6 files") => (Some(6), None),
        Some("changed · 1 file") => (Some(1), None),
        Some(_) => (Some(0), None),
        None => (None, Some(RevisionSide::Left)),
    };
    mount_picker(
        PickerHostProps {
            commits: Rc::new(commits),
            worktree_files,
            initial_range: RevisionRange::Worktree,
            initial_picking,
            theme: Theme::DARK,
            branch_drawing,
            range_log: None,
            picking_log: None,
        },
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
    let mut h = harness(
        decorated_commits(),
        Some("changed · 6 files"),
        true,
        110,
        10,
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

    // `k` steps straight back across the separator to Working tree (row 1).
    h.press(key!(k)).force_draw();
    assert_eq!(h.style_at(0, 1).bg, cursor_bg);
}

#[test]
fn search_filters_into_compact_flat_list_without_pipes_and_treats_j_k_q_as_query_text() {
    let mut h = harness(
        decorated_commits(),
        Some("changed · 6 files"),
        false,
        110,
        10,
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
            let hash = format!("{index:07x}");
            let parent = format!("{:07x}", index + 1);
            let mut c = commit(&hash, &[&parent], &format!("subject-{index:02}"));
            if index == 2 || index == 30 {
                c.tags = vec![format!("match-tag-{index:02}")];
            }
            c
        })
        .collect();

    let mut h = harness(commits, Some("changed · 6 files"), true, 100, 10);

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

    let range_log = Rc::new(RefCell::new(RevisionRange::Worktree));
    let mut h = mount_picker(
        PickerHostProps {
            commits: Rc::new(commits),
            worktree_files: Some(6),
            initial_range: RevisionRange::Worktree,
            initial_picking: Some(RevisionSide::Right),
            theme: Theme::DARK,
            branch_drawing: true,
            range_log: Some(Rc::clone(&range_log)),
            picking_log: None,
        },
        100,
        10,
    );

    // Enter on initial row opens Rev::Worktree.
    h.press(key!(enter)).force_draw();
    assert_eq!(*range_log.borrow(), RevisionRange::Worktree);

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
        *range_log.borrow(),
        RevisionRange::Between("fb0d76c".to_owned(), "c7c7c7c".to_owned())
    );

    // Non-matching query renders " No revisions match." and Enter is a safe no-op.
    press_text(&mut h, "zzzz");
    assert_eq!(h.screen_line(2), " No revisions match.");
    h.press(key!(enter)).force_draw();
}

#[test]
fn matching_working_tree_and_commits_together_keeps_separator_between_them() {
    let mut commits = decorated_commits();
    commits[2].subject = "feat: working tree status badge".to_owned();

    let mut h = harness(commits, Some("changed · 6 files"), false, 100, 10);

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
    let range_log = Rc::new(RefCell::new(RevisionRange::Worktree));
    let mut h = mount_picker(
        PickerHostProps {
            commits: Rc::new(decorated_commits()),
            worktree_files: Some(6),
            initial_range: RevisionRange::Worktree,
            initial_picking: Some(RevisionSide::Right),
            theme: Theme::DARK,
            branch_drawing: true,
            range_log: Some(Rc::clone(&range_log)),
            picking_log: None,
        },
        100,
        10,
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
    assert_eq!(
        *range_log.borrow(),
        RevisionRange::Between("fb0d76c".to_owned(), "89480e6".to_owned())
    );

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
        *range_log.borrow(),
        RevisionRange::Between("fb0d76c".to_owned(), "89480e6".to_owned())
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
            let hash = format!("{index:07x}");
            let parent = format!("{:07x}", index + 1);
            let mut c = commit(&hash, &[&parent], &format!("subject-{index:02}"));
            if index == 2 {
                c.tags = vec!["early-tag".to_owned()];
            }
            c
        })
        .collect();
    let target_hash = CommitHash::new("0000019"); // commit #25 (`subject-25`)
    let cursor_bg = Theme::DARK.normal.patch(Theme::DARK.cursor_line).bg;

    // Case 1: `commits` already populated at mount.
    let mut sync_h = mount_picker(
        PickerHostProps {
            commits: Rc::new(commits.clone()),
            worktree_files: Some(6),
            initial_range: RevisionRange::Between(
                "HEAD".to_owned(),
                target_hash.as_str().to_owned(),
            ),
            initial_picking: Some(RevisionSide::Right),
            theme: Theme::DARK,
            branch_drawing: true,
            range_log: None,
            picking_log: None,
        },
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
    let mut async_h = mount_picker(
        PickerHostProps {
            commits: Rc::new(Vec::new()),
            worktree_files: Some(6),
            initial_range: RevisionRange::Between(
                "HEAD".to_owned(),
                target_hash.as_str().to_owned(),
            ),
            initial_picking: Some(RevisionSide::Right),
            theme: Theme::DARK,
            branch_drawing: true,
            range_log: None,
            picking_log: None,
        },
        100,
        10,
    );
    assert!(async_h.screen_line(1).starts_with(" Working tree"));
    assert_eq!(async_h.style_at(0, 1).bg, cursor_bg);

    async_h.set_props::<PickerHost>(PickerHostProps {
        commits: Rc::new(commits),
        worktree_files: None,
        initial_range: RevisionRange::Against(target_hash.as_str().to_owned()),
        initial_picking: Some(RevisionSide::Left),
        theme: Theme::DARK,
        branch_drawing: true,
        range_log: None,
        picking_log: None,
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
fn two_step_esc_exits_search_before_closing_picking_and_q_closes_without_exiting() {
    let picking_log = Rc::new(RefCell::new(Some(RevisionSide::Right)));

    let mut h = mount_picker(
        PickerHostProps {
            commits: Rc::new(decorated_commits()),
            worktree_files: Some(6),
            initial_range: RevisionRange::Worktree,
            initial_picking: Some(RevisionSide::Right),
            theme: Theme::DARK,
            branch_drawing: false,
            range_log: None,
            picking_log: Some(Rc::clone(&picking_log)),
        },
        110,
        10,
    );

    // Open search and type a query.
    h.press(key!('/')).force_draw();
    press_text(&mut h, "v0.28");
    assert_eq!(h.screen_line(1), " > v0.28");
    assert_eq!(*picking_log.borrow(), Some(RevisionSide::Right));

    // First `Esc`: closes search and restores browse mode; keeps `picking` open.
    h.press(key!(esc)).force_draw();
    assert!(h.screen_line(0).ends_with("[/] search"));
    assert!(h.screen_line(1).contains("Working tree"));
    assert_eq!(*picking_log.borrow(), Some(RevisionSide::Right));
    assert!(!h.exiting());

    // Second `Esc` in browse mode: clears `picking` without exiting the app.
    h.press(key!(esc)).force_draw();
    assert_eq!(*picking_log.borrow(), None);
    assert!(!h.exiting());

    // Pressing `q` in browse mode with `picking: Some(...)` also clears `picking` instead of `exit()`.
    let picking_log_q = Rc::new(RefCell::new(Some(RevisionSide::Left)));
    let mut h_q = mount_picker(
        PickerHostProps {
            commits: Rc::new(decorated_commits()),
            worktree_files: None,
            initial_range: RevisionRange::Worktree,
            initial_picking: Some(RevisionSide::Left),
            theme: Theme::DARK,
            branch_drawing: false,
            range_log: None,
            picking_log: Some(Rc::clone(&picking_log_q)),
        },
        110,
        10,
    );
    h_q.press(key!(q)).force_draw();
    assert_eq!(*picking_log_q.borrow(), None);
    assert!(!h_q.exiting());
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
        let mut h = harness(Vec::new(), None, branch_drawing, 80, 6);
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
    let mut branch_h = harness(graph_fixture_commits(), None, true, 120, 7);
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
    let mut box_h = harness(graph_fixture_commits(), None, false, 120, 7);
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
        let mut h = harness(graph_fixture_commits(), None, branch_drawing, 100, 7);

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
            let mut h = mount_picker(
                PickerHostProps {
                    commits: Rc::new(graph_fixture_commits()),
                    worktree_files: Some(6),
                    initial_range: RevisionRange::Worktree,
                    initial_picking: None,
                    theme,
                    branch_drawing,
                    range_log: None,
                    picking_log: None,
                },
                120,
                12,
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
            let hash = format!("{index:07x}");
            let parent = format!("{:07x}", index + 1);
            commit(&hash, &[&parent], &format!("subject-{index:02}"))
        })
        .collect();

    let mut h = harness(commits, Some("changed · 6 files"), true, 90, 10);
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
fn reloading_commits_preserves_selected_commit_by_hash_and_settles() {
    let initial = vec![
        commit("1111111", &["2222222"], "first"),
        commit("2222222", &[], "target"),
    ];
    let mut h = harness(initial, Some("changed · 6 files"), true, 80, 8);
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
    h.set_props::<PickerHost>(PickerHostProps {
        commits: Rc::new(reloaded),
        worktree_files: Some(6),
        initial_range: RevisionRange::Worktree,
        initial_picking: None,
        theme: Theme::DARK,
        branch_drawing: true,
        range_log: None,
        picking_log: None,
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
        let mut h = harness(commits.clone(), Some("changed · 6 files"), true, width, 6);
        assert_eq!(h.screen().len(), 6);
        assert!(!h.screen().join("\n").contains('›'));
        h.press(key!('/')).force_draw();
        press_text(&mut h, "主分支");
        assert_eq!(h.screen().len(), 6);
    }
}

#[test]
fn clicking_an_unselected_row_selects_it_and_clicking_again_opens_it() {
    let commits = vec![
        commit("aaaaaaa", &["bbbbbbb"], "first commit"),
        commit("bbbbbbb", &["ccccccc"], "second commit"),
        commit("ccccccc", &[], "third commit"),
    ];
    let range_log = Rc::new(RefCell::new(RevisionRange::Worktree));
    let picking_log = Rc::new(RefCell::new(Some(RevisionSide::Left)));
    let mut h = mount_picker(
        PickerHostProps {
            commits: Rc::new(commits),
            worktree_files: None,
            initial_range: RevisionRange::Worktree,
            initial_picking: Some(RevisionSide::Left),
            theme: Theme::DARK,
            branch_drawing: false,
            range_log: Some(Rc::clone(&range_log)),
            picking_log: Some(Rc::clone(&picking_log)),
        },
        80,
        8,
    );

    // Row 0 is header, row 1 is first commit, row 2 is second commit
    h.click(5, 2);
    assert_eq!(*range_log.borrow(), RevisionRange::Worktree);
    assert_eq!(*picking_log.borrow(), Some(RevisionSide::Left));

    // Clicking the now-selected row applies it (same as Enter)
    h.click(5, 2);
    h.force_draw();
    assert_eq!(
        *range_log.borrow(),
        RevisionRange::Against("bbbbbbb".to_owned())
    );
    assert_eq!(*picking_log.borrow(), None);

    // Rapid two-click batch on third commit (row 3) without intermediate draw
    let range_log2 = Rc::new(RefCell::new(RevisionRange::Worktree));
    let mut h2 = mount_picker(
        PickerHostProps {
            commits: Rc::new(vec![
                commit("aaaaaaa", &["bbbbbbb"], "first commit"),
                commit("bbbbbbb", &["ccccccc"], "second commit"),
                commit("ccccccc", &[], "third commit"),
            ]),
            worktree_files: None,
            initial_range: RevisionRange::Worktree,
            initial_picking: Some(RevisionSide::Left),
            theme: Theme::DARK,
            branch_drawing: false,
            range_log: Some(Rc::clone(&range_log2)),
            picking_log: None,
        },
        80,
        8,
    );
    h2.click(5, 3);
    h2.click(5, 3);
    h2.force_draw();
    assert_eq!(
        *range_log2.borrow(),
        RevisionRange::Against("ccccccc".to_owned())
    );
}
