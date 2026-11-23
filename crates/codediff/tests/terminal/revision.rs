//! End-to-end terminal coverage for `codediff [REV]`.

use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, Instant, SystemTime};

use portable_pty::{CommandBuilder, PtySize, native_pty_system};

use super::common::{send_and_expect, wait_for_screen};
use super::pty::{ENTER_ALT, LEAVE_ALT, collect, on_a_terminal, written};

const SIZE: (u16, u16) = (24, 120);

struct Repo {
    dir: PathBuf,
}

impl Repo {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "codediff-revision-{name}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        fs::create_dir_all(&dir).expect("creating test repository");
        let repo = Self { dir };
        repo.git(&["init", "-q"]);
        repo.git(&["config", "user.email", "codediff@example.com"]);
        repo.git(&["config", "user.name", "codediff e2e"]);
        repo
    }

    fn git(&self, args: &[&str]) {
        let out = Command::new("git")
            .args(args)
            .current_dir(&self.dir)
            .output()
            .expect("running git");
        assert!(out.status.success(), "git {args:?} failed: {out:?}");
    }

    fn write(&self, path: &str, text: &str) {
        fs::write(self.dir.join(path), text).expect("writing test file");
    }
}

impl Drop for Repo {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

fn review_and_quit(
    repo: &Repo,
    args: &[&str],
    expected_files: &[&str],
    expected_diff: &[&str],
) -> String {
    let pty = native_pty_system()
        .openpty(PtySize {
            rows: SIZE.0,
            cols: SIZE.1,
            pixel_width: 0,
            pixel_height: 0,
        })
        .expect("opening a pty");

    let mut command = CommandBuilder::new(env!("CARGO_BIN_EXE_codediff"));
    command.args(args);
    command.cwd(&repo.dir);
    command.env("TERM", "xterm-256color");

    let mut child = pty.slave.spawn_command(command).expect("spawning codediff");
    drop(pty.slave);

    let reader = pty.master.try_clone_reader().expect("reading the pty");
    let (collector, output) = collect(reader);
    let mut writer = pty.master.take_writer().expect("writing to the pty");

    let initial = wait_for_screen(&output, SIZE, expected_files);
    assert!(
        !initial.contains("dirty-only.txt"),
        "dirty worktree file leaked into committed revision view:\n{initial}"
    );

    send_and_expect(&mut writer, &output, SIZE, b"j", expected_diff);

    writer.write_all(b"q").expect("sending quit");
    writer.flush().expect("flushing quit");
    drop(writer);

    let deadline = Instant::now() + Duration::from_secs(20);
    let status = loop {
        if let Some(status) = child.try_wait().expect("waiting for codediff") {
            break status;
        }
        assert!(Instant::now() < deadline, "codediff {args:?} never exited");
        std::thread::sleep(Duration::from_millis(25));
    };

    drop(pty.master);
    let raw = written(collector, &output);
    assert!(status.success(), "codediff {args:?} failed:\n{raw}");
    assert!(raw.contains(ENTER_ALT));
    assert!(raw.contains(LEAVE_ALT));
    raw
}

#[test]
fn revision_range_opens_committed_changes_and_ignores_dirty_worktree() {
    let repo = Repo::new("range");
    repo.write("committed.txt", "before line\n");
    repo.write("untouched.txt", "stay\n");
    repo.git(&["add", "-A"]);
    repo.git(&["commit", "-qm", "first"]);

    repo.write("committed.txt", "after committed line\n");
    repo.git(&["add", "-A"]);
    repo.git(&["commit", "-qm", "second"]);

    // Leave uncommitted changes on disk that must not appear in HEAD~1..HEAD.
    repo.write("dirty-only.txt", "uncommitted\n");
    repo.write("untouched.txt", "dirty edit\n");

    review_and_quit(
        &repo,
        &["HEAD~1..HEAD"],
        &["committed.txt"],
        &["before line", "after committed line"],
    );
}

#[test]
fn double_dash_reviews_a_branch_named_doctor() {
    let repo = Repo::new("doctor-branch");
    repo.write("branch.txt", "from doctor branch\n");
    repo.git(&["add", "-A"]);
    repo.git(&["commit", "-qm", "doctor base"]);
    repo.git(&["branch", "-M", "doctor"]);

    // Commit on `feature` so the worktree is completely clean (`RevisionRange::Worktree`
    // would list zero files); only `RevisionRange::Against("doctor")` finds `branch.txt`.
    repo.git(&["checkout", "-qb", "feature"]);
    repo.write("branch.txt", "committed on feature\n");
    repo.git(&["add", "-A"]);
    repo.git(&["commit", "-qm", "feature commit"]);

    review_and_quit(
        &repo,
        &["--", "doctor"],
        &["branch.txt"],
        &["from doctor branch", "committed on feature"],
    );
}

#[test]
fn invalid_revision_exits_nonzero_without_entering_alternate_screen() {
    let repo = Repo::new("invalid-rev");
    repo.write("file.txt", "hello\n");
    repo.git(&["add", "-A"]);
    repo.git(&["commit", "-qm", "initial"]);

    let (output, ok) = on_a_terminal(&["nonexistent-rev"], Some(&repo.dir), &[]);
    assert!(!ok, "expected codediff nonexistent-rev to fail:\n{output}");
    assert!(
        output.contains("nonexistent-rev"),
        "expected error mentioning revision, got:\n{output}"
    );
    assert!(
        !output.contains(ENTER_ALT),
        "invalid revision entered the alternate screen:\n{output:?}"
    );
}
