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

use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use portable_pty::{CommandBuilder, PtySize, native_pty_system};

/// Everything the child has written, and when it last wrote.
#[derive(Default)]
struct Output {
    bytes: Vec<u8>,
    last: Option<Instant>,
}

/// Reads the pty on a thread of its own, into something a test can watch.
fn collect(mut reader: Box<dyn Read + Send>) -> (std::thread::JoinHandle<()>, Arc<Mutex<Output>>) {
    let shared = Arc::new(Mutex::new(Output::default()));
    let filling = Arc::clone(&shared);
    let thread = std::thread::spawn(move || {
        let mut chunk = [0u8; 4096];
        while let Ok(read) = reader.read(&mut chunk) {
            if read == 0 {
                break;
            }
            let mut held = filling.lock().expect("nothing else holds the lock");
            held.bytes.extend_from_slice(&chunk[..read]);
            held.last = Some(Instant::now());
        }
    });
    (thread, shared)
}

/// Blocks until the child has drawn and then gone quiet.
///
/// A keystroke means nothing until the first frame is on screen, and what that
/// frame waits for — a process started, a repository listed, a file diffed —
/// takes a different length of time on every machine. Measured here at 840 ms
/// for a debug build, against the 400 ms a fixed sleep used to guess: the keys
/// landed before the diff they act on, and the test read a screen with only
/// the list on it. So wait for the writing to stop rather than guess.
fn drawn(output: &Arc<Mutex<Output>>) {
    const QUIET: Duration = Duration::from_millis(250);
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        {
            let held = output.lock().expect("nothing else holds the lock");
            if held.last.is_some_and(|last| last.elapsed() >= QUIET) {
                return;
            }
        }
        assert!(Instant::now() < deadline, "the child never drew anything");
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// What the child wrote, once it has stopped writing.
fn written(thread: std::thread::JoinHandle<()>, output: &Arc<Mutex<Output>>) -> String {
    thread.join().expect("collecting output");
    let held = output.lock().expect("the reader is gone");
    String::from_utf8_lossy(&held.bytes).into_owned()
}

/// `CSI ? 1049 h` and `l` — the alternate screen on and off.
const ENTER_ALT: &str = "\u{1b}[?1049h";
const LEAVE_ALT: &str = "\u{1b}[?1049l";

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

/// Runs the binary on a real terminal, sends `keys`, and returns everything it
/// wrote plus its exit status.
fn on_a_terminal(args: &[&str], cwd: Option<&PathBuf>, keys: &[u8]) -> (String, bool) {
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
    if let Some(cwd) = cwd {
        command.cwd(cwd);
    }
    // Otherwise the child inherits the test runner's idea of the terminal,
    // which may be `dumb` and would change what crossterm emits.
    command.env("TERM", "xterm-256color");

    let mut child = pty.slave.spawn_command(command).expect("spawning codediff");
    drop(pty.slave);

    let reader = pty.master.try_clone_reader().expect("reading the pty");
    let (collector, output) = collect(reader);

    if !keys.is_empty() {
        // The child has to have drawn its first frame before a keystroke means
        // anything; without this the key can arrive before raw mode is on and
        // be echoed instead of read.
        drawn(&output);
        let mut writer = pty.master.take_writer().expect("writing to the pty");
        writer.write_all(keys).expect("sending keys");
        writer.flush().expect("flushing");
        drop(writer);
    }

    let deadline = Instant::now() + Duration::from_secs(20);
    let status = loop {
        if let Some(status) = child.try_wait().expect("waiting for codediff") {
            break status;
        }
        assert!(Instant::now() < deadline, "codediff {args:?} never exited");
        std::thread::sleep(Duration::from_millis(25));
    };

    drop(pty.master);
    (written(collector, &output), status.success())
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
