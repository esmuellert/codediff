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
