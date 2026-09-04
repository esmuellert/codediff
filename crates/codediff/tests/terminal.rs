//! Giving the terminal back.
#![cfg(unix)]
//!
//! The one failure mode a read-only reviewer must never have: quitting, or
//! crashing, and leaving a shell with no echo, an invisible cursor and the
//! diff still on screen. Recovering from that needs `reset`, typed blind.
//!
//! Checked from outside the process, through a real pty, because the escape
//! sequences involved are only produced when stdout is a terminal and only
//! observable by whatever is on the other end of it.

mod support;

use std::path::PathBuf;
use std::time::{Duration, Instant};

use portable_pty::{CommandBuilder, PtySize, native_pty_system};
use support::{ENTER_ALT, LEAVE_ALT, collect, drawn, on_a_terminal, written};

struct Fixture {
    dir: PathBuf,
}

impl Fixture {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("codediff-s7-{name}-{}", std::process::id()));
        fixtures::repo(&dir).expect("building the fixture repository");
        Self { dir }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

#[test]
fn quitting_gives_the_terminal_back() {
    let fixture = Fixture::new("quit");
    let (output, ok) = on_a_terminal(&["untracked.txt"], Some(&fixture.dir), b"q");

    assert!(ok, "codediff exited unsuccessfully:\n{output}");
    assert!(
        output.contains(ENTER_ALT),
        "never entered the alternate screen"
    );
    assert!(
        output.contains(LEAVE_ALT),
        "never left the alternate screen"
    );
    assert!(
        output.rfind(LEAVE_ALT) > output.find(ENTER_ALT),
        "left before it entered"
    );
    assert_eq!(
        output.matches(ENTER_ALT).count(),
        output.matches(LEAVE_ALT).count(),
        "the alternate screen was not balanced"
    );
}

#[test]
fn a_panic_still_gives_the_terminal_back() {
    let (output, ok) = on_a_terminal(&["--self-panic"], None, &[]);

    assert!(!ok, "--self-panic is supposed to fail");
    assert!(output.contains(ENTER_ALT));

    // The order is the whole point: the panic message has to land on the
    // shell's screen, not on the one that is about to be thrown away. It is
    // restored twice — once by the hook and once when `Screen` drops — which
    // is harmless, so the *first* restore is what matters.
    let restored = output
        .find(LEAVE_ALT)
        .expect("never left the alternate screen");
    let message = output.find("deliberate panic").expect("no panic message");
    assert!(restored < message, "{output:?}");
}

#[test]
fn the_cursor_is_hidden_while_reviewing_and_shown_afterwards() {
    let fixture = Fixture::new("cursor");
    let (output, ok) = on_a_terminal(&["untracked.txt"], Some(&fixture.dir), b"q");
    assert!(ok);
    assert!(output.contains("\u{1b}[?25l"), "cursor never hidden");
    assert!(output.contains("\u{1b}[?25h"), "cursor never shown again");
}

#[test]
fn a_command_that_prints_text_never_touches_the_alternate_screen() {
    // `debug` and `doctor` are pipeable. Taking the terminal for them would
    // make `codediff doctor | less` produce nothing.
    let (output, ok) = on_a_terminal(&["doctor"], None, &[]);
    assert!(ok, "{output}");
    assert!(!output.contains(ENTER_ALT), "{output:?}");
}

#[test]
#[cfg(unix)]
fn a_signal_still_gives_the_terminal_back() {
    // `Drop` does not run for a signal, so `kill` used to leave the reader in
    // the alternate screen with the cursor hidden and raw mode on — a shell
    // they had to type `reset` into blind.
    //
    // The first attempt at this was worse than the bug: it registered a
    // handler that set a flag, and nothing ever read the flag, because
    // crossterm retries its wait on `EINTR` rather than reporting it. The
    // program became unkillable. Hence a wait with a timeout, and hence this
    // test asserting the process actually goes.
    // 15 and 1. Written as numbers because `kill` takes numbers and this
    // crate has no libc dependency to name them from.
    for signal in [15, 1] {
        let fixture = Fixture::new(&format!("signal{signal}"));
        let (output, status) = killed_by(&["modified.txt"], &fixture.dir, signal);

        assert!(status.is_some(), "signal {signal} did not stop it");
        assert!(
            output.contains(ENTER_ALT),
            "signal {signal}: it never took the screen"
        );
        assert!(
            output.contains(LEAVE_ALT),
            "signal {signal}: the terminal was not given back"
        );
        assert!(
            output.contains("\u{1b}[?25h"),
            "signal {signal}: the cursor was left hidden"
        );
    }

    let fixture = Fixture::new("story-signal");
    let (output, status) = killed_by(
        &["debug", "ui", "side-by-side/replacement"],
        &fixture.dir,
        15,
    );
    assert!(status.is_some(), "the story ignored SIGTERM");
    assert!(output.contains(ENTER_ALT));
    assert!(output.contains(LEAVE_ALT));
    assert!(output.contains("\u{1b}[?25h"));
}

/// Runs `codediff` on a pty, sends it a signal, and returns what it wrote.
///
/// `None` for the status means it was still running when the wait ran out,
/// which is the failure this exists to catch.
#[cfg(unix)]
fn killed_by(args: &[&str], cwd: &PathBuf, signal: i32) -> (String, Option<i32>) {
    let pty = native_pty_system()
        .openpty(PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        })
        .expect("opening a pty");

    let mut command = CommandBuilder::new(env!("CARGO_BIN_EXE_codediff"));
    command.args(args);
    command.cwd(cwd);
    command.env("TERM", "xterm-256color");

    let mut child = pty.slave.spawn_command(command).expect("spawning codediff");
    drop(pty.slave);

    let reader = pty.master.try_clone_reader().expect("reading the pty");
    let (collector, output) = collect(reader);

    // The first frame has to be on screen, or there is nothing to restore.
    drawn(&output);
    let pid = child.process_id().expect("a process id") as i32;
    std::process::Command::new("kill")
        .args([format!("-{signal}"), pid.to_string()])
        .status()
        .expect("sending the signal");

    let deadline = Instant::now() + Duration::from_secs(10);
    let status = loop {
        if let Some(status) = child.try_wait().expect("waiting for codediff") {
            break Some(status.exit_code() as i32);
        }
        if Instant::now() > deadline {
            let _ = child.kill();
            break None;
        }
        std::thread::sleep(Duration::from_millis(50));
    };

    drop(pty.master);
    (written(collector, &output), status)
}
