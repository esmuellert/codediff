use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

use serde_json::Value;

pub(super) const PROCESS_TIMEOUT: Duration = Duration::from_secs(3);
const QUIET_PERIOD: Duration = Duration::from_millis(400);

pub(super) fn git(repo: &Path, arguments: &[&str]) -> bool {
    Command::new("git")
        .args(arguments)
        .current_dir(repo)
        .env("GIT_AUTHOR_NAME", "Test")
        .env("GIT_AUTHOR_EMAIL", "t@t")
        .env("GIT_COMMITTER_NAME", "Test")
        .env("GIT_COMMITTER_EMAIL", "t@t")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .unwrap()
        .success()
}

pub(super) fn committed() -> tempfile::TempDir {
    committed_with_prefix("codediff-watcher")
}

pub(super) fn committed_with_prefix(prefix: &str) -> tempfile::TempDir {
    let repo = tempfile::Builder::new().prefix(prefix).tempdir().unwrap();
    assert!(git(repo.path(), &["init", "-b", "main"]));
    std::fs::write(repo.path().join(".gitignore"), "target/\n").unwrap();
    std::fs::write(repo.path().join("file.txt"), "before\n").unwrap();
    assert!(git(repo.path(), &["add", "."]));
    assert!(git(repo.path(), &["commit", "-m", "init"]));
    repo
}

pub(super) fn binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_codediff-watcher"))
}

pub(super) fn parse_message(line: &str) -> Value {
    serde_json::from_str(line).unwrap_or_else(|error| panic!("invalid JSON {line:?}: {error}"))
}

fn parse_refresh(line: &str) -> Value {
    let refresh = parse_message(line);
    assert_eq!(refresh["type"].as_str(), Some("refresh"));
    for field in ["worktree", "index", "head", "refs"] {
        assert!(refresh[field].is_boolean(), "missing boolean {field}");
    }
    refresh
}

pub(super) fn wait_for_exit(child: &mut Child) -> std::process::ExitStatus {
    let deadline = Instant::now() + PROCESS_TIMEOUT;
    loop {
        if let Some(status) = child.try_wait().unwrap() {
            return status;
        }
        if Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            panic!("codediff-watcher did not exit");
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

pub(super) fn run_to_exit(mut command: Command) -> Output {
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let status = wait_for_exit(&mut child);
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    child
        .stdout
        .take()
        .unwrap()
        .read_to_end(&mut stdout)
        .unwrap();
    child
        .stderr
        .take()
        .unwrap()
        .read_to_end(&mut stderr)
        .unwrap();
    Output {
        status,
        stdout,
        stderr,
    }
}

pub(super) struct WatcherProcess {
    child: Child,
    lines: Receiver<String>,
}

impl WatcherProcess {
    pub(super) fn start(repo: &Path) -> Self {
        let mut child = Command::new(binary())
            .arg(repo)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let stdout = child.stdout.take().unwrap();
        let (line_sender, lines) = mpsc::channel();
        std::thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let Ok(line) = line else {
                    break;
                };
                if line_sender.send(line).is_err() {
                    break;
                }
            }
        });
        Self { child, lines }
    }

    pub(super) fn next_message(&self) -> Value {
        let line = self
            .lines
            .recv_timeout(PROCESS_TIMEOUT)
            .expect("codediff-watcher must produce a complete line");
        parse_message(&line)
    }

    pub(super) fn wait_for_refresh(&self, expected: impl Fn(&Value) -> bool) -> Value {
        let deadline = Instant::now() + PROCESS_TIMEOUT;
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            let line = self
                .lines
                .recv_timeout(remaining)
                .expect("codediff-watcher must report the Git change");
            let refresh = parse_refresh(&line);
            if expected(&refresh) {
                return refresh;
            }
        }
    }

    pub(super) fn drain_until_quiet(&self) {
        let deadline = Instant::now() + PROCESS_TIMEOUT;
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            assert!(!remaining.is_zero(), "codediff-watcher never became quiet");
            match self.lines.recv_timeout(QUIET_PERIOD.min(remaining)) {
                Ok(line) => {
                    parse_refresh(&line);
                }
                Err(mpsc::RecvTimeoutError::Timeout) => return,
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    panic!("codediff-watcher closed stdout")
                }
            }
        }
    }

    pub(super) fn assert_quiet(&self) {
        match self.lines.recv_timeout(QUIET_PERIOD) {
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                panic!("codediff-watcher closed stdout")
            }
            Ok(line) => panic!("unexpected watcher message: {line}"),
        }
    }

    pub(super) fn assert_running(&mut self) {
        if let Some(status) = self.child.try_wait().unwrap() {
            panic!("codediff-watcher exited with {status}");
        }
    }
}

impl Drop for WatcherProcess {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
