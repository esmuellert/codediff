use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use diff_service::DiffService;
use diff_service::diff::Settings;

struct TempRepo(PathBuf);

impl Drop for TempRepo {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn git(dir: &Path, args: &[&str]) {
    let status = Command::new("git")
        .args(args)
        .current_dir(dir)
        .env("GIT_AUTHOR_NAME", "Tester")
        .env("GIT_AUTHOR_EMAIL", "tester@example.com")
        .env("GIT_COMMITTER_NAME", "Tester")
        .env("GIT_COMMITTER_EMAIL", "tester@example.com")
        .status()
        .expect("running git");
    assert!(status.success(), "git {args:?} failed");
}

#[test]
fn list_commits_loads_repository_history_and_notifies_redraw() {
    let dir = std::env::temp_dir().join(format!(
        "codediff-list-commits-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&dir).expect("create tempdir");
    let repo = TempRepo(dir);
    let root = repo.0.as_path();

    git(root, &["init", "-b", "main", "--quiet"]);
    fs::write(root.join("a.txt"), "one\n").unwrap();
    git(root, &["add", "."]);
    git(root, &["commit", "-m", "first commit", "--quiet"]);
    fs::write(root.join("a.txt"), "two\n").unwrap();
    git(root, &["add", "."]);
    git(root, &["commit", "-m", "second commit", "--quiet"]);

    let (tx, rx) = mpsc::channel();
    let service = DiffService::start(
        move || {
            let _ = tx.send(());
        },
        Settings::default(),
    );
    let slot = service.list_commits(root, 10);

    rx.recv_timeout(Duration::from_secs(5))
        .expect("redraw notification after list_commits");
    let commits = slot
        .lock()
        .unwrap()
        .clone()
        .expect("commits populated")
        .expect("commits ok");
    assert_eq!(commits.len(), 2);
    assert_eq!(commits[0].subject, "second commit");
    assert_eq!(commits[1].subject, "first commit");
}
