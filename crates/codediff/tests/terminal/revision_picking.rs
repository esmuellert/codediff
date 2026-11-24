//! End-to-end real-PTY tests for interactive revision picking and swapping.

use std::fs;
use std::io::Write;
use std::num::NonZero;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use portable_pty::{CommandBuilder, PtySize, native_pty_system};

use super::pty::{collect, drawn};

fn git(dir: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args(args)
        .current_dir(dir)
        .env("GIT_AUTHOR_NAME", "Tester")
        .env("GIT_AUTHOR_EMAIL", "tester@example.com")
        .env("GIT_COMMITTER_NAME", "Tester")
        .env("GIT_COMMITTER_EMAIL", "tester@example.com")
        .output()
        .expect("running git");
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .expect("utf-8")
        .trim()
        .to_owned()
}

struct TempRepo(PathBuf);

impl TempRepo {
    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for TempRepo {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn build_test_repo() -> TempRepo {
    let dir = std::env::temp_dir().join(format!(
        "codediff-rev-pick-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&dir).expect("create tempdir");
    let dir = TempRepo(dir);
    let root = dir.path();
    git(root, &["init", "-b", "main", "--quiet"]);

    fs::write(
        root.join("alpha.txt"),
        "v1 line
",
    )
    .unwrap();
    git(root, &["add", "."]);
    git(root, &["commit", "-m", "initial v1", "--quiet"]);
    git(root, &["tag", "v0.1.0"]);

    fs::write(
        root.join("alpha.txt"),
        "v2 line
extra line
",
    )
    .unwrap();
    fs::write(
        root.join("beta.txt"),
        "added in v2
",
    )
    .unwrap();
    git(root, &["add", "."]);
    git(root, &["commit", "-m", "second v2", "--quiet"]);

    // Create both unstaged and staged changes in the worktree
    fs::write(
        root.join("staged.txt"),
        "staged content
",
    )
    .unwrap();
    git(root, &["add", "staged.txt"]);
    fs::write(
        root.join("dirty.txt"),
        "unstaged content
",
    )
    .unwrap();

    dir
}

struct PtyApp {
    child: Box<dyn portable_pty::Child + Send + Sync>,
    writer: Box<dyn Write + Send>,
    output: Arc<Mutex<super::pty::Output>>,
    parser: vt100::Parser,
    processed: usize,
    _master: Box<dyn portable_pty::MasterPty + Send>,
    _collector: std::thread::JoinHandle<()>,
}

impl PtyApp {
    fn start(repo: &Path) -> Self {
        let pty = native_pty_system()
            .openpty(PtySize {
                rows: 24,
                cols: 120,
                pixel_width: 0,
                pixel_height: 0,
            })
            .expect("open pty");
        let mut cmd = CommandBuilder::new(env!("CARGO_BIN_EXE_codediff"));
        cmd.cwd(repo);
        cmd.env("TERM", "xterm-256color");
        let child = pty.slave.spawn_command(cmd).expect("spawn codediff");
        drop(pty.slave);

        let reader = pty.master.try_clone_reader().expect("clone reader");
        let (collector, output) = collect(reader);
        drawn(&output);
        let writer = pty.master.take_writer().expect("take writer");
        let parser = vt100::Parser::new(NonZero::new(24).unwrap(), NonZero::new(120).unwrap(), 0);
        Self {
            child,
            writer,
            output,
            parser,
            processed: 0,
            _master: pty.master,
            _collector: collector,
        }
    }

    fn screen(&mut self) -> String {
        let bytes = self.output.lock().unwrap().bytes.clone();
        if bytes.len() > self.processed {
            self.parser.process(&bytes[self.processed..]);
            self.processed = bytes.len();
        }
        self.parser.screen().contents()
    }

    fn send(&mut self, bytes: &[u8]) {
        self.writer.write_all(bytes).unwrap();
        self.writer.flush().unwrap();
    }

    fn wait_for(&mut self, label: &str, mut pred: impl FnMut(&str) -> bool) -> String {
        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            let s = self.screen();
            if pred(&s) {
                return s;
            }
            assert!(
                Instant::now() < deadline,
                "timed out waiting for {label}; last screen:
{s}"
            );
            std::thread::sleep(Duration::from_millis(25));
        }
    }

    fn quit(mut self) {
        let _ = self.writer.write_all(b"q");
        let _ = self.writer.flush();
        let deadline = Instant::now() + Duration::from_secs(10);
        while self.child.try_wait().unwrap().is_none() {
            assert!(Instant::now() < deadline, "codediff did not exit on q");
            std::thread::sleep(Duration::from_millis(25));
        }
    }
}

#[test]
fn interactive_revision_picking_search_swap_and_return_to_worktree() {
    let repo = build_test_repo();
    let mut app = PtyApp::start(repo.path());

    // 1. Wait for header to load commits (main · <oid>) and explorer to show worktree groups
    let initial = app.wait_for("initial worktree header", |s| {
        s.contains("Working tree ▾")
            && s.contains("\u{f418} main ▾")
            && s.contains("Staged Changes")
            && s.contains("dirty.txt")
    });
    assert!(
        !initial.contains("⇄"),
        "worktree should hide swap:
{initial}"
    );

    // 2. Press 'a' -> left picker opens, MUST NOT show Working tree row inside picker
    app.send(b"a");
    let left_picker = app.wait_for("left picker commits", |s| {
        s.contains("initial v1") && s.contains("second v2")
    });
    // The only "Working tree" on screen should be in the top header row (row 0)
    let body_lines: String = left_picker.lines().skip(1).collect::<Vec<_>>().join(
        "
",
    );
    assert!(
        !body_lines.contains("Working tree"),
        "left picker must not include Working tree:
{left_picker}"
    );

    // 3. Inside left picker, test '/' search with 'a', 'b', 's', 'q' (must not trigger global actions),
    // then Esc exits search, second Esc closes picker (without quitting codediff!)
    app.send(b"/absq");
    app.wait_for("search query absq", |s| s.contains("> absq"));
    app.send(b"\x1b");
    app.wait_for("search closed", |s| {
        !s.contains("> absq") && s.contains("initial v1")
    });
    app.send(b"\x1b");
    app.wait_for("picker closed", |s| {
        !s.contains("initial v1") && s.contains("dirty.txt")
    });

    // 4. Open left picker again, select v0.1.0 (initial v1)
    app.send(b"a");
    app.wait_for("left picker reopened", |s| s.contains("v0.1.0"));
    app.send(b"/v0.1.0\r");
    app.wait_for("left picker filtered to v0.1.0", |s| {
        s.contains("initial v1")
    });
    app.send(b"\r");
    let against = app.wait_for("against v0.1.0", |s| {
        s.contains("v0.1.0") && s.contains("Working tree ▾") && s.contains("beta.txt")
    });
    assert!(
        !against.contains("⇄"),
        "Against(rev) should still hide swap:
{against}"
    );

    // 5. Press 'b' -> right picker opens WITH Working tree pinned at top; pick second v2 (HEAD)
    app.send(b"b");
    let right_picker = app.wait_for("right picker with Working tree", |s| {
        let body: String = s.lines().skip(1).collect::<Vec<_>>().join(
            "
",
        );
        body.contains("Working tree") && body.contains("changed ·") && body.contains("second v2")
    });
    assert!(
        right_picker.contains("changed ·"),
        "expected worktree summary:
{right_picker}"
    );
    app.send(b"/second v2\r");
    app.wait_for("right picker filtered to second v2", |s| {
        s.contains("second v2")
    });
    app.send(b"\r");
    let between = app.wait_for("between v0.1.0 and main", |s| {
        s.contains("⇄") && s.contains("v0.1.0") && s.contains("\u{f418} main ▾")
    });
    let header_before_swap = between.lines().next().unwrap_or_default().to_owned();
    assert!(
        header_before_swap.find("v0.1.0").unwrap()
            < header_before_swap.find("\u{f418} main").unwrap(),
        "expected v0.1.0 on left before swap: {header_before_swap}"
    );

    // 6. Press 's' to swap endpoints
    app.send(b"s");
    let swapped = app.wait_for("swapped endpoints", |s| {
        let h = s.lines().next().unwrap_or_default();
        h.find("\u{f418} main").is_some()
            && h.find("v0.1.0").is_some()
            && h.find("\u{f418} main").unwrap() < h.find("v0.1.0").unwrap()
    });
    assert!(
        swapped.contains("⇄"),
        "swapped Between should still show swap"
    );

    // 7. Return right endpoint to Working tree and left endpoint to main -> restores Staged Changes
    app.send(b"b");
    app.wait_for("right picker for worktree", |s| {
        s.lines().skip(1).any(|line| line.contains("Working tree"))
    });
    app.send(b"/Working tree");
    app.wait_for("right picker filtered to Working tree", |s| {
        s.contains("> Working tree")
    });
    app.send(b"\r");
    app.wait_for("back to worktree mode", |s| {
        s.contains("Working tree ▾") && s.contains("Staged Changes") && s.contains("dirty.txt")
    });

    app.quit();
}
