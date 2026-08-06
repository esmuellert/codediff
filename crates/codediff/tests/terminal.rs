//! Giving the terminal back.
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
use std::time::{Duration, Instant};

use portable_pty::{CommandBuilder, PtySize, native_pty_system};

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

    let mut reader = pty.master.try_clone_reader().expect("reading the pty");
    let collector = std::thread::spawn(move || {
        let mut output = Vec::new();
        let mut chunk = [0u8; 4096];
        while let Ok(read) = reader.read(&mut chunk) {
            if read == 0 {
                break;
            }
            output.extend_from_slice(&chunk[..read]);
        }
        output
    });

    if !keys.is_empty() {
        // The child has to have drawn its first frame before a keystroke means
        // anything; without this the key can arrive before raw mode is on and
        // be echoed instead of read.
        std::thread::sleep(Duration::from_millis(400));
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
    let output = collector.join().expect("collecting output");
    (
        String::from_utf8_lossy(&output).into_owned(),
        status.success(),
    )
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
fn the_diff_is_actually_drawn_before_it_is_dismissed() {
    // Guards against the previous test passing on a program that opens the
    // alternate screen and immediately closes it.
    let fixture = Fixture::new("drawn");
    let (output, ok) = on_a_terminal(&["modified.txt"], Some(&fixture.dir), b"q");
    assert!(ok);
    assert!(
        output.contains("modified.txt"),
        "no status line:\n{output:?}"
    );
    assert!(output.contains('│'), "no divider between the two columns");
}

#[test]
fn a_one_sided_file_is_drawn_in_one_pane() {
    // An untracked file has no original to compare against, so there is no
    // second pane and nothing to separate. See D23.
    let fixture = Fixture::new("one-sided");
    let (output, ok) = on_a_terminal(&["untracked.txt"], Some(&fixture.dir), b"q");
    assert!(ok);
    assert!(output.contains("never added"), "no content:\n{output:?}");
    assert!(output.contains("(added)"), "not labelled:\n{output:?}");
    assert!(
        !output.contains('│'),
        "a second pane was drawn:\n{output:?}"
    );
    assert!(!output.contains('╱'), "fillers were drawn:\n{output:?}");
}

#[test]
fn a_change_key_with_nowhere_to_go_says_so_on_a_real_terminal() {
    // `]c` is the only binding made of punctuation, and an in-memory test
    // cannot show that a terminal delivers `]` as itself. Two presses on a
    // one-change file: the first lands on it, the second has nowhere to go.
    //
    // Asserted in fragments because only changed cells are redrawn, so the
    // phrase arrives split across a cursor move — `no` and then `next change`,
    // the space between them already being right. Matching the whole phrase
    // would be asserting on the redraw strategy rather than on the screen.
    let fixture = Fixture::new("exhausted");
    let (output, ok) = on_a_terminal(&["modified.txt"], Some(&fixture.dir), b"]c]cq");
    assert!(ok);
    assert!(
        output.contains("change 1/1"),
        "the first `]c` never landed:\n{output:?}"
    );
    assert!(
        output.contains("next change"),
        "the second `]c` said nothing:\n{output:?}"
    );
}

#[test]
fn the_layout_key_is_delivered_by_a_real_terminal() {
    // What only a pty can show: that `t` arrives as `t` and the program stays
    // healthy after rebuilding its buffer. What is on screen afterwards is
    // asserted against the exact grid in `screens.rs` instead — the capture
    // here is cumulative and ratatui redraws only the cells that changed, so
    // the first frame's two columns are still in the bytes, and the row total
    // going from `1/4` to `1/5` emits one digit rather than a phrase.
    let fixture = Fixture::new("inline");
    let (columns, ok) = on_a_terminal(&["modified.txt"], Some(&fixture.dir), b"q");
    assert!(ok);
    assert!(columns.contains("1/4"), "not four view lines:\n{columns:?}");

    let (toggled, ok) = on_a_terminal(&["modified.txt"], Some(&fixture.dir), b"tq");
    assert!(ok, "toggling then quitting failed");
    assert!(
        toggled.len() > columns.len(),
        "`t` redrew nothing, so it never arrived"
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

/// Runs the binary on a real terminal, suspends it, resumes it, then quits.
///
/// Unix only: `SIGTSTP` has no Windows equivalent, and there `Ctrl-Z` is
/// simply an unbound key.
#[cfg(unix)]
#[test]
fn suspending_gives_the_terminal_back_and_resuming_redraws() {
    use std::process::Command;

    let fixture = Fixture::new("suspend");
    let pty = native_pty_system()
        .openpty(PtySize {
            rows: 24,
            cols: 80,
            pixel_width: 0,
            pixel_height: 0,
        })
        .expect("opening a pty");

    let mut command = CommandBuilder::new(env!("CARGO_BIN_EXE_codediff"));
    command.args(["untracked.txt"]);
    command.cwd(&fixture.dir);
    command.env("TERM", "xterm-256color");

    let mut child = pty.slave.spawn_command(command).expect("spawning codediff");
    drop(pty.slave);
    let pid = child.process_id().expect("a process id");

    let mut reader = pty.master.try_clone_reader().expect("reading the pty");
    let collector = std::thread::spawn(move || {
        let mut output = Vec::new();
        let mut chunk = [0u8; 4096];
        while let Ok(read) = reader.read(&mut chunk) {
            if read == 0 {
                break;
            }
            output.extend_from_slice(&chunk[..read]);
        }
        output
    });

    let mut writer = pty.master.take_writer().expect("writing to the pty");
    std::thread::sleep(Duration::from_millis(400));
    writer.write_all(&[0x1a]).expect("Ctrl-Z"); // ^Z
    writer.flush().unwrap();

    // Stopped, not exited, and the terminal handed back.
    std::thread::sleep(Duration::from_millis(500));
    assert!(
        child.try_wait().expect("checking").is_none(),
        "Ctrl-Z should stop the process, not end it"
    );

    let resumed = Command::new("kill")
        .args(["-CONT", &pid.to_string()])
        .status()
        .expect("running kill");
    assert!(resumed.success(), "could not resume the process");

    std::thread::sleep(Duration::from_millis(400));
    writer.write_all(b"q").expect("quit");
    writer.flush().unwrap();
    drop(writer);

    let deadline = Instant::now() + Duration::from_secs(20);
    let status = loop {
        if let Some(status) = child.try_wait().expect("waiting") {
            break status;
        }
        assert!(Instant::now() < deadline, "never exited after resuming");
        std::thread::sleep(Duration::from_millis(25));
    };
    drop(pty.master);

    let output = String::from_utf8_lossy(&collector.join().expect("output")).into_owned();
    assert!(status.success(), "{output}");
    assert_eq!(
        output.matches(ENTER_ALT).count(),
        2,
        "entered once, then again on resuming:\n{output:?}"
    );
    assert_eq!(output.matches(LEAVE_ALT).count(), 2, "left both times");
    assert!(
        output.matches("untracked.txt").count() >= 2,
        "resuming must repaint the whole screen, not send a difference \
         against a frame the terminal no longer holds"
    );
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

    let mut reader = pty.master.try_clone_reader().expect("reading the pty");
    let collector = std::thread::spawn(move || {
        let mut output = Vec::new();
        let mut chunk = [0u8; 4096];
        while let Ok(read) = reader.read(&mut chunk) {
            if read == 0 {
                break;
            }
            output.extend_from_slice(&chunk[..read]);
        }
        output
    });

    // The first frame has to be on screen, or there is nothing to restore.
    std::thread::sleep(Duration::from_millis(600));
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
    let output = collector.join().expect("collecting output");
    (String::from_utf8_lossy(&output).into_owned(), status)
}
