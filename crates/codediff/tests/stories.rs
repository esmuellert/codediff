//! Every gallery story through the binary that users run.

#[cfg(unix)]
mod support;

#[cfg(unix)]
use std::io::Write;
use std::process::Command;
#[cfg(unix)]
use std::time::{Duration, Instant};

#[cfg(unix)]
use portable_pty::{CommandBuilder, PtySize, native_pty_system};
#[cfg(unix)]
use support::{ENTER_ALT, LEAVE_ALT, collect, drawn, drawn_after, on_a_terminal, written};

const EXPECTED_STORIES: &[(&str, &[&str], &[&str])] = &[
    ("welcome/default", &["Select a file to review."], &[]),
    ("explorer/empty", &[], &["Changes"]),
    ("explorer/tree", &["src", "button.rs", "README.md"], &[]),
    (
        "explorer/list",
        &["src/app.rs", "tests/app_test.rs"],
        &["├ ", "└ "],
    ),
    ("explorer/folded", &["src"], &["button.rs"]),
    ("explorer/selected", &["button.rs"], &[]),
    ("explorer/long-list", &["story-01.rs", "story-20.rs"], &[]),
    (
        "explorer/mixed-status",
        &["Staged Changes", "conflict.rs", "new feature.rs"],
        &[],
    ),
    (
        "explorer/awkward-paths",
        &["中文.rs", "with spaces.rs", "same-name.rs"],
        &[],
    ),
    ("side-by-side/unchanged", &["same on both sides", "│"], &[]),
    ("side-by-side/replacement", &["blue", "green", "│"], &[]),
    (
        "side-by-side/insert-delete",
        &["removed original", "inserted modified", "╱"],
        &[],
    ),
    ("side-by-side/tabs-unicode", &["你好", "您好", "│"], &[]),
    (
        "side-by-side/long-lines",
        &["ORIGINAL_LONG_PREFIX", "MODIFIED_LONG_PREFIX"],
        &[],
    ),
    (
        "side-by-side/edge-matrix",
        &["whitespace only", "deleted only", "你好"],
        &[],
    ),
    (
        "single-file/added",
        &["newly_added", "added story"],
        &["│", "╱"],
    ),
    (
        "single-file/deleted",
        &["removed_file", "deleted story"],
        &["│", "╱"],
    ),
    (
        "single-file/rust-syntax",
        &["fn highlighted", "let answer"],
        &["│", "╱"],
    ),
    (
        "single-file/long-lines",
        &["SINGLE_LONG_PREFIX", "short tail"],
        &["│", "╱"],
    ),
    ("single-file/empty", &[], &["  1 ", "│", "╱"]),
    (
        "single-file/long-syntax-file",
        &["generated_001", "generated_020"],
        &["│", "╱"],
    ),
];

fn snapshot(story: &str) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_codediff"))
        .args([
            "debug",
            "ui",
            story,
            "--snapshot",
            "--width",
            "100",
            "--height",
            "24",
        ])
        .output()
        .expect("running a story");
    assert!(
        output.status.success(),
        "{story} failed:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).expect("story snapshot is utf-8")
}

#[test]
fn list_names_every_story_in_order() {
    let output = Command::new(env!("CARGO_BIN_EXE_codediff"))
        .args(["debug", "ui", "--list"])
        .output()
        .expect("listing stories");
    assert!(output.status.success());
    let listed: Vec<&str> = std::str::from_utf8(&output.stdout)
        .expect("story list is utf-8")
        .lines()
        .collect();
    let expected: Vec<&str> = EXPECTED_STORIES.iter().map(|(name, _, _)| *name).collect();
    assert_eq!(listed, expected);
}

#[test]
fn catalog_snapshot_has_a_clear_two_line_menu() {
    let output = Command::new(env!("CARGO_BIN_EXE_codediff"))
        .args([
            "debug",
            "ui",
            "--snapshot",
            "--width",
            "100",
            "--height",
            "24",
        ])
        .output()
        .expect("rendering the catalog");
    assert!(output.status.success());
    let screen = String::from_utf8(output.stdout).expect("catalog is utf-8");
    let mut lines = screen.lines();

    assert_eq!(lines.next(), Some(" STORIES  21"));
    let menu = lines.next().unwrap_or_default();
    for label in ["j/k Select", "Enter Open", "/ Filter", "q Quit"] {
        assert!(menu.contains(label), "missing {label:?}: {menu:?}");
    }
    assert!(screen.contains("explorer/tree"));
    assert!(screen.contains("Nested changed files in tree mode"));
}

#[test]
fn an_unknown_story_points_to_the_catalog() {
    let output = Command::new(env!("CARGO_BIN_EXE_codediff"))
        .args(["debug", "ui", "not-a-story", "--snapshot"])
        .output()
        .expect("running an unknown story");

    assert!(!output.status.success());
    let error = String::from_utf8_lossy(&output.stderr);
    assert!(error.contains("unknown UI story"), "{error}");
    assert!(error.contains("--list"), "{error}");
}

#[test]
fn every_story_opens_through_the_binary_with_its_expected_content() {
    let mut failures = Vec::new();
    for (story, required_text, forbidden_text) in EXPECTED_STORIES {
        let screen = snapshot(story);
        let heading = screen.lines().next().unwrap_or_default();
        if !heading.contains("STORY") || !heading.contains(story) {
            failures.push(format!("{story}: missing gallery heading\n{screen}"));
            continue;
        }
        for marker in *required_text {
            if !screen.contains(marker) {
                failures.push(format!("{story}: missing {marker:?}\n{screen}"));
            }
        }
        for marker in *forbidden_text {
            if screen.contains(marker) {
                failures.push(format!(
                    "{story}: unexpectedly contains {marker:?}\n{screen}"
                ));
            }
        }
        if *story == "side-by-side/unchanged" && screen.matches("same on both sides").count() != 2 {
            failures.push(format!(
                "{story}: unchanged text did not appear twice\n{screen}"
            ));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}

#[test]
#[cfg(unix)]
fn every_story_draws_on_a_real_terminal() {
    let stories = [
        ("welcome/default", "Select a file to review."),
        ("explorer/empty", "explorer/empty"),
        ("explorer/tree", "button.rs"),
        ("explorer/list", "src/app.rs"),
        ("explorer/folded", "explorer/folded"),
        ("explorer/selected", "button.rs"),
        ("explorer/long-list", "story-01.rs"),
        ("explorer/mixed-status", "conflict.rs"),
        ("explorer/awkward-paths", "with spaces.rs"),
        ("side-by-side/unchanged", "same on both sides"),
        ("side-by-side/replacement", "green"),
        ("side-by-side/insert-delete", "inserted modified"),
        ("side-by-side/tabs-unicode", "您"),
        ("side-by-side/long-lines", "MODIFIED"),
        ("side-by-side/edge-matrix", "whitespace only"),
        ("single-file/added", "newly_added"),
        ("single-file/deleted", "removed_file"),
        ("single-file/rust-syntax", "highlighted"),
        ("single-file/long-lines", "SINGLE_LONG_PREFIX"),
        ("single-file/empty", "single-file/empty"),
        ("single-file/long-syntax-file", "generated_001"),
    ];

    for (story, marker) in stories {
        let (output, ok) = on_a_terminal(&["debug", "ui", story], None, b"q");
        assert!(ok, "{story} exited unsuccessfully:\n{output}");
        assert!(output.contains(ENTER_ALT), "{story} never took the screen");
        assert!(
            output.contains(marker),
            "{story} did not draw {marker:?}:\n{output:?}"
        );
        assert!(output.contains(LEAVE_ALT), "{story} kept the screen");
    }
}

#[test]
#[cfg(unix)]
fn a_story_accepts_keys_wheels_resize_and_quit() {
    let pty = native_pty_system()
        .openpty(PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        })
        .expect("opening a pty");
    let mut command = CommandBuilder::new(env!("CARGO_BIN_EXE_codediff"));
    command.args(["debug", "ui", "side-by-side/long-lines"]);
    command.env("TERM", "xterm-256color");
    let mut child = pty.slave.spawn_command(command).expect("spawning codediff");
    drop(pty.slave);

    let reader = pty.master.try_clone_reader().expect("reading the pty");
    let (collector, output) = collect(reader);
    drawn(&output);
    let before = output
        .lock()
        .expect("nothing else holds the lock")
        .bytes
        .len();

    let mut writer = pty.master.take_writer().expect("writing to the pty");
    writer
        .write_all(b"jll\x1b[<65;10;3M\x1b[<67;10;3M")
        .expect("sending key and wheel input");
    writer.flush().expect("flushing input");
    pty.master
        .resize(PtySize {
            rows: 30,
            cols: 100,
            pixel_width: 0,
            pixel_height: 0,
        })
        .expect("resizing the pty");
    drawn_after(&output, before);

    writer.write_all(b"q").expect("quitting");
    writer.flush().expect("flushing quit");
    drop(writer);
    let deadline = Instant::now() + Duration::from_secs(20);
    let status = loop {
        if let Some(status) = child.try_wait().expect("waiting for codediff") {
            break status;
        }
        assert!(Instant::now() < deadline, "the story never exited");
        std::thread::sleep(Duration::from_millis(25));
    };
    drop(pty.master);
    let output = written(collector, &output);

    assert!(status.success(), "{output}");
    assert!(output.contains(ENTER_ALT));
    let resized_frame = output.rsplit("\u{1b}[2J").next().unwrap_or(&output);
    assert!(
        resized_frame.contains("row 32"),
        "the vertical viewport never moved: {output:?}"
    );
    assert!(
        resized_frame.contains("nal"),
        "the horizontal viewport never moved: {output:?}"
    );
    assert!(output.contains(LEAVE_ALT));
}
