use std::io::Write;
use std::time::{Duration, Instant};

use portable_pty::{CommandBuilder, PtySize, native_pty_system};

use super::super::pty::{ENTER_ALT, LEAVE_ALT, collect, wait_for_screen, written};
use super::super::screen::Screen;

const SIZE: (usize, usize) = (100, 24);

fn send(writer: &mut dyn Write, keys: &[u8]) {
    writer.write_all(keys).expect("sending input");
    writer.flush().expect("flushing input");
}

fn shows_all(texts: &'static [&'static str]) -> impl Fn(&Screen) -> bool {
    move |screen| texts.iter().all(|text| screen.contains(text))
}

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
    wait_for_screen(&output, SIZE, "the catalog", shows_all(&["STORIES"]));

    let mut writer = pty.master.take_writer().expect("writing to the pty");
    send(&mut writer, b"/");
    wait_for_screen(&output, SIZE, "the filter", shows_all(&["FILTER"]));
    send(&mut writer, b"edge-matrix");
    // The filter has applied once a story it excludes is gone.
    wait_for_screen(&output, SIZE, "the filtered list", |screen| {
        screen.contains("edge-matrix") && !screen.contains("explorer/empty")
    });

    send(&mut writer, b"\r");
    wait_for_screen(
        &output,
        SIZE,
        "the opened story",
        shows_all(&["fn edge_matrix()"]),
    );

    send(&mut writer, b"]");
    wait_for_screen(
        &output,
        SIZE,
        "the next story",
        shows_all(&["inline/unchanged", "same in one column"]),
    );
    send(&mut writer, b"[");
    wait_for_screen(
        &output,
        SIZE,
        "the previous story",
        shows_all(&["fn edge_matrix()"]),
    );
    send(&mut writer, b"r\x1b");
    wait_for_screen(
        &output,
        SIZE,
        "the catalog again",
        shows_all(&["Welcome", "explorer/empty"]),
    );

    // Queued keys all go to the frame drawn before them, so the filter has
    // to be on screen before its text is typed.
    send(&mut writer, b"/");
    wait_for_screen(&output, SIZE, "the filter", shows_all(&["FILTER"]));
    send(&mut writer, b"explorer/list");
    wait_for_screen(&output, SIZE, "the filtered list", |screen| {
        screen.contains("explorer/list") && !screen.contains("explorer/empty")
    });
    send(&mut writer, b"\r");
    wait_for_screen(
        &output,
        SIZE,
        "the Explorer story's setup keys applied",
        shows_all(&["src/app.rs"]),
    );
    send(&mut writer, b"\x1b");
    // A lone Esc must be read before `q`, or the pair parses as Alt+q.
    wait_for_screen(
        &output,
        SIZE,
        "the catalog after the Explorer story",
        |screen| !screen.contains("src/app.rs"),
    );

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
