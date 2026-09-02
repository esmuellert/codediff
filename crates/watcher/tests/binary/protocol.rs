use std::io::{BufRead, BufReader};
use std::process::{Command, Stdio};
use std::sync::mpsc;

use crate::process::{
    PROCESS_TIMEOUT, WatcherProcess, binary, committed, committed_with_prefix, git, parse_message,
    run_to_exit, wait_for_exit,
};

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
    process.assert_quiet();
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
