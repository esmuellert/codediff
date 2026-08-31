use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::time::{Duration, Instant};

use serde_json::Value;

const PROCESS_TIMEOUT: Duration = Duration::from_secs(3);

fn git(repo: &Path, arguments: &[&str]) -> bool {
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

fn committed() -> tempfile::TempDir {
    committed_with_prefix("codediff-watcher")
}

fn committed_with_prefix(prefix: &str) -> tempfile::TempDir {
    let repo = tempfile::Builder::new().prefix(prefix).tempdir().unwrap();
    assert!(git(repo.path(), &["init", "-b", "main"]));
    std::fs::write(repo.path().join(".gitignore"), "target/\n").unwrap();
    std::fs::write(repo.path().join("file.txt"), "before\n").unwrap();
    assert!(git(repo.path(), &["add", "."]));
    assert!(git(repo.path(), &["commit", "-m", "init"]));
    repo
}

fn binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_codediff-watcher"))
}

fn parse_message(line: &str) -> Value {
    serde_json::from_str(line).unwrap_or_else(|error| panic!("invalid JSON {line:?}: {error}"))
}

fn wait_for_exit(child: &mut Child) -> std::process::ExitStatus {
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

fn run_to_exit(mut command: Command) -> Output {
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

struct WatcherProcess {
    child: Child,
    lines: Receiver<String>,
}

impl WatcherProcess {
    fn start(repo: &Path) -> Self {
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

    fn next_message(&self) -> Value {
        let line = self
            .lines
            .recv_timeout(PROCESS_TIMEOUT)
            .expect("codediff-watcher must produce a complete line");
        parse_message(&line)
    }

    fn wait_for_refresh(&self, expected: impl Fn(&Value) -> bool) -> Value {
        let deadline = Instant::now() + PROCESS_TIMEOUT;
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            let line = self
                .lines
                .recv_timeout(remaining)
                .expect("codediff-watcher must report the Git change");
            let refresh = parse_message(&line);
            assert_eq!(refresh["type"].as_str(), Some("refresh"));
            for field in ["worktree", "index", "head", "refs"] {
                assert!(refresh[field].is_boolean(), "missing boolean {field}");
            }
            if expected(&refresh) {
                return refresh;
            }
        }
    }
}

impl Drop for WatcherProcess {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[test]
fn version_is_available_without_a_repository() {
    let mut command = Command::new(binary());
    command.arg("--version");
    let output = run_to_exit(command);
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        format!("codediff-watcher {}\n", env!("CARGO_PKG_VERSION"))
    );
    assert!(output.stderr.is_empty());
}

#[test]
fn ready_precedes_refresh_and_both_are_complete_json_lines() {
    let repo = committed();
    let process = WatcherProcess::start(repo.path());

    let ready = process.next_message();
    assert_eq!(ready["type"].as_str(), Some("ready"));
    assert_eq!(ready["protocol"].as_u64(), Some(1));
    assert_eq!(
        ready["binary_version"].as_str(),
        Some(env!("CARGO_PKG_VERSION"))
    );

    std::fs::write(repo.path().join("file.txt"), "changed\n").unwrap();
    let refresh = process.next_message();
    assert_eq!(refresh["type"].as_str(), Some("refresh"));
    assert_eq!(refresh["worktree"].as_bool(), Some(true));
    assert_eq!(refresh["index"].as_bool(), Some(false));
    assert_eq!(refresh["head"].as_bool(), Some(false));
    assert_eq!(refresh["refs"].as_bool(), Some(false));
}

#[test]
fn idle_repository_emits_only_ready() {
    let repo = committed();
    let process = WatcherProcess::start(repo.path());
    assert_eq!(process.next_message()["type"].as_str(), Some("ready"));
    assert!(
        process
            .lines
            .recv_timeout(Duration::from_millis(500))
            .is_err(),
        "an idle repository must not produce refresh messages"
    );
}

#[test]
fn git_state_changes_cross_the_process_boundary() {
    let repo = committed();
    let process = WatcherProcess::start(repo.path());
    assert_eq!(process.next_message()["type"].as_str(), Some("ready"));

    std::fs::write(repo.path().join("file.txt"), "staged\n").unwrap();
    assert!(git(repo.path(), &["add", "file.txt"]));
    process.wait_for_refresh(|refresh| refresh["index"].as_bool() == Some(true));

    assert!(git(repo.path(), &["branch", "topic"]));
    process.wait_for_refresh(|refresh| refresh["refs"].as_bool() == Some(true));

    assert!(git(repo.path(), &["switch", "topic"]));
    process.wait_for_refresh(|refresh| refresh["head"].as_bool() == Some(true));
}

#[test]
fn repository_path_with_spaces_is_accepted() {
    let repo = committed_with_prefix("repo with spaces");
    assert!(repo.path().to_string_lossy().contains(' '));
    let process = WatcherProcess::start(repo.path());
    assert_eq!(process.next_message()["type"].as_str(), Some("ready"));
}

#[test]
fn invalid_repository_exits_without_claiming_readiness() {
    let dir = tempfile::tempdir().unwrap();
    let mut command = Command::new(binary());
    command.arg(dir.path());
    let output = run_to_exit(command);

    assert!(!output.status.success());
    assert!(
        output.stdout.is_empty(),
        "ready must not precede startup success"
    );
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("not a Git worktree"),
        "stderr should explain the startup failure"
    );
}

#[test]
fn missing_repository_argument_is_an_error() {
    let output = run_to_exit(Command::new(binary()));
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("usage:"));
}

#[test]
fn extra_repository_argument_is_an_error() {
    let mut command = Command::new(binary());
    command.args(["first", "second"]);
    let output = run_to_exit(command);
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    assert!(String::from_utf8_lossy(&output.stderr).contains("usage:"));
}

#[test]
fn refresh_write_to_closed_stdout_stops_process() {
    let repo = committed();
    let mut child = Command::new(binary())
        .arg(repo.path())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let stdout = child.stdout.take().unwrap();
    let (ready_sender, ready_receiver) = mpsc::channel();
    std::thread::spawn(move || {
        let mut line = String::new();
        let mut stdout = BufReader::new(stdout);
        let read = stdout.read_line(&mut line);
        drop(stdout);
        if read.is_ok() {
            let _ = ready_sender.send(line);
        }
    });

    let ready = ready_receiver.recv_timeout(PROCESS_TIMEOUT).unwrap();
    assert_eq!(parse_message(&ready)["type"].as_str(), Some("ready"));
    std::fs::write(repo.path().join("file.txt"), "after stdout closed\n").unwrap();

    assert!(!wait_for_exit(&mut child).success());
}
