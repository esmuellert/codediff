use std::fs;
use std::io::Write;
use std::process::Command;
use std::time::{Duration, Instant};

use portable_pty::{CommandBuilder, PtySize, native_pty_system};

use super::pty::{ENTER_ALT, LEAVE_ALT, collect, drawn_after, wait_for_screen, written};

#[test]
fn clicking_a_changed_file_opens_its_diff() {
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
    let size = (100, 24);
    wait_for_screen(&output, size, "the Explorer listing sample.txt", |screen| {
        screen.contains("sample.txt")
    });

    let mut writer = pty.master.take_writer().expect("writing to the pty");
    // SGR mouse press/release at the first changed-file line in the Explorer.
    writer
        .write_all(b"\x1b[<0;10;5M\x1b[<0;10;5m")
        .expect("sending the Explorer click");
    writer.flush().expect("flushing the Explorer click");
    wait_for_screen(&output, size, "the clicked file's diff", |screen| {
        screen.contains("changed line 002")
    });

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
