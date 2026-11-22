//! Changes on disk that leave every file's text the same must not change the
//! screen.
//!
//! `touch`, rewriting the same bytes, or replacing a file with an identical
//! copy all wake the watcher. The watcher cannot tell that nothing changed
//! without reading the files, so codediff lists them again. That listing must
//! not cause another refresh, and the screen must stay as it was.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime};

use portable_pty::{Child, CommandBuilder, MasterPty, PtySize, native_pty_system};

use super::common::{screen_text, send_and_expect, wait_for_screen};
use super::pty::{Output, collect};

const SIZE: (u16, u16) = (30, 140);

/// How long the screen is watched for changes after a refresh.
const WATCH_FOR: Duration = Duration::from_millis(1500);

/// codediff running in a terminal, in a repository of its own.
struct Codediff {
    dir: PathBuf,
    repo: PathBuf,
    log: PathBuf,
    child: Box<dyn Child + Send + Sync>,
    _master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    output: Arc<Mutex<Output>>,
}

impl Codediff {
    /// Starts codediff where `src/a.rs` and `src/b.rs` have changed since the
    /// last commit and `src/c.rs` has not. Each file has 60 lines.
    fn start(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "codediff-refresh-{name}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let repo = dir.join("repo");
        fs::create_dir_all(repo.join("src")).unwrap();
        for name in ["a", "b", "c"] {
            fs::write(repo.join(format!("src/{name}.rs")), lines(name, "line")).unwrap();
        }
        for args in [
            &["init", "-q"][..],
            &["config", "user.email", "codediff@example.com"],
            &["config", "user.name", "codediff e2e"],
            &["add", "-A"],
            &["commit", "-qm", "initial"],
        ] {
            let status = Command::new("git")
                .args(args)
                .current_dir(&repo)
                .status()
                .unwrap();
            assert!(status.success(), "git {args:?} failed");
        }
        fs::write(repo.join("src/a.rs"), lines("a", "CHANGED")).unwrap();
        fs::write(repo.join("src/b.rs"), lines("b", "CHANGED")).unwrap();

        let log = dir.join("codediff.log");
        let pty = native_pty_system()
            .openpty(PtySize {
                rows: SIZE.0,
                cols: SIZE.1,
                pixel_width: 0,
                pixel_height: 0,
            })
            .unwrap();
        let mut command = CommandBuilder::new(env!("CARGO_BIN_EXE_codediff"));
        command.arg("--log");
        command.arg(&log);
        command.cwd(&repo);
        command.env("TERM", "xterm-256color");
        let child = pty.slave.spawn_command(command).unwrap();
        drop(pty.slave);
        let (_reader, output) = collect(pty.master.try_clone_reader().unwrap());
        let writer = pty.master.take_writer().unwrap();
        wait_for_screen(&output, SIZE, &["a.rs", "b.rs"]);
        Self {
            dir,
            repo,
            log,
            child,
            _master: pty.master,
            writer,
            output,
        }
    }

    /// Sends `keys`, then waits until the screen shows all of `expected`.
    fn press(&mut self, keys: &[u8], expected: &[&str]) {
        send_and_expect(&mut self.writer, &self.output, SIZE, keys, expected);
    }

    fn screen(&self) -> String {
        screen_text(&self.output, SIZE.0, SIZE.1)
    }

    /// Everything drawn after the first `start` bytes.
    fn drawn_since(&self, start: usize) -> String {
        String::from_utf8_lossy(&self.output.lock().unwrap().bytes[start..]).into_owned()
    }

    fn drawn_so_far(&self) -> usize {
        self.output.lock().unwrap().bytes.len()
    }

    /// How many lines of codediff's log contain `message`.
    fn count_in_log(&self, message: &str) -> usize {
        fs::read_to_string(&self.log)
            .unwrap_or_default()
            .matches(message)
            .count()
    }

    /// Selects `src/a.rs`, and waits until its diff is shown on its own.
    fn select_src_a_rs(&mut self) {
        // The first key selects src, whose cards show a.rs under a header.
        self.press(b"j", &["M src/a.rs"]);
        self.press(b"j", &[]);
        wait_until("src/a.rs to open", || {
            let screen = self.screen();
            screen.contains("a CHANGED 1")
                && !screen.contains("M src/a.rs")
                && !screen.contains("src/b.rs")
                && !screen.contains("Loading file")
        });
    }
}

impl Drop for Codediff {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = fs::remove_dir_all(&self.dir);
    }
}

/// 60 lines such as `a line 1` … `a line 60`.
fn lines(file: &str, word: &str) -> String {
    (1..=60)
        .map(|line| format!("{file} {word} {line}\n"))
        .collect()
}

/// Waits up to 20 seconds for `condition` to hold.
fn wait_until(what: &str, mut condition: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(20);
    while !condition() {
        assert!(Instant::now() < deadline, "gave up waiting for {what}");
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// Changes a file's modification time and nothing else.
///
/// The time is set a minute back. Git does not record the time of a file
/// changed in the same second as the index, so a time of "now" would hide
/// whether a command rewrites the index.
fn change_modified_time(path: &Path) {
    fs::File::options()
        .write(true)
        .open(path)
        .unwrap()
        .set_modified(SystemTime::now() - Duration::from_secs(60))
        .unwrap();
}

fn index_modified_time(repo: &Path) -> SystemTime {
    fs::metadata(repo.join(".git/index"))
        .unwrap()
        .modified()
        .unwrap()
}

#[test]
fn listing_the_files_after_a_change_does_not_rewrite_the_index() {
    let codediff = Codediff::start("index");
    // Changes made while setting up the repository can still arrive; wait
    // until no refresh has come for a while.
    loop {
        let refreshes = codediff.count_in_log("refresh triggered");
        std::thread::sleep(WATCH_FOR);
        if codediff.count_in_log("refresh triggered") == refreshes {
            break;
        }
    }
    let listings = codediff.count_in_log("listed files");
    let refreshes = codediff.count_in_log("refresh triggered");
    let index_time = index_modified_time(&codediff.repo);

    change_modified_time(&codediff.repo.join("src/c.rs"));
    wait_until("the files to be listed again", || {
        codediff.count_in_log("listed files") > listings
    });

    // A rewritten index would wake the watcher and refresh everything again.
    assert_eq!(
        index_modified_time(&codediff.repo),
        index_time,
        "listing the files rewrote .git/index"
    );
    std::thread::sleep(WATCH_FOR);
    assert_eq!(
        codediff.count_in_log("refresh triggered"),
        refreshes + 1,
        "one change on disk caused more than one refresh"
    );
}

#[test]
fn a_change_that_leaves_the_text_the_same_keeps_the_directory_cards_in_place() {
    let mut codediff = Codediff::start("directory");
    // Select src, then scroll its cards down by five wheel steps of three
    // lines, which puts line 14 at the top.
    codediff.press(b"j", &["M src/a.rs"]);
    for _ in 0..5 {
        codediff.press(b"\x1b[<65;90;15M", &[]);
    }
    wait_until("the cards to scroll", || {
        codediff
            .screen()
            .lines()
            .nth(2)
            .is_some_and(|line| line.contains(" 14 a line 14 "))
    });
    let screen_before = codediff.screen();
    let listings = codediff.count_in_log("listed files");
    let reads = codediff.count_in_log("reading file");

    change_modified_time(&codediff.repo.join("src/a.rs"));
    wait_until("the files to be listed again", || {
        codediff.count_in_log("listed files") > listings
    });

    let deadline = Instant::now() + WATCH_FOR;
    while Instant::now() < deadline {
        assert_eq!(
            codediff.screen(),
            screen_before,
            "the cards moved after a refresh"
        );
        std::thread::sleep(Duration::from_millis(25));
    }
    assert_eq!(
        codediff.count_in_log("reading file"),
        reads,
        "unchanged files were read again"
    );
}

#[test]
fn a_change_that_leaves_the_text_the_same_never_shows_the_open_file_loading() {
    let mut codediff = Codediff::start("file");
    codediff.select_src_a_rs();
    let reads = codediff.count_in_log("reading file");
    let start = codediff.drawn_so_far();

    change_modified_time(&codediff.repo.join("src/a.rs"));
    // Both sides of the file are read again.
    wait_until("src/a.rs to be read again", || {
        codediff.count_in_log("reading file") >= reads + 2
    });
    std::thread::sleep(WATCH_FOR);

    assert!(
        !codediff.drawn_since(start).contains("Loading file"),
        "the open file was replaced by a loading message"
    );
}

#[test]
fn a_change_to_the_open_file_shows_the_new_text_without_a_loading_message() {
    let mut codediff = Codediff::start("change");
    codediff.select_src_a_rs();
    let listings = codediff.count_in_log("listed files");
    let start = codediff.drawn_so_far();

    // Lines 31 to 60 go back to the commit, so the line counts change and the
    // file is listed with new counts.
    let text: String = (1..=60)
        .map(|line| match line {
            1..=30 => format!("a EDITED {line}\n"),
            _ => format!("a line {line}\n"),
        })
        .collect();
    fs::write(codediff.repo.join("src/a.rs"), text).unwrap();
    wait_for_screen(&codediff.output, SIZE, &["a EDITED 1"]);
    // The new listing reaches the Explorer after the file itself is read.
    wait_until("the files to be listed again", || {
        codediff.count_in_log("listed files") > listings
    });
    std::thread::sleep(WATCH_FOR);

    assert!(
        !codediff.drawn_since(start).contains("Loading file"),
        "the open file was replaced by a loading message"
    );
}
