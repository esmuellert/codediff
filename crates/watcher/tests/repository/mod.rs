use std::fs;
use std::path::Path;
use std::process::Command;
use std::sync::mpsc::Receiver;
use std::time::Duration;

use watcher::{Refresh, Subscription};

pub const EVENT_TIMEOUT: Duration = Duration::from_secs(3);

fn git_in(dir: &Path, args: &[&str]) -> bool {
    Command::new("git")
        .args(args)
        .current_dir(dir)
        .env("GIT_AUTHOR_NAME", "Test")
        .env("GIT_AUTHOR_EMAIL", "t@t")
        .env("GIT_COMMITTER_NAME", "Test")
        .env("GIT_COMMITTER_EMAIL", "t@t")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .unwrap()
        .success()
}

pub struct Repo {
    dir: tempfile::TempDir,
}

impl Repo {
    pub fn path(&self) -> &Path {
        self.dir.path()
    }

    fn git(&self, args: &[&str]) -> bool {
        git_in(self.path(), args)
    }
}

pub fn committed() -> Repo {
    let repo = Repo {
        dir: tempfile::tempdir().unwrap(),
    };
    repo.git(&["init", "-b", "main"]);
    fs::write(repo.path().join(".gitignore"), "target/\n").unwrap();
    fs::write(repo.path().join("file.txt"), "aaaa\n").unwrap();
    repo.git(&["add", "."]);
    repo.git(&["commit", "-m", "init"]);
    repo
}

pub fn subscribe(repo: &Repo) -> (Subscription, Receiver<Refresh>) {
    let (tx, rx) = std::sync::mpsc::channel();
    let emitter = channel::Emitter::new(tx, std::convert::identity);
    let watcher = watcher::subscribe(repo.path(), emitter).unwrap();
    std::thread::sleep(Duration::from_millis(500));
    drain_events(&rx);
    (watcher, rx)
}

pub fn watched() -> (Repo, Subscription, Receiver<Refresh>) {
    let repo = committed();
    let (watcher, rx) = subscribe(&repo);
    (repo, watcher, rx)
}

pub fn wait_for_refresh(rx: &Receiver<Refresh>) -> Refresh {
    rx.recv_timeout(EVENT_TIMEOUT).unwrap_or_default()
}

pub fn drain_events(rx: &Receiver<Refresh>) {
    while rx.try_recv().is_ok() {}
}
