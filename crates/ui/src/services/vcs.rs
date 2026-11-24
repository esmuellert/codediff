//! Runs repository queries and staging on one background thread.

#![allow(clippy::type_complexity)]

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex, Weak};
use std::thread;

use anyhow::Context;
use file_types::{File, RepoPath, Rev, Revs};
use vcs::{Commit, CommitOrder, Repository, RevisionRange};

enum Request {
    ListFiles {
        repo: PathBuf,
        revision_range: RevisionRange,
        slot: Weak<Mutex<Option<Result<Vec<File>, String>>>>,
    },
    ListCommits {
        repo: PathBuf,
        limit: usize,
        slot: Weak<Mutex<Option<Result<Vec<Commit>, String>>>>,
    },
    ToggleStage {
        path: RepoPath,
        after: Rev,
    },
}

enum Backend {
    Git {
        cached: Option<(PathBuf, Repository)>,
    },
    Fake {
        list_files: Box<dyn FnMut(&Path, &RevisionRange) -> Result<Vec<File>, String> + Send>,
        list_commits: Box<dyn FnMut(&Path, usize) -> Result<Vec<Commit>, String> + Send>,
    },
}

impl Backend {
    fn list_files(
        &mut self,
        repo: &Path,
        revision_range: &RevisionRange,
    ) -> Result<Vec<File>, String> {
        match self {
            Self::Git { cached } => {
                list_files(cached, repo, revision_range).map_err(|why| format!("{why:#}"))
            }
            Self::Fake { list_files, .. } => list_files(repo, revision_range),
        }
    }

    fn list_commits(&mut self, repo: &Path, limit: usize) -> Result<Vec<Commit>, String> {
        match self {
            Self::Git { cached } => {
                list_commits(cached, repo, limit).map_err(|why| format!("{why:#}"))
            }
            Self::Fake { list_commits, .. } => list_commits(repo, limit),
        }
    }

    fn toggle_stage(&mut self, path: &RepoPath, after: &Rev) {
        if let Self::Git { cached } = self {
            let _ = toggle_stage(cached, path, after);
        }
    }
}

/// Lists changed files, lists commits, and toggles staging on one `"vcs"` thread.
pub struct VcsService {
    requests: Sender<Request>,
    pending_jobs: Arc<AtomicUsize>,
}

impl VcsService {
    /// Starts the `"vcs"` worker thread backed by real Git repositories.
    pub fn start(redraw: impl Fn() + Send + Sync + 'static) -> Self {
        Self::spawn(redraw, Backend::Git { cached: None })
    }

    /// Starts the `"vcs"` worker thread with fake file and commit closures for tests and stories.
    pub fn fake(
        redraw: impl Fn() + Send + Sync + 'static,
        list_files: impl FnMut(&Path, &RevisionRange) -> Result<Vec<File>, String> + Send + 'static,
        list_commits: impl FnMut(&Path, usize) -> Result<Vec<Commit>, String> + Send + 'static,
    ) -> Self {
        Self::spawn(
            redraw,
            Backend::Fake {
                list_files: Box::new(list_files),
                list_commits: Box::new(list_commits),
            },
        )
    }

    fn spawn(redraw: impl Fn() + Send + Sync + 'static, backend: Backend) -> Self {
        let redraw: Arc<dyn Fn() + Send + Sync> = Arc::new(redraw);
        let pending_jobs = Arc::new(AtomicUsize::new(0));
        let (requests, receiver) = channel();
        let worker_pending = Arc::clone(&pending_jobs);
        thread::Builder::new()
            .name("vcs".to_owned())
            .spawn(move || worker(backend, receiver, &redraw, &worker_pending))
            .expect("the vcs thread starts");
        Self {
            requests,
            pending_jobs,
        }
    }

    /// Starts listing changed files for `revision_range` under `repo`.
    pub fn list_files(
        &self,
        repo: &Path,
        revision_range: RevisionRange,
    ) -> Arc<Mutex<Option<Result<Vec<File>, String>>>> {
        let slot = Arc::new(Mutex::new(None));
        self.pending_jobs.fetch_add(1, Ordering::SeqCst);
        self.requests
            .send(Request::ListFiles {
                repo: repo.to_path_buf(),
                revision_range,
                slot: Arc::downgrade(&slot),
            })
            .expect("the vcs thread is running");
        slot
    }

    /// Starts listing up to `limit` commits under `repo`.
    pub fn list_commits(
        &self,
        repo: &Path,
        limit: usize,
    ) -> Arc<Mutex<Option<Result<Vec<Commit>, String>>>> {
        let slot = Arc::new(Mutex::new(None));
        self.pending_jobs.fetch_add(1, Ordering::SeqCst);
        self.requests
            .send(Request::ListCommits {
                repo: repo.to_path_buf(),
                limit,
                slot: Arc::downgrade(&slot),
            })
            .expect("the vcs thread is running");
        slot
    }

    /// Stages or unstages `path` according to `revs` on the `"vcs"` worker thread.
    pub fn toggle_stage(&self, path: &RepoPath, revs: &Revs) {
        if !matches!(revs.after, Rev::Index | Rev::Worktree) {
            return;
        }
        self.pending_jobs.fetch_add(1, Ordering::SeqCst);
        self.requests
            .send(Request::ToggleStage {
                path: path.clone(),
                after: revs.after.clone(),
            })
            .expect("the vcs thread is running");
    }

    /// Whether the `"vcs"` thread still has work queued or running.
    pub fn is_busy(&self) -> bool {
        self.pending_jobs.load(Ordering::SeqCst) > 0
    }
}

fn worker(
    mut backend: Backend,
    requests: Receiver<Request>,
    redraw: &Arc<dyn Fn() + Send + Sync>,
    pending_jobs: &AtomicUsize,
) {
    for request in requests {
        let should_redraw = match request {
            Request::ListFiles {
                repo,
                revision_range,
                slot,
            } => {
                if slot.strong_count() > 0 {
                    let result = backend.list_files(&repo, &revision_range);
                    if let Some(slot) = slot.upgrade() {
                        *slot.lock().unwrap() = Some(result);
                    }
                }
                true
            }
            Request::ListCommits { repo, limit, slot } => {
                if slot.strong_count() > 0 {
                    let result = backend.list_commits(&repo, limit);
                    if let Some(slot) = slot.upgrade() {
                        *slot.lock().unwrap() = Some(result);
                    }
                }
                true
            }
            Request::ToggleStage { path, after } => {
                backend.toggle_stage(&path, &after);
                false
            }
        };
        if should_redraw {
            redraw();
        }
        pending_jobs.fetch_sub(1, Ordering::SeqCst);
    }
}

fn repository<'a>(
    cached: &'a mut Option<(PathBuf, Repository)>,
    path: &Path,
) -> anyhow::Result<&'a mut Repository> {
    let matches_cached = cached
        .as_ref()
        .is_some_and(|(key, repo)| key == path || repo.repo_path().root == path);
    if !matches_cached {
        let opened = Repository::open(path).context("opening a repository")?;
        *cached = Some((path.to_path_buf(), opened));
    }
    Ok(&mut cached.as_mut().expect("just populated").1)
}

fn list_files(
    cached: &mut Option<(PathBuf, Repository)>,
    repo: &Path,
    revision_range: &RevisionRange,
) -> anyhow::Result<Vec<File>> {
    tracing::info!("listing files");
    let files = repository(cached, repo)?
        .get_changed_files(revision_range, &[])
        .context("listing changed files and line counts")?;
    tracing::info!(count = files.len(), "listed files");
    Ok(files)
}

fn list_commits(
    cached: &mut Option<(PathBuf, Repository)>,
    repo: &Path,
    limit: usize,
) -> anyhow::Result<Vec<Commit>> {
    repository(cached, repo)?
        .list_commits(CommitOrder::Topological, limit)
        .context("listing commits")
}

fn toggle_stage(
    cached: &mut Option<(PathBuf, Repository)>,
    path: &RepoPath,
    after: &Rev,
) -> anyhow::Result<()> {
    let repo = repository(cached, path.root())?;
    match after {
        Rev::Index => repo.unstage(path.as_str()).context("unstaging path")?,
        Rev::Worktree => repo.stage(path.as_str()).context("staging path")?,
        Rev::Conflict(_) | Rev::Commit(_) => {}
    }
    Ok(())
}
