//! Browsing and switching stories without restarting the binary.
#![cfg(unix)]

mod support;

use std::io::Write;
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
        next.contains("newly_added"),
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
