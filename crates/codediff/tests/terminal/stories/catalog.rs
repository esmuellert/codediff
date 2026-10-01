use std::io::Write;
use std::time::{Duration, Instant};

use portable_pty::{CommandBuilder, PtySize, native_pty_system};

use super::super::common::{send_and_wait, send_and_wait_for_screen, wait_for_screen};
use super::super::pty::{ENTER_ALT, LEAVE_ALT, collect, written};

#[test]
fn story_catalog_filters_opens_switches_resets_and_returns() {
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
    wait_for_screen(&output, |screen| screen.contains("STORIES"));

    let mut writer = pty.master.take_writer().expect("writing to the pty");
    send_and_wait_for_screen(&mut writer, &output, b"/", |screen| {
        screen.contains("FILTER")
    });
    send_and_wait_for_screen(&mut writer, &output, b"edge-matrix", |screen| {
        screen.contains("FILTER") && screen.contains("edge-matrix")
    });

    send_and_wait_for_screen(&mut writer, &output, b"\r", |screen| {
        screen.contains("fn edge_matrix()")
    });
    send_and_wait_for_screen(&mut writer, &output, b"]", |screen| {
        screen.contains("inline/unchanged") && screen.contains("same in one column")
    });
    send_and_wait_for_screen(&mut writer, &output, b"[", |screen| {
        screen.contains("edge_matrix")
    });
    let _ = send_and_wait(&mut writer, &output, b"r");
    send_and_wait_for_screen(&mut writer, &output, b"\x1b", |screen| {
        screen.contains("Welcome") && screen.contains("explorer/empty")
    });

    send_and_wait_for_screen(&mut writer, &output, b"/", |screen| {
        screen.contains("FILTER")
    });
    send_and_wait_for_screen(&mut writer, &output, b"explorer/list", |screen| {
        screen.contains("FILTER") && screen.contains("explorer/list")
    });
    send_and_wait_for_screen(&mut writer, &output, b"\r", |screen| {
        screen.contains("src/app.rs")
    });
    send_and_wait_for_screen(&mut writer, &output, b"\x1b", |screen| {
        screen.contains("STORIES")
    });

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
