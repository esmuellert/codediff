use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use portable_pty::{CommandBuilder, PtySize, native_pty_system};

/// Everything the child has written, and when it last wrote.
#[derive(Default)]
pub struct Output {
    pub bytes: Vec<u8>,
    last: Option<Instant>,
}

/// Reads the pty on a thread of its own, into something a test can watch.
pub fn collect(
    mut reader: Box<dyn Read + Send>,
) -> (std::thread::JoinHandle<()>, Arc<Mutex<Output>>) {
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
pub fn drawn(output: &Arc<Mutex<Output>>) {
    drawn_after(output, 0);
}

/// Blocks until output grew past `before` and the new frame went quiet.
pub fn drawn_after(output: &Arc<Mutex<Output>>, before: usize) {
    const QUIET: Duration = Duration::from_millis(250);
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        {
            let held = output.lock().expect("nothing else holds the lock");
            if held.bytes.len() > before && held.last.is_some_and(|last| last.elapsed() >= QUIET) {
                return;
            }
        }
        assert!(Instant::now() < deadline, "the child never drew anything");
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// What the child wrote, once it has stopped writing.
pub fn written(thread: std::thread::JoinHandle<()>, output: &Arc<Mutex<Output>>) -> String {
    thread.join().expect("collecting output");
    let held = output.lock().expect("the reader is gone");
    String::from_utf8_lossy(&held.bytes).into_owned()
}

/// `CSI ? 1049 h` and `l` — the alternate screen on and off.
pub const ENTER_ALT: &str = "\u{1b}[?1049h";
pub const LEAVE_ALT: &str = "\u{1b}[?1049l";

/// Runs the binary on a real terminal, sends `keys`, and returns everything it
/// wrote plus its exit status.
#[allow(dead_code)]
pub fn on_a_terminal(args: &[&str], cwd: Option<&PathBuf>, keys: &[u8]) -> (String, bool) {
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
