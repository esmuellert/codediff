//! Browsing and switching stories without restarting the binary.
#![cfg(unix)]

mod support;

use std::fs;
use std::io::Write;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use portable_pty::{CommandBuilder, PtySize, native_pty_system};
use support::{ENTER_ALT, LEAVE_ALT, Output, collect, drawn, drawn_after, written};

#[test]
fn catalog_filters_opens_switches_resets_and_returns() {
    let pty = native_pty_system()
        .openpty(PtySize {
            rows: 24,
            cols: 100,
            pixel_width: 0,
            pixel_height: 0,
        })
        .expect("opening a pty");
    let mut command = CommandBuilder::new(env!("CARGO_BIN_EXE_codediff"));
    command.args(["debug", "ui"]);
    command.env("TERM", "xterm-256color");
    let mut child = pty.slave.spawn_command(command).expect("spawning codediff");
    drop(pty.slave);

    let reader = pty.master.try_clone_reader().expect("reading the pty");
    let (collector, output) = collect(reader);
    drawn(&output);
    assert!(all_output(&output).contains("STORIES"));

    let mut writer = pty.master.take_writer().expect("writing to the pty");
    let search = send_and_wait(&mut writer, &output, b"/");
    assert!(search.contains("FILTER"), "filter did not open: {search:?}");
    let filtered = send_and_wait(&mut writer, &output, b"edge-matrix");
    assert!(
        filtered.contains("edge-matrix"),
        "filter text did not arrive: {filtered:?}"
    );

    let opened = send_and_wait(&mut writer, &output, b"\r");
    assert!(
        opened.contains("fn edge_matrix()"),
        "story did not open: {opened:?}"
    );

    let next = send_and_wait(&mut writer, &output, b"]");
    assert!(
        next.contains("inline/unchanged") && next.contains("same in one column"),
        "next story did not open: {next:?}"
    );
    let previous = send_and_wait(&mut writer, &output, b"[");
    assert!(
        previous.contains("edge_matrix"),
        "previous story did not open: {previous:?}"
    );
    let _ = send_and_wait(&mut writer, &output, b"r");
    let catalog = send_and_wait(&mut writer, &output, b"\x1b");
    assert!(
        catalog.contains("Welcome") && catalog.contains("explorer/empty"),
        "catalog did not return: {catalog:?}"
    );

    let _ = send_and_wait(&mut writer, &output, b"/");
    let _ = send_and_wait(&mut writer, &output, b"explorer/list");
    let explorer = send_and_wait(&mut writer, &output, b"\r");
    assert!(
        explorer.contains("src/app.rs"),
        "Explorer setup keys were not applied: {explorer:?}"
    );
    let _ = send_and_wait(&mut writer, &output, b"\x1b");

    writer.write_all(b"q").expect("quitting");
    writer.flush().expect("flushing quit");
    drop(writer);
    let deadline = Instant::now() + Duration::from_secs(20);
    let status = loop {
        if let Some(status) = child.try_wait().expect("waiting for codediff") {
            break status;
        }
        assert!(Instant::now() < deadline, "the catalog never exited");
        std::thread::sleep(Duration::from_millis(25));
    };
    drop(pty.master);
    let output = written(collector, &output);

    assert!(status.success(), "{output:?}");
    assert!(output.contains(ENTER_ALT));
    assert!(output.contains(LEAVE_ALT));
}

#[test]
fn a_real_repo_mouse_click_opens_a_changed_file() {
    let repo = std::env::temp_dir().join(format!(
        "codediff-browser-mouse-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_nanos()
    ));
    fs::create_dir_all(&repo).expect("creating the test repository");
    let original = (1..=300)
        .map(|line| format!("line {line:03} {}", "x".repeat(80)))
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    fs::write(repo.join("sample.txt"), &original).expect("writing the original file");
    for args in [
        vec!["init", "-q"],
        vec!["config", "user.email", "codediff@example.com"],
        vec!["config", "user.name", "codediff e2e"],
        vec!["add", "sample.txt"],
        vec!["commit", "-qm", "initial"],
    ] {
        assert!(
            Command::new("git")
                .args(&args)
                .current_dir(&repo)
                .status()
                .expect("running git")
                .success(),
            "git {:?} failed",
            args
        );
    }
    let changed = (1..=320)
        .map(|line| {
            if line == 2 {
                format!("changed line {line:03} {}", "y".repeat(80))
            } else {
                format!("line {line:03} {}", "x".repeat(80))
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    fs::write(repo.join("sample.txt"), changed).expect("writing the changed file");

    let pty = native_pty_system()
        .openpty(PtySize {
            rows: 24,
            cols: 100,
            pixel_width: 0,
            pixel_height: 0,
        })
        .expect("opening a pty");
    let mut command = CommandBuilder::new(env!("CARGO_BIN_EXE_codediff"));
    command.cwd(&repo);
    command.env("TERM", "xterm-256color");
    let mut child = pty.slave.spawn_command(command).expect("spawning codediff");
    drop(pty.slave);
    let reader = pty.master.try_clone_reader().expect("reading the pty");
    let (collector, output) = collect(reader);
    drawn(&output);
    std::thread::sleep(Duration::from_millis(500));

    let mut writer = pty.master.take_writer().expect("writing to the pty");
    let before = output.lock().expect("output lock").bytes.len();
    // SGR mouse press/release at the first changed-file row in the Explorer.
    writer
        .write_all(b"\x1b[<0;10;5M\x1b[<0;10;5m")
        .expect("sending the Explorer click");
    writer.flush().expect("flushing the Explorer click");
    drawn_after(&output, before);
    let clicked = wait_for_output(&output, "changed");
    assert!(
        clicked.contains("line") && clicked.contains("changed"),
        "Explorer click did not open the changed file: {clicked:?}"
    );

    let before_navigation = output.lock().expect("output lock").bytes.len();
    writer
        .write_all(b"jjjjjjllllllll\x1b[<65;10;5M\x1b[<67;10;5M")
        .expect("sending diff navigation");
    writer.flush().expect("flushing diff navigation");
    pty.master
        .resize(PtySize {
            rows: 30,
            cols: 120,
            pixel_width: 0,
            pixel_height: 0,
        })
        .expect("resizing the real terminal");
    drawn_after(&output, before_navigation);

    writer.write_all(b"q").expect("sending quit");
    writer.flush().expect("flushing quit");
    drop(writer);
    let deadline = Instant::now() + Duration::from_secs(20);
    let status = loop {
        if let Some(status) = child.try_wait().expect("waiting for codediff") {
            break status;
        }
        assert!(
            Instant::now() < deadline,
            "codediff did not exit after the click"
        );
        std::thread::sleep(Duration::from_millis(25));
    };
    drop(pty.master);
    let output = written(collector, &output);
    let _ = fs::remove_dir_all(&repo);
    assert!(
        status.success(),
        "codediff exited unsuccessfully: {output:?}"
    );
    assert!(output.contains(ENTER_ALT));
    assert!(output.contains(LEAVE_ALT));
}

fn wait_for_output(output: &Arc<Mutex<Output>>, needle: &str) -> String {
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        let current = all_output(output);
        if current.contains(needle) {
            return current;
        }
        assert!(
            Instant::now() < deadline,
            "terminal output never contained {needle:?}"
        );
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn send_and_wait(writer: &mut dyn Write, output: &Arc<Mutex<Output>>, input: &[u8]) -> String {
    let before = output
        .lock()
        .expect("nothing else holds the lock")
        .bytes
        .len();
    writer.write_all(input).expect("sending input");
    writer.flush().expect("flushing input");
    drawn_after(output, before);
    let held = output.lock().expect("nothing else holds the lock");
    String::from_utf8_lossy(&held.bytes[before..]).into_owned()
}

fn all_output(output: &Arc<Mutex<Output>>) -> String {
    String::from_utf8_lossy(&output.lock().expect("nothing else holds the lock").bytes).into_owned()
}
