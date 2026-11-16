//! A change on disk that alters no file must not disturb what is on screen.
//!
//! `touch`, rewriting the same bytes, or replacing a file with an identical
//! copy all wake the watcher. The watcher cannot know nothing changed without
//! reading the files, so codediff lists them again. That listing must not
//! cause a second refresh by itself, and the screen must stay as it was.

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

/// How long the screen must stay unchanged once the refresh has been handled.
const SETTLED: Duration = Duration::from_millis(1500);

struct App {
    dir: PathBuf,
    repo: PathBuf,
    log: PathBuf,
    child: Box<dyn Child + Send + Sync>,
    _master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    output: Arc<Mutex<Output>>,
}

impl App {
    /// A repository with two changed files and one unchanged one, open in codediff.
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
        let lines = |marker: &str| {
            (1..=60)
                .map(|line| format!("{marker} {line}\n"))
                .collect::<String>()
        };
        fs::write(repo.join("src/a.rs"), lines("a line")).unwrap();
        fs::write(repo.join("src/b.rs"), lines("b line")).unwrap();
        fs::write(repo.join("src/c.rs"), lines("c line")).unwrap();
        for args in [
            &["init", "-q"][..],
            &["config", "user.email", "codediff@example.com"],
            &["config", "user.name", "codediff e2e"],
            &["add", "-A"],
            &["commit", "-qm", "initial"],
        ] {
            assert!(
                Command::new("git")
                    .args(args)
                    .current_dir(&repo)
                    .status()
                    .unwrap()
                    .success()
            );
        }
        fs::write(repo.join("src/a.rs"), lines("a CHANGED")).unwrap();
        fs::write(repo.join("src/b.rs"), lines("b CHANGED")).unwrap();

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
        let (_collector, output) = collect(pty.master.try_clone_reader().unwrap());
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

    fn send(&mut self, input: &[u8], expected: &[&str]) -> String {
        send_and_expect(&mut self.writer, &self.output, SIZE, input, expected)
    }

    fn screen(&self) -> String {
        screen_text(&self.output, SIZE.0, SIZE.1)
    }

    fn logged(&self, message: &str) -> usize {
        fs::read_to_string(&self.log)
            .unwrap_or_default()
            .matches(message)
            .count()
    }

    /// Waits until codediff has logged `message` at least `count` times.
    fn wait_for_log(&self, message: &str, count: usize) {
        let deadline = Instant::now() + Duration::from_secs(20);
        while self.logged(message) < count {
            assert!(
                Instant::now() < deadline,
                "codediff never logged {message:?} {count} times"
            );
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    /// Selects src/a.rs, and waits until its diff is shown on its own.
    fn open_a(&mut self) {
        // The first key selects src, whose cards show a.rs under a header.
        self.send(b"j", &["M src/a.rs"]);
        self.send(b"j", &[]);
        let deadline = Instant::now() + Duration::from_secs(20);
        loop {
            let screen = self.screen();
            if screen.contains("a CHANGED 1")
                && !screen.contains("src/a.rs")
                && !screen.contains("Loading file")
            {
                return;
            }
            assert!(
                Instant::now() < deadline,
                "src/a.rs did not open:\n{screen}"
            );
            std::thread::sleep(Duration::from_millis(20));
        }
    }

    /// Waits until a whole `SETTLED` period passes without a refresh, so
    /// events from setting up the repository are not counted later.
    fn settle(&self) {
        let mut refreshes = self.logged("refresh triggered");
        loop {
            std::thread::sleep(SETTLED);
            let now = self.logged("refresh triggered");
            if now == refreshes {
                return;
            }
            refreshes = now;
        }
    }

    /// Changes a file's modification time and nothing else.
    ///
    /// The new time lies in the past: Git does not record the times of files
    /// changed in the same second as the index, so a time of "now" can hide
    /// whether a command rewrites the index.
    fn change_modified_time(&self, path: &str) {
        let file = fs::File::options()
            .write(true)
            .open(self.repo.join(path))
            .unwrap();
        file.set_modified(SystemTime::now() - Duration::from_secs(60))
            .unwrap();
    }
}

impl Drop for App {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
        let _ = fs::remove_dir_all(&self.dir);
    }
}

fn index_modified(repo: &Path) -> SystemTime {
    fs::metadata(repo.join(".git/index"))
        .unwrap()
        .modified()
        .unwrap()
}

#[test]
fn listing_files_after_a_change_does_not_rewrite_the_index() {
    let app = App::start("index");
    app.settle();
    let listed = app.logged("listed files");
    let refreshes = app.logged("refresh triggered");
    let index = index_modified(&app.repo);

    app.change_modified_time("src/c.rs");
    app.wait_for_log("listed files", listed + 1);

    // Writing the index would wake the watcher and refresh everything again.
    assert_eq!(
        index_modified(&app.repo),
        index,
        "listing the files rewrote .git/index"
    );
    std::thread::sleep(SETTLED);
    assert_eq!(
        app.logged("refresh triggered"),
        refreshes + 1,
        "one change on disk caused more than one refresh"
    );
}

#[test]
fn a_change_that_alters_no_file_keeps_the_directory_cards_where_they_were() {
    let mut app = App::start("directory");
    // Select src, then scroll down inside its cards.
    app.send(b"j", &["a CHANGED 1"]);
    for _ in 0..5 {
        app.send(b"\x1b[<65;90;15M", &[]);
    }
    // Five wheel steps of three lines put line 14 at the top of the pane.
    let deadline = Instant::now() + Duration::from_secs(20);
    let before = loop {
        let screen = app.screen();
        if screen
            .lines()
            .nth(1)
            .is_some_and(|line| line.contains(" 14 a line 14 "))
        {
            break screen;
        }
        assert!(
            Instant::now() < deadline,
            "the cards did not scroll:\n{screen}"
        );
        std::thread::sleep(Duration::from_millis(20));
    };
    let listed = app.logged("listed files");
    let reads = app.logged("reading file");

    app.change_modified_time("src/a.rs");
    app.wait_for_log("listed files", listed + 1);

    let deadline = Instant::now() + SETTLED;
    while Instant::now() < deadline {
        assert_eq!(app.screen(), before, "the cards moved after a refresh");
        std::thread::sleep(Duration::from_millis(25));
    }
    assert_eq!(
        app.logged("reading file"),
        reads,
        "unchanged files were read again"
    );
}

#[test]
fn a_change_that_alters_nothing_never_shows_the_open_file_loading() {
    let mut app = App::start("file");
    app.open_a();
    let reads = app.logged("reading file");
    let shown = app.output.lock().unwrap().bytes.len();

    app.change_modified_time("src/a.rs");
    // Both sides are read again; wait until that has been drawn.
    app.wait_for_log("reading file", reads + 2);
    wait_for_screen(&app.output, SIZE, &["a CHANGED 1"]);
    std::thread::sleep(SETTLED);

    let drawn = String::from_utf8_lossy(&app.output.lock().unwrap().bytes[shown..]).into_owned();
    assert!(
        !drawn.contains("Loading file"),
        "the open file was replaced by a loading message"
    );
}

#[test]
fn a_real_change_to_the_open_file_appears_without_a_loading_message() {
    let mut app = App::start("change");
    app.open_a();
    let listed = app.logged("listed files");
    let shown = app.output.lock().unwrap().bytes.len();

    // Half the lines go back to the commit, so the line counts change too and
    // the file is listed anew.
    let text: String = (1..=60)
        .map(|line| match line {
            1..=30 => format!("a EDITED {line}\n"),
            _ => format!("a line {line}\n"),
        })
        .collect();
    fs::write(app.repo.join("src/a.rs"), text).unwrap();
    wait_for_screen(&app.output, SIZE, &["a EDITED 1"]);
    // The new listing reaches the Explorer after the file itself.
    app.wait_for_log("listed files", listed + 1);
    std::thread::sleep(SETTLED);

    let drawn = String::from_utf8_lossy(&app.output.lock().unwrap().bytes[shown..]).into_owned();
    assert!(
        !drawn.contains("Loading file"),
        "the open file was replaced by a loading message"
    );
}
