#[path = "support/fake_services.rs"]
mod fake_services;

use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;

use diff::DiffContent;
use file_types::{CommitHash, File, RepoPath, Revs, Stats};
use loom::testing::Harness;
use loom::{Node, Scope, component, crokey, rsx};
use ui::Theme;
use ui::components::diff_viewer::{DiffViewer, DiffViewerProps};
use ui::components::{Context, DirectorySelection, Ui, UiProps};
use ui::ratatui::style::Modifier;

fn revs() -> Revs {
    Revs::worktree_against(CommitHash::new("base"))
}

fn path(relative: &str) -> RepoPath {
    RepoPath::new(relative, Path::new("/repo"))
}

fn make_diff(file: File, original: &[&str], modified: &[&str]) -> DiffContent {
    let changes = diff::compute(original, modified, diff::Settings::default()).unwrap();
    let alignment = diff::align(changes, original, modified).unwrap();
    DiffContent::Diff(diff::Diff { file, alignment })
}

fn single(file: File, lines: &[&str]) -> DiffContent {
    DiffContent::SingleFile(diff::SingleFile {
        file,
        lines: Arc::new(lines.iter().map(|line| (*line).to_owned()).collect()),
    })
}

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

fn find_in_screen(screen: &[String], label: &str) -> (u16, u16) {
    for (row, line) in screen.iter().enumerate() {
        if let Some(byte_offset) = line.find(label) {
            return (byte_to_col(line, byte_offset), row as u16);
        }
    }
    panic!("{label:?} missing from screen: {screen:?}");
}

#[component]
fn ViewerHost(_scope: &mut Scope, context: Context) -> Node {
    rsx! {
        Ui {
            value: context.clone(),
            DiffViewer {}
        }
    }
}

#[test]
fn welcome_hides_the_header_and_selected_file_opens_floating_menu_without_shifting_diff_rows() {
    let mut welcome =
        Harness::new::<DiffViewer>(DiffViewerProps {}, 90, 12).provide::<Ui>(Context {
            theme: Rc::new(Theme::DARK),
            ..Context::default()
        });
    let welcome_screen = welcome.screen();
    assert!(
        welcome_screen.join("\n").contains("Select a file"),
        "{welcome_screen:?}"
    );
    assert!(
        !welcome_screen.join("\n").contains("View ▾"),
        "welcome rendered View ▾: {welcome_screen:?}"
    );

    let modified =
        File::unchanged_path(path("src/modified.rs"), revs()).set_stats(Stats::new(4, 2));
    let renamed = File::renamed(path("src/old_name.rs"), path("src/new_name.rs"), revs())
        .set_stats(Stats::new(7, 3));
    let (diff, redraws) = fake_services::fake_diff(vec![
        make_diff(
            modified.clone(),
            &["left_row_1", "left_row_2"],
            &["right_row_1", "right_row_2"],
        ),
        make_diff(renamed.clone(), &["before"], &["after"]),
    ]);

    let mut harness = Harness::new::<ViewerHost>(
        ViewerHostProps {
            context: Context {
                theme: Rc::new(Theme::DARK),
                file: Some(Rc::new(modified)),
                diff: Some(Rc::clone(&diff)),
                ..Context::default()
            },
        },
        80,
        10,
    );
    fake_services::wait_diff(&diff, &redraws, &mut harness);

    let header = harness.screen_line(0);
    assert!(header.starts_with(" M  src/modified.rs"), "{header:?}");
    assert!(header.contains("View ▾"), "{header:?}");
    assert!(!header.contains("Side by side"), "{header:?}");
    assert!(!header.contains("Inline"), "{header:?}");
    assert!(!header.contains("Wrap lines"), "{header:?}");
    // First diff line is at y = 1.
    assert!(harness.screen_line(1).contains("left_row_1"));
    assert!(harness.screen_line(2).contains("left_row_2"));

    // Clicking View ▾ opens floating popover over right side without shifting left diff lines down.
    let trigger_col = col_of(&header, "View ▾");
    harness.click(trigger_col, 0).force_draw();
    let open_screen = harness.screen();
    let joined = open_screen.join("\n");
    assert!(joined.contains("● Side by side"), "{joined}");
    assert!(joined.contains("Inline"), "{joined}");
    assert!(joined.contains("✓ Wrap lines"), "{joined}");
    // Left side of rows 1 and 2 stays at the exact same y coordinates!
    assert!(open_screen[1].contains("left_row_1"), "{open_screen:?}");
    assert!(open_screen[2].contains("left_row_2"), "{open_screen:?}");

    // Clicking View ▾ again (via full-pane backdrop) closes the menu.
    harness.click(trigger_col, 0).force_draw();
    assert!(!harness.screen().join("\n").contains("Wrap lines"));

    // Renamed file shows `← previous` on the left alongside `View ▾` on the right.
    harness.set_props::<ViewerHost>(ViewerHostProps {
        context: Context {
            theme: Rc::new(Theme::DARK),
            file: Some(Rc::new(renamed)),
            diff: Some(Rc::clone(&diff)),
            ..Context::default()
        },
    });
    fake_services::wait_diff(&diff, &redraws, &mut harness);
    let renamed_header = harness.screen_line(0);
    assert!(
        renamed_header.starts_with(" R  src/new_name.rs ← src/old_name.rs"),
        "{renamed_header:?}"
    );
    assert!(renamed_header.contains("View ▾"), "{renamed_header:?}");
}

#[test]
fn dropdown_items_toggle_layout_and_wrap_and_esc_or_backdrop_closes_menu() {
    let selected = File::unchanged_path(path("src/layout.rs"), revs()).set_stats(Stats::new(1, 1));
    let long_old = "OLD_START_0123456789_0123456789_0123456789_OLD_TAIL";
    let long_new = "NEW_START_0123456789_0123456789_0123456789_NEW_TAIL";
    let (diff, redraws) =
        fake_services::fake_diff(vec![make_diff(selected.clone(), &[long_old], &[long_new])]);
    let mut harness = Harness::new::<ViewerHost>(
        ViewerHostProps {
            context: Context {
                theme: Rc::new(Theme::DARK),
                file: Some(Rc::new(selected)),
                diff: Some(Rc::clone(&diff)),
                ..Context::default()
            },
        },
        64,
        12,
    );
    fake_services::wait_diff(&diff, &redraws, &mut harness);

    let trigger_col = col_of(&harness.screen_line(0), "View ▾");

    // 1. Open menu -> click Inline -> switches layout & closes menu.
    harness.click(trigger_col, 0).force_draw();
    let (inline_x, inline_y) = find_in_screen(&harness.screen(), "Inline");
    harness.click(inline_x, inline_y).force_draw();
    assert!(
        !harness.screen().join("\n").contains("Wrap lines"),
        "selecting Inline must close menu"
    );
    assert!(
        !harness.screen()[1..].iter().any(|line| line.contains('│')),
        "Inline layout must remove side-by-side divider: {:?}",
        harness.screen()
    );

    // 2. Open menu -> click Side by side -> restores two columns & closes menu.
    harness.click(trigger_col, 0).force_draw();
    assert!(harness.screen().join("\n").contains("● Inline"));
    let (side_x, side_y) = find_in_screen(&harness.screen(), "Side by side");
    harness.click(side_x, side_y).force_draw();
    assert!(!harness.screen().join("\n").contains("Wrap lines"));
    assert!(harness.screen()[1..].iter().any(|line| line.contains('│')));

    // 3. Open menu -> click Wrap lines -> disables wrapping & closes menu.
    assert!(harness.screen().join("\n").contains("NEW_TAIL"));
    harness.click(trigger_col, 0).force_draw();
    let (wrap_x, wrap_y) = find_in_screen(&harness.screen(), "Wrap lines");
    harness.click(wrap_x, wrap_y).force_draw();
    assert!(!harness.screen().join("\n").contains("Wrap lines"));
    assert!(
        !harness.screen().join("\n").contains("NEW_TAIL"),
        "clicking Wrap lines did not disable wrapping: {:?}",
        harness.screen()
    );

    // 4. Open menu -> Esc closes menu without changing state; backdrop click also closes menu.
    harness.click(trigger_col, 0).force_draw();
    assert!(harness.screen().join("\n").contains("Wrap lines"));
    assert!(!harness.screen().join("\n").contains("✓ Wrap lines"));
    harness.press(crokey::key!(esc)).force_draw();
    assert!(!harness.screen().join("\n").contains("Wrap lines"));

    harness.click(trigger_col, 0).force_draw();
    assert!(harness.screen().join("\n").contains("Wrap lines"));
    harness.click(2, 5).force_draw();
    assert!(!harness.screen().join("\n").contains("Wrap lines"));
}

#[test]
fn one_sided_file_dropdown_shows_all_view_options_and_pre_switches_layout() {
    let added = File::added(path("src/added.rs"), revs()).set_stats(Stats::new(3, 0));
    let two_sided =
        File::unchanged_path(path("src/modified.rs"), revs()).set_stats(Stats::new(1, 1));
    let body = "one_sided_0123456789_0123456789_0123456789_0123456789_0123456789_TAIL";
    let (diff, redraws) = fake_services::fake_diff(vec![
        single(added.clone(), &[body]),
        make_diff(two_sided.clone(), &["old_line"], &["new_line"]),
    ]);
    let mut harness = Harness::new::<ViewerHost>(
        ViewerHostProps {
            context: Context {
                theme: Rc::new(Theme::DARK),
                file: Some(Rc::new(added)),
                diff: Some(Rc::clone(&diff)),
                ..Context::default()
            },
        },
        60,
        10,
    );
    fake_services::wait_diff(&diff, &redraws, &mut harness);

    let header = harness.screen_line(0);
    assert!(header.starts_with(" A  src/added.rs"), "{header:?}");
    assert!(header.contains("View ▾"), "{header:?}");
    assert!(harness.screen().join("\n").contains("TAIL"));

    let trigger_col = col_of(&header, "View ▾");
    harness.click(trigger_col, 0).force_draw();
    let menu_text = harness.screen().join("\n");
    assert!(menu_text.contains("● Side by side"), "{menu_text}");
    assert!(menu_text.contains("Inline"), "{menu_text}");
    assert!(menu_text.contains("✓ Wrap lines"), "{menu_text}");

    // Pre-switching to Inline while viewing a one-sided file applies when opening a two-sided diff.
    let (inline_x, inline_y) = find_in_screen(&harness.screen(), "Inline");
    harness.click(inline_x, inline_y).force_draw();

    harness.click(trigger_col, 0).force_draw();
    assert!(harness.screen().join("\n").contains("● Inline"));
    let (wrap_x, wrap_y) = find_in_screen(&harness.screen(), "Wrap lines");
    harness.click(wrap_x, wrap_y).force_draw();
    assert!(!harness.screen().join("\n").contains("TAIL"));

    harness.set_props::<ViewerHost>(ViewerHostProps {
        context: Context {
            theme: Rc::new(Theme::DARK),
            file: Some(Rc::new(two_sided)),
            diff: Some(Rc::clone(&diff)),
            ..Context::default()
        },
    });
    fake_services::wait_diff(&diff, &redraws, &mut harness);
    assert!(
        !harness.screen()[1..].iter().any(|line| line.contains('│')),
        "pre-selected Inline layout did not carry over to two-sided diff: {:?}",
        harness.screen()
    );
}

#[test]
fn directory_selection_shows_folder_path_on_left_and_dropdown_switches_cards() {
    let first =
        File::unchanged_path(path("src/components/a.rs"), revs()).set_stats(Stats::new(1, 1));
    let second =
        File::unchanged_path(path("src/components/b.rs"), revs()).set_stats(Stats::new(2, 0));
    let added = File::added(path("src/components/new.rs"), revs());
    let (diff, redraws) = fake_services::fake_diff(vec![
        make_diff(first.clone(), &["old_a"], &["new_a"]),
        make_diff(second.clone(), &["old_b"], &["new_b"]),
    ]);
    let directory = Rc::new(DirectorySelection {
        path: path("src/components"),
        revs: revs(),
        files: Rc::new(vec![first, second, added]),
    });
    let mut harness = Harness::new::<ViewerHost>(
        ViewerHostProps {
            context: Context {
                theme: Rc::new(Theme::DARK),
                directory: Some(directory),
                diff: Some(Rc::clone(&diff)),
                ..Context::default()
            },
        },
        80,
        18,
    );
    fake_services::wait_diff(&diff, &redraws, &mut harness);

    let header = harness.screen_line(0);
    assert!(header.contains("src/components (3)"), "{header:?}");
    assert!(header.contains("View ▾"), "{header:?}");
    assert_eq!(
        harness.style_at(col_of(&header, "src/components"), 0).fg,
        Some(Theme::DARK.tree.directory)
    );
    assert_eq!(
        harness.style_at(col_of(&header, "(3)"), 0).fg,
        Some(Theme::DARK.tree.count)
    );

    let card_body_has_divider = |screen: &[String]| {
        screen
            .iter()
            .skip(1)
            .filter(|line| line.contains("old_a") || line.contains("new_a"))
            .any(|line| {
                let inner = line.trim_start_matches('│').trim_end_matches('│');
                inner.contains('│')
            })
    };
    assert!(card_body_has_divider(&harness.screen()));

    let trigger_col = col_of(&header, "View ▾");
    harness.click(trigger_col, 0).force_draw();
    let (inline_x, inline_y) = find_in_screen(&harness.screen(), "Inline");
    harness.click(inline_x, inline_y).force_draw();
    assert!(
        !card_body_has_divider(&harness.screen()),
        "selecting Inline from directory dropdown did not switch DiffCards: {:?}",
        harness.screen()
    );
}

#[test]
fn every_theme_highlights_open_trigger_and_menu_items_cleanly() {
    let selected = File::unchanged_path(path("src/theme.rs"), revs());
    for theme in Theme::ALL {
        let (diff, redraws) =
            fake_services::fake_diff(vec![make_diff(selected.clone(), &["old"], &["new"])]);
        let mut harness = Harness::new::<ViewerHost>(
            ViewerHostProps {
                context: Context {
                    theme: Rc::new(theme),
                    file: Some(Rc::new(selected.clone())),
                    diff: Some(Rc::clone(&diff)),
                    ..Context::default()
                },
            },
            70,
            10,
        );
        fake_services::wait_diff(&diff, &redraws, &mut harness);

        let header = harness.screen_line(0);
        let trigger_col = col_of(&header, "View ▾");
        let closed_style = harness.style_at(trigger_col, 0);
        assert_eq!(closed_style.bg, theme.normal.bg, "{}", theme.name);

        harness.click(trigger_col, 0).force_draw();
        let open_style = harness.style_at(trigger_col, 0);
        assert_ne!(open_style, closed_style, "{}", theme.name);
        assert!(
            open_style.add_modifier.contains(Modifier::UNDERLINED),
            "{}",
            theme.name
        );
        let (side_x, side_y) = find_in_screen(&harness.screen(), "Side by side");
        let (inline_x, inline_y) = find_in_screen(&harness.screen(), "Inline");
        assert_ne!(
            harness.style_at(side_x, side_y),
            harness.style_at(inline_x, inline_y),
            "{}",
            theme.name
        );
    }
}
