//! Verifies live OSC 11 + DA1 terminal probing across a real Unix PTY.
#![cfg(unix)]

use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use portable_pty::{CommandBuilder, PtySize, native_pty_system};

use super::common::wait_for_screen;
use super::pty::{ENTER_ALT, LEAVE_ALT, Output};

const ROWS: u16 = 24;
const COLS: u16 = 100;

struct Fixture {
    dir: PathBuf,
}

impl Fixture {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "codediff-theme-probe-{name}-{}",
            std::process::id()
        ));
        fixtures::repo(&dir).expect("building the fixture repository");
        Self { dir }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn run_with_terminal_reply(
    args: &[&str],
    cwd: Option<&PathBuf>,
    reply: &'static [u8],
    quit: bool,
) -> String {
    let pty = native_pty_system()
        .openpty(PtySize {
            rows: ROWS,
            cols: COLS,
            pixel_width: 0,
            pixel_height: 0,
        })
        .expect("opening a pty");

    let mut command = CommandBuilder::new(env!("CARGO_BIN_EXE_codediff"));
    command.args(args);
    if let Some(cwd) = cwd {
        command.cwd(cwd);
    }
    // Simulate a bare SSH session where COLORTERM, COLORFGBG, and TERM_PROGRAM are absent.
    command.env("TERM", "xterm-256color");
    command.env_remove("COLORTERM");
    command.env_remove("COLORFGBG");
    command.env_remove("TERM_PROGRAM");
    command.env_remove("WT_SESSION");
    command.env_remove("KONSOLE_VERSION");
    command.env_remove("VTE_VERSION");
    command.env_remove("NO_COLOR");
    // Isolate from any user config file on the host.
    if let Some(cwd) = cwd {
        command.env("XDG_CONFIG_HOME", cwd.join(".empty-xdg"));
        command.env("HOME", cwd.join(".empty-home"));
    }

    let mut child = pty.slave.spawn_command(command).expect("spawning codediff");
    drop(pty.slave);

    let mut reader = pty.master.try_clone_reader().expect("reading the pty");
    let mut writer = pty.master.take_writer().expect("writing to the pty");

    let output = Arc::new(Mutex::new(Output::default()));
    let filling = Arc::clone(&output);

    let (answered_tx, answered_rx) = std::sync::mpsc::channel();
    let collector = std::thread::spawn(move || {
        let mut chunk = [0u8; 4096];
        let mut answered = false;
        while let Ok(read) = reader.read(&mut chunk) {
            if read == 0 {
                break;
            }
            let just_saw_da1 = {
                let mut held = filling.lock().expect("output lock");
                held.bytes.extend_from_slice(&chunk[..read]);
                !answered && held.bytes.windows(3).any(|w| w == b"\x1b[c")
            };
            if just_saw_da1 {
                answered = true;
                let _ = answered_tx.send(());
            }
        }
    });

    // Reply as soon as codediff sends the OSC 11 + DA1 query batch.
    answered_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("codediff never queried the terminal with DA1");
    writer
        .write_all(reply)
        .expect("sending terminal probe reply");
    writer.flush().expect("flushing terminal probe reply");

    if quit {
        wait_for_screen(&output, (ROWS, COLS), &["Changes"]);
        writer.write_all(b"q").expect("sending quit key");
        writer.flush().expect("flushing quit key");
    }
    drop(writer);

    let deadline = Instant::now() + Duration::from_secs(20);
    let status = loop {
        if let Some(status) = child.try_wait().expect("waiting for codediff") {
            break status;
        }
        assert!(Instant::now() < deadline, "codediff {args:?} never exited");
        std::thread::sleep(Duration::from_millis(20));
    };

    drop(pty.master);
    collector.join().expect("collecting pty output");
    let rendered = String::from_utf8_lossy(&output.lock().expect("output lock").bytes).into_owned();
    assert!(
        status.success(),
        "codediff exited unsuccessfully:\n{rendered}"
    );
    rendered
}

#[test]
fn light_osc11_reply_selects_24bit_catppuccin_latte_without_env_vars() {
    let fixture = Fixture::new("light");
    let output = run_with_terminal_reply(
        &[],
        Some(&fixture.dir),
        b"\x1b]11;rgb:ffff/ffff/ffff\x07\x1b[?62;22c",
        true,
    );

    assert!(output.contains(ENTER_ALT));
    assert!(output.contains(LEAVE_ALT));
    // Catppuccin Latte base background (239, 241, 245) in 24-bit SGR.
    assert!(
        output.contains("48;2;239;241;245"),
        "expected Catppuccin Latte 24-bit base background in output"
    );
    assert!(
        !output.contains("48;2;30;30;46"),
        "unexpected Catppuccin Mocha background on a light terminal"
    );
}

#[test]
fn dark_osc11_reply_selects_24bit_catppuccin_mocha_without_env_vars() {
    let fixture = Fixture::new("dark");
    let output = run_with_terminal_reply(
        &[],
        Some(&fixture.dir),
        b"\x1b]11;rgb:1e1e/1e1e/2e2e\x1b\\\x1b[?62;22c",
        true,
    );

    assert!(output.contains(ENTER_ALT));
    assert!(output.contains(LEAVE_ALT));
    // Catppuccin Mocha base background (30, 30, 46) in 24-bit SGR.
    assert!(
        output.contains("48;2;30;30;46"),
        "expected Catppuccin Mocha 24-bit base background in output"
    );
    assert!(
        !output.contains("48;2;239;241;245"),
        "unexpected Catppuccin Latte background on a dark terminal"
    );
}

#[test]
fn doctor_reports_probed_background_and_theme_without_entering_alt_screen() {
    let fixture = Fixture::new("doctor");
    let output = run_with_terminal_reply(
        &["doctor"],
        Some(&fixture.dir),
        b"\x1b]11;rgb:eff1/f5f5/ffff\x07\x1bP1+r524742=382f382f38\x1b\\\x1b[?62;22c",
        false,
    );

    assert!(
        !output.contains(ENTER_ALT),
        "doctor entered the alternate screen"
    );
    assert!(
        output.contains("rgb(239, 245, 255) (light)"),
        "doctor did not report probed light background:\n{output}"
    );
    assert!(
        output.contains("catppuccin-latte (default)"),
        "doctor did not report catppuccin-latte:\n{output}"
    );
    assert!(
        output.contains("graph symbols"),
        "doctor did not report graph symbols:\n{output}"
    );
}
