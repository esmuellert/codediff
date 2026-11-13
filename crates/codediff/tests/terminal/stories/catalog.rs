use std::io::Write;
use std::time::{Duration, Instant};

use portable_pty::{CommandBuilder, PtySize, native_pty_system};

use super::super::common::{send_and_expect, wait_for_screen};
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
    let size = (24, 100);
    wait_for_screen(&output, size, &["STORIES"]);

    let mut writer = pty.master.take_writer().expect("writing to the pty");
    send_and_expect(&mut writer, &output, size, b"/", &["FILTER"]);
    // The typed filter, not a list line that already matched it.
    send_and_expect(
        &mut writer,
        &output,
        size,
        b"edge-matrix",
        &["FILTER  edge-matrix"],
    );
    send_and_expect(&mut writer, &output, size, b"\r", &["fn edge_matrix()"]);
    send_and_expect(
        &mut writer,
        &output,
        size,
        b"]",
        &["inline/unchanged", "same in one column"],
    );
    send_and_expect(&mut writer, &output, size, b"[", &["edge_matrix"]);
    send_and_expect(&mut writer, &output, size, b"r", &["edge_matrix"]);
    send_and_expect(
        &mut writer,
        &output,
        size,
        b"\x1b",
        &["STORIES", "side-by-side/edge-matrix"],
    );

    send_and_expect(&mut writer, &output, size, b"/", &["FILTER"]);
    send_and_expect(
        &mut writer,
        &output,
        size,
        b"explorer/list",
        &["FILTER  explorer/list"],
    );
    // The list story presses its own keys once the files are listed.
    send_and_expect(&mut writer, &output, size, b"\r", &["src/app.rs"]);
    send_and_expect(&mut writer, &output, size, b"\x1b", &["STORIES"]);

    send_and_expect(&mut writer, &output, size, b"/", &["FILTER"]);
    send_and_expect(
        &mut writer,
        &output,
        size,
        b"explorer/cards",
        &["FILTER  explorer/cards"],
    );
    let cards = send_and_expect(&mut writer, &output, size, b"\r", &["src/panel.rs"]);
    assert!(
        !cards.contains("No two-sided diffs in this directory"),
        "directory card story rendered the empty state: {cards}"
    );
    // Selecting a file replaces the cards with that file's diff.
    send_and_expect(
        &mut writer,
        &output,
        size,
        b"\x1b[<0;10;6M\x1b[<0;10;6m",
        &["new_name"],
    );
    // Reselecting the directory brings its cards back.
    send_and_expect(
        &mut writer,
        &output,
        size,
        b"\x1b[<0;10;5M\x1b[<0;10;5m",
        &["src/panel.rs"],
    );
    send_and_expect(&mut writer, &output, size, b"\x1b", &["STORIES"]);

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
