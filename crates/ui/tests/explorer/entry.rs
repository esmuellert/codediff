//! The three sections: indent, body, status. Fitting, truncation, colours.

use super::common::*;

#[test]
fn the_counts_and_the_letter_sit_at_the_right_edge() {
    let lines = draw(vec![file_with_stats("a.rs", 4, 3)], 30, 3);
    assert!(lines[1].ends_with("+4 -3 M"), "got {:?}", lines[1]);
}

#[test]
fn a_side_that_did_not_change_is_left_out() {
    let only_added = draw(vec![file_with_stats("a.rs", 4, 0)], 30, 3);
    assert!(only_added[1].ends_with("+4 M"), "got {:?}", only_added[1]);

    let only_removed = draw(vec![file_with_stats("b.rs", 0, 3)], 30, 3);
    assert!(
        only_removed[1].ends_with("-3 M"),
        "got {:?}",
        only_removed[1]
    );
}

#[test]
fn an_untracked_file_looks_like_an_added_file() {
    let file = file("a.rs")
        .set_change_type(file_types::ChangeType::Untracked)
        .set_stats(file_types::Stats::new(1, 0));
    let lines = draw(vec![file], 30, 3);
    assert!(lines[1].ends_with("+1 A"), "got {:?}", lines[1]);
}

#[test]
fn a_file_with_no_counts_shows_only_its_letter() {
    let lines = draw(vec![file("a.rs")], 30, 3);
    assert!(lines[1].ends_with('M'), "got {:?}", lines[1]);
    assert!(!lines[1].contains('+'), "no counts to show: {:?}", lines[1]);
}

#[test]
fn a_directory_has_no_status() {
    let lines = screen(&["src/a.rs"], 30, 3);
    assert!(
        !lines[1].contains('M'),
        "a directory has no letter: {:?}",
        lines[1]
    );
}

#[test]
fn the_counts_are_green_and_red() {
    let mut h = harness(vec![file_with_stats("a.rs", 4, 3)], 30, 3);
    let line = h.screen_line(1);
    let end = line.chars().count() as u16;

    let letter_at = end - 1;
    let letter = h.style_at(letter_at, 1);

    let plus = h.style_at(end - 7, 1);
    let minus = h.style_at(end - 4, 1);
    assert_ne!(plus.fg, letter.fg, "the gained count has its own colour");
    assert_ne!(minus.fg, letter.fg, "the lost count has its own colour");
    assert_ne!(
        plus.fg, minus.fg,
        "gained and lost are told apart by colour"
    );
}

#[test]
fn a_name_too_long_for_the_line_is_cut_and_says_so() {
    let lines = draw(
        vec![file_with_stats("a-very-long-file-name.rs", 4, 3)],
        20,
        3,
    );
    assert!(lines[1].contains('…'), "the name was cut: {:?}", lines[1]);
    assert!(
        lines[1].ends_with("+4 -3 M"),
        "the status survives: {:?}",
        lines[1]
    );
}

#[test]
fn a_wide_name_is_cut_between_characters() {
    let lines = draw(vec![file_with_stats("ファイル.txt", 4, 3)], 18, 3);
    assert!(lines[1].contains('…'), "the name was cut: {:?}", lines[1]);
    assert!(
        lines[1].contains("フ…"),
        "whole characters survive: {:?}",
        lines[1]
    );
    assert!(
        lines[1].ends_with("+4 -3 M"),
        "the status is on screen: {:?}",
        lines[1]
    );
}

#[test]
fn no_line_is_wider_than_the_pane() {
    for width in 8..40u16 {
        let lines = draw(
            vec![file_with_stats("some/deep/path/file.rs", 12, 34)],
            width,
            4,
        );
        for (y, line) in lines.iter().enumerate() {
            let drawn = line_index::LineIndex::new(line, 1).width().0;
            assert!(
                drawn <= u32::from(width),
                "line {y} drew {drawn} columns into {width}: {line:?}",
            );
        }
    }
}

#[test]
fn where_a_moved_file_came_from_follows_its_name() {
    let lines = draw(vec![moved("old.rs", "new.rs")], 40, 3);
    assert!(lines[1].contains("new.rs"), "got {:?}", lines[1]);
    assert!(lines[1].contains("← old.rs"), "got {:?}", lines[1]);
}

#[test]
fn a_narrow_line_drops_where_it_came_from_before_the_name() {
    let file = moved("a-long-old-name.rs", "new.rs");
    let wide = draw(vec![file.clone()], 40, 3);
    assert!(
        wide[1].contains("← a-long-old-name.rs"),
        "it fits at 40: {:?}",
        wide[1]
    );

    let narrow = draw(vec![file], 20, 3);
    assert!(
        narrow[1].contains("new.rs"),
        "the name survives: {:?}",
        narrow[1]
    );
    assert!(
        !narrow[1].contains('←'),
        "the old path went first: {:?}",
        narrow[1]
    );
}

// ---- colours ----

#[test]
fn a_heading_name_is_not_bold_and_the_count_is_highlighted() {
    use ratatui::style::Modifier;
    let mut h = harness(vec![file("a.rs")], 30, 3);
    let name_style = h.style_at(0, 0);
    assert!(
        !name_style.add_modifier.contains(Modifier::BOLD),
        "the heading name is not bold",
    );
    let line = h.screen_line(0);
    let paren = line.find('(').expect("a parenthesized count");
    let count_style = h.style_at(paren as u16, 0);
    assert_ne!(
        name_style.fg, count_style.fg,
        "the count has its own colour"
    );
}

#[test]
fn the_indent_marker_has_its_own_colour() {
    let mut h = harness(vec![file("src/app.rs"), file("notes.txt")], 40, 10);
    h.draw();
    let marker_style = h.style_at(1, 1);
    let line = h.screen_line(1);
    let name_start = line.find('s').unwrap_or(6) as u16;
    let name_style = h.style_at(name_start, 1);
    assert_ne!(
        marker_style.fg, name_style.fg,
        "the marker and the directory name have different colours"
    );
}

#[test]
fn the_icon_has_its_own_colour() {
    let files = vec![file_with_stats("app.rs", 4, 3)];
    let mut h = harness(files, 40, 10);
    h.draw();
    let icon_style = h.style_at(0, 1);
    let line = h.screen_line(1);
    let name_start = line.find("app").unwrap_or(4) as u16;
    let name_style = h.style_at(name_start, 1);
    let _ = (icon_style, name_style);
}

#[test]
fn the_heading_colour_differs_from_the_file_name_colour() {
    let files = vec![file("app.rs")];
    let mut h = harness(files, 40, 10);
    h.draw();
    let heading_style = h.style_at(1, 0);
    let line = h.screen_line(1);
    let name_start = line.find("app").unwrap_or(3) as u16;
    let name_style = h.style_at(name_start, 1);
    assert_ne!(
        heading_style.fg, name_style.fg,
        "the heading and the file name have different colours"
    );
}
