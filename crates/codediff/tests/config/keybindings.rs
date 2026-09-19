use std::process::Command;
use std::time::Duration;

use serde_json::json;

use super::{Fixture, Run, run_app_steps, screen};

fn steps<'a>(items: &[(&'a [u8], u64)]) -> Vec<(&'a [u8], Duration)> {
    items
        .iter()
        .map(|(keys, pause)| (*keys, Duration::from_millis(*pause)))
        .collect()
}

fn repeated(key: u8, count: usize) -> Vec<u8> {
    vec![key; count]
}

fn assert_success(run: &Run) {
    assert!(
        run.success && !run.timed_out,
        "codediff did not exit cleanly: timed_out={}, output={}",
        run.timed_out,
        run.output
    );
}

#[test]
fn move_down_keybinding_opens_the_selected_file() {
    let fixture = Fixture::new("key-move-down");
    fixture.write("modified.txt", "one\nCUSTOM_DOWN\nthree\n");
    let run = run_app_steps(
        &fixture,
        json!({"keybindings": {"move_down": ["n"]}}),
        Some("modified.txt"),
        &steps(&[(b"n\r", 500), (b"q", 0)]),
    );

    assert_success(&run);
    assert!(screen(&run).contains("CUSTOM_DOWN"));
}

#[test]
fn move_up_keybinding_scrolls_back_to_the_first_line() {
    let fixture = Fixture::new("key-move-up");
    let text = (1..=40)
        .map(|line| format!("KEY_UP_LINE_{line:02}\n"))
        .collect::<String>();
    fixture.write("modified.txt", &text);
    let down = repeated(b'j', 20);
    let up = repeated(b'p', 20);
    let run = run_app_steps(
        &fixture,
        json!({"keybindings": {"move_up": ["p"]}}),
        Some("modified.txt"),
        &steps(&[(&down, 200), (&up, 200), (b"q", 0)]),
    );

    assert_success(&run);
    assert!(screen(&run).row_of("KEY_UP_LINE_01").is_some());
}

#[test]
fn open_keybinding_expands_a_directory() {
    let fixture = Fixture::new("key-open");
    fixture.write("aaa/inside.txt", "inside directory\n");
    let run = run_app_steps(
        &fixture,
        json!({
            "keybindings": {
                "move_down": ["n"],
                "open": ["o"]
            }
        }),
        Some("aaa"),
        &steps(&[(b"no", 500), (b"q", 0)]),
    );

    assert_success(&run);
    assert!(
        screen(&run).contains("inside.txt"),
        "directory did not open"
    );
}

#[test]
fn toggle_layout_keybinding_switches_the_running_view() {
    let fixture = Fixture::new("key-layout");
    fixture.write("modified.txt", "one\nTWO\nthree\n");
    let toggled = run_app_steps(
        &fixture,
        json!({
            "keybindings": {
                "toggle_layout": ["v"],
                "focus_next": ["f"]
            }
        }),
        Some("modified.txt"),
        &steps(&[(b"fv", 400), (b"q", 0)]),
    );
    let default = run_app_steps(
        &fixture,
        json!({}),
        Some("modified.txt"),
        &steps(&[(b"q", 0)]),
    );

    assert_success(&toggled);
    assert_success(&default);
    assert!(
        screen(&toggled).vertical_bar_columns().len()
            < screen(&default).vertical_bar_columns().len()
    );
}

#[test]
fn toggle_wrap_keybinding_changes_the_running_view() {
    let fixture = Fixture::new("key-wrap");
    let long = "0123456789 ".repeat(12);
    fixture.write("modified.txt", &format!("one\nKEY_WRAP {long}\nTAIL\n"));
    let toggled = run_app_steps(
        &fixture,
        json!({
            "keybindings": {
                "toggle_wrap": ["z"],
                "focus_next": ["f"]
            }
        }),
        Some("modified.txt"),
        &steps(&[(b"fz", 400), (b"q", 0)]),
    );
    let default = run_app_steps(
        &fixture,
        json!({}),
        Some("modified.txt"),
        &steps(&[(b"q", 0)]),
    );

    assert_success(&toggled);
    assert_success(&default);
    assert!(screen(&toggled).row_of("TAIL") < screen(&default).row_of("TAIL"));
}

#[test]
fn toggle_explorer_mode_keybinding_switches_to_list_mode() {
    let fixture = Fixture::new("key-explorer-mode");
    let run = run_app_steps(
        &fixture,
        json!({"keybindings": {"toggle_explorer_mode": ["m"]}}),
        None,
        &steps(&[(b"m", 400), (b"q", 0)]),
    );

    assert_success(&run);
    let screen = screen(&run);
    assert!(screen.contains("deep/only/one/chain/leaf.txt"));
    assert!(!screen.contains("├ ") && !screen.contains("└ "));
}

#[test]
fn stage_keybinding_stages_after_focus_previous() {
    let fixture = Fixture::new("key-stage");
    let run = run_app_steps(
        &fixture,
        json!({
            "keybindings": {
                "stage": ["s"],
                "focus_previous": ["u"]
            }
        }),
        Some("modified.txt"),
        &steps(&[(b"u", 200), (b"s", 800), (b"q", 0)]),
    );

    assert_success(&run);
    assert!(wait_for_staged(&fixture));
}

#[test]
fn focus_next_keybinding_returns_to_the_diff_view() {
    let fixture = Fixture::new("key-focus-next");
    let text = (1..=40)
        .map(|line| format!("KEY_FOCUS_LINE_{line:02}\n"))
        .collect::<String>();
    fixture.write("modified.txt", &text);
    let down = repeated(b'n', 20);
    let run = run_app_steps(
        &fixture,
        json!({
            "keybindings": {
                "focus_previous": ["u"],
                "focus_next": ["o"],
                "move_down": ["n"]
            }
        }),
        Some("modified.txt"),
        &steps(&[(b"u", 200), (b"o", 200), (&down, 500), (b"q", 0)]),
    );

    assert_success(&run);
    assert!(screen(&run).row_of("KEY_FOCUS_LINE_01").is_none());
}

#[test]
fn horizontal_move_keybindings_change_the_scroll_position() {
    let fixture = Fixture::new("key-horizontal");
    let line = format!("HORIZONTAL_KEY_MARKER {}\n", "x".repeat(120));
    fixture.write("modified.txt", &format!("one\n{line}tail\n"));
    let right = repeated(b'r', 20);
    let left = repeated(b'l', 20);
    let scrolled = run_app_steps(
        &fixture,
        json!({
            "ui": {"wrap": false},
            "keybindings": {
                "focus_next": ["f"],
                "move_right": ["r"],
                "move_left": ["l"]
            }
        }),
        Some("modified.txt"),
        &steps(&[(b"f", 200), (&right, 400), (b"q", 0)]),
    );
    let restored = run_app_steps(
        &fixture,
        json!({
            "ui": {"wrap": false},
            "keybindings": {
                "focus_next": ["f"],
                "move_right": ["r"],
                "move_left": ["l"]
            }
        }),
        Some("modified.txt"),
        &steps(&[(b"f", 100), (&right, 100), (&left, 400), (b"q", 0)]),
    );

    assert_success(&scrolled);
    assert_success(&restored);
    assert!(!screen(&scrolled).contains("HORIZONTAL_KEY_MARKER"));
    assert!(screen(&restored).contains("HORIZONTAL_KEY_MARKER"));
}

#[test]
fn horizontal_endpoint_keybindings_reach_start_and_end() {
    let fixture = Fixture::new("key-endpoints");
    let line = format!("ENDPOINT_KEY_MARKER {}\n", "x".repeat(120));
    fixture.write("modified.txt", &format!("one\n{line}tail\n"));
    let at_end = run_app_steps(
        &fixture,
        json!({
            "ui": {"wrap": false},
            "keybindings": {
                "focus_next": ["f"],
                "end": ["e"],
                "start": ["a"]
            }
        }),
        Some("modified.txt"),
        &steps(&[(b"f", 200), (b"e", 400), (b"q", 0)]),
    );
    let at_start = run_app_steps(
        &fixture,
        json!({
            "ui": {"wrap": false},
            "keybindings": {
                "focus_next": ["f"],
                "end": ["e"],
                "start": ["a"]
            }
        }),
        Some("modified.txt"),
        &steps(&[(b"f", 100), (b"e", 100), (b"a", 400), (b"q", 0)]),
    );

    assert_success(&at_end);
    assert_success(&at_start);
    assert!(!screen(&at_end).contains("ENDPOINT_KEY_MARKER"));
    assert!(screen(&at_start).contains("ENDPOINT_KEY_MARKER"));
}

fn wait_for_staged(fixture: &Fixture) -> bool {
    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    while std::time::Instant::now() < deadline {
        let output = Command::new("git")
            .args(["diff", "--cached", "--name-only"])
            .current_dir(&fixture.dir)
            .output()
            .expect("checking staged files");
        if String::from_utf8_lossy(&output.stdout)
            .lines()
            .any(|path| path == "modified.txt")
        {
            return true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    false
}
