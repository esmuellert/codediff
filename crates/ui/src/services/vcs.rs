//! Runs repository queries and staging on one background thread.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::thread;

use anyhow::Context;
use file_types::{File, RepoPath, Rev, Revs};
use vcs::{Commit, CommitOrder, Repository, RevisionRange};

use super::{Resource, Writer};
use crate::components::revisions::find_commit;

const WARM_COMMIT_LIMIT: usize = 500;

struct CachedFirstPage {
    requested_repo: PathBuf,
    repo_root: PathBuf,
    limit: usize,
    commits: Vec<Commit>,
}

type FirstPageCache = Arc<Mutex<Option<CachedFirstPage>>>;

enum Request {
    ListFiles {
        repo: PathBuf,
        revision_range: RevisionRange,
        out: Writer<Vec<File>>,
    },
    ListCommits {
        repo: PathBuf,
        limit: usize,
        out: Writer<Vec<Commit>>,
    },
    WarmCommits {
        repo: PathBuf,
    },
    ResolveCommits {
        repo: PathBuf,
        revs: Vec<String>,
        out: Writer<Vec<Commit>>,
    },
    ToggleStage {
        path: RepoPath,
        after: Rev,
    },
}

/// Lists changed files, lists commits, and toggles staging on one `"vcs"` thread.
pub struct VcsService {
    requests: Sender<Request>,
    pending_jobs: Arc<AtomicUsize>,
    first_page_cache: FirstPageCache,
}

impl VcsService {
    /// Starts the `"vcs"` worker thread backed by real Git repositories.
    pub fn start(redraw: impl Fn() + Send + Sync + 'static) -> Self {
        let mut cached: Option<(PathBuf, Repository)> = None;
        let first_page_cache: FirstPageCache = Arc::new(Mutex::new(None));
        let worker_cache = Arc::clone(&first_page_cache);
        Self::spawn(redraw, first_page_cache, move |request| match request {
            Request::ListFiles {
                repo,
                revision_range,
                out,
            } => {
                out.fill(|| {
                    list_files(&mut cached, &repo, &revision_range)
                        .map_err(|why| format!("{why:#}"))
                });
                true
            }
            Request::ListCommits { repo, limit, out } => {
                out.fill(|| {
                    let (root, commits) = list_commits(&mut cached, &repo, limit)
                        .map_err(|why| format!("{why:#}"))?;
                    store_first_page(&worker_cache, &repo, &root, limit, &commits);
                    Ok(commits)
                });
                true
            }
            Request::WarmCommits { repo } => {
                if let Ok((root, commits)) = list_commits(&mut cached, &repo, WARM_COMMIT_LIMIT) {
                    store_first_page(&worker_cache, &repo, &root, WARM_COMMIT_LIMIT, &commits);
                }
                false
            }
            Request::ResolveCommits { repo, revs, out } => {
                out.fill(|| {
                    resolve_commits(&mut cached, &repo, &revs).map_err(|why| format!("{why:#}"))
                });
                true
            }
            Request::ToggleStage { path, after } => {
                let _ = toggle_stage(&mut cached, &path, &after);
                false
            }
        })
    }

    /// Starts the `"vcs"` worker thread with fake file and commit closures for tests and stories.
    pub fn fake(
        redraw: impl Fn() + Send + Sync + 'static,
        mut list_files: impl FnMut(&Path, &RevisionRange) -> Result<Vec<File>, String> + Send + 'static,
        mut list_commits: impl FnMut(&Path, usize) -> Result<Vec<Commit>, String> + Send + 'static,
    ) -> Self {
        let first_page_cache: FirstPageCache = Arc::new(Mutex::new(None));
        let worker_cache = Arc::clone(&first_page_cache);
        Self::spawn(redraw, first_page_cache, move |request| match request {
            Request::ListFiles {
                repo,
                revision_range,
                out,
            } => {
                out.fill(|| list_files(&repo, &revision_range));
                true
            }
            Request::ListCommits { repo, limit, out } => {
                out.fill(|| {
                    let result = list_commits(&repo, limit);
                    if let Ok(commits) = &result {
                        store_first_page(&worker_cache, &repo, &repo, limit, commits);
                    }
                    result
                });
                true
            }
            Request::WarmCommits { repo } => {
                if let Ok(commits) = list_commits(&repo, WARM_COMMIT_LIMIT) {
                    store_first_page(&worker_cache, &repo, &repo, WARM_COMMIT_LIMIT, &commits);
                }
                false
            }
            Request::ResolveCommits { repo, revs, out } => {
                out.fill(|| {
                    let all = list_commits(&repo, usize::MAX)?;
                    Ok(filter_resolved_commits(&all, &revs))
                });
                true
            }
            Request::ToggleStage { .. } => false,
        })
    }

    fn spawn(
        redraw: impl Fn() + Send + Sync + 'static,
        first_page_cache: FirstPageCache,
        handle: impl FnMut(Request) -> bool + Send + 'static,
    ) -> Self {
        let redraw: Arc<dyn Fn() + Send + Sync> = Arc::new(redraw);
        let pending_jobs = Arc::new(AtomicUsize::new(0));
        let (requests, receiver) = channel();
        let worker_pending = Arc::clone(&pending_jobs);
        thread::Builder::new()
            .name("vcs".to_owned())
            .spawn(move || worker(handle, receiver, &redraw, &worker_pending))
            .expect("the vcs thread starts");
        Self {
            requests,
            pending_jobs,
            first_page_cache,
        }
    }

    /// Starts listing changed files for `revision_range` under `repo`.
    pub fn list_files(&self, repo: &Path, revision_range: RevisionRange) -> Resource<Vec<File>> {
        let (resource, out) = Resource::pending();
        self.pending_jobs.fetch_add(1, Ordering::SeqCst);
        self.requests
            .send(Request::ListFiles {
                repo: repo.to_path_buf(),
                revision_range,
                out,
            })
            .expect("the vcs thread is running");
        resource
    }

    /// Starts listing up to `limit` commits under `repo`, returning synchronously on a first-page cache hit.
    pub fn list_commits(&self, repo: &Path, limit: usize) -> Resource<Vec<Commit>> {
        if let Some(cached) = read_first_page(&self.first_page_cache, repo, limit) {
            return Resource::ready(Ok(cached));
        }
        let (resource, out) = Resource::pending();
        self.pending_jobs.fetch_add(1, Ordering::SeqCst);
        self.requests
            .send(Request::ListCommits {
                repo: repo.to_path_buf(),
                limit,
                out,
            })
            .expect("the vcs thread is running");
        resource
    }

    /// Queues a quiet background prewarm of the first commit page under `repo`.
    pub fn warm_commits(&self, repo: &Path) {
        self.pending_jobs.fetch_add(1, Ordering::SeqCst);
        self.requests
            .send(Request::WarmCommits {
                repo: repo.to_path_buf(),
            })
            .expect("the vcs thread is running");
    }

    /// Clears the cached first commit page so the next lookup or prewarm reads fresh refs.
    pub fn invalidate_commits(&self) {
        *self.first_page_cache.lock().unwrap() = None;
    }

    /// Starts resolving `revs` under `repo` into deduplicated commits, returning synchronously when
    /// every requested revision resolves in the warm first-page cache.
    pub fn resolve_commits(&self, repo: &Path, revs: Vec<String>) -> Resource<Vec<Commit>> {
        if let Some(resolved) = resolve_from_first_page(&self.first_page_cache, repo, &revs) {
            return Resource::ready(Ok(resolved));
        }
        let (resource, out) = Resource::pending();
        self.pending_jobs.fetch_add(1, Ordering::SeqCst);
        self.requests
            .send(Request::ResolveCommits {
                repo: repo.to_path_buf(),
                revs,
                out,
            })
            .expect("the vcs thread is running");
        resource
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

fn matches_repo(entry: &CachedFirstPage, repo: &Path) -> bool {
    entry.requested_repo == repo || entry.repo_root == repo || repo.starts_with(&entry.repo_root)
}

fn store_first_page(
    cache: &FirstPageCache,
    requested_repo: &Path,
    repo_root: &Path,
    limit: usize,
    commits: &[Commit],
) {
    let mut guard = cache.lock().unwrap();
    let keep_existing = guard
        .as_ref()
        .is_some_and(|entry| matches_repo(entry, requested_repo) && entry.limit > limit);
    if !keep_existing {
        *guard = Some(CachedFirstPage {
            requested_repo: requested_repo.to_path_buf(),
            repo_root: repo_root.to_path_buf(),
            limit,
            commits: commits.to_vec(),
        });
    }
}

fn read_first_page(cache: &FirstPageCache, repo: &Path, limit: usize) -> Option<Vec<Commit>> {
    let guard = cache.lock().unwrap();
    let entry = guard.as_ref()?;
    if matches_repo(entry, repo) && entry.limit >= limit {
        Some(entry.commits[..limit.min(entry.commits.len())].to_vec())
    } else {
        None
    }
}

fn resolve_from_first_page(
    cache: &FirstPageCache,
    repo: &Path,
    revs: &[String],
) -> Option<Vec<Commit>> {
    let guard = cache.lock().unwrap();
    let entry = guard.as_ref()?;
    if !matches_repo(entry, repo) {
        return None;
    }
    let mut resolved: Vec<Commit> = Vec::new();
    for rev in revs {
        let commit = find_commit(rev, &entry.commits)?;
        if !resolved.iter().any(|seen| seen.hash == commit.hash) {
            resolved.push(commit.clone());
        }
    }
    Some(resolved)
}

fn filter_resolved_commits(commits: &[Commit], revs: &[String]) -> Vec<Commit> {
    let mut resolved: Vec<Commit> = Vec::new();
    for rev in revs {
        if let Some(commit) = find_commit(rev, commits)
            && !resolved.iter().any(|seen| seen.hash == commit.hash)
        {
            resolved.push(commit.clone());
        }
    }
    resolved
}

fn worker(
    mut handle: impl FnMut(Request) -> bool,
    requests: Receiver<Request>,
    redraw: &Arc<dyn Fn() + Send + Sync>,
    pending_jobs: &AtomicUsize,
) {
    for request in requests {
        if handle(request) {
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
) -> anyhow::Result<(PathBuf, Vec<Commit>)> {
    let repository = repository(cached, repo)?;
    let root = repository.repo_path().root.clone();
    let commits = repository
        .list_commits(CommitOrder::Topological, limit)
        .context("listing commits")?;
    Ok((root, commits))
}

fn resolve_commits(
    cached: &mut Option<(PathBuf, Repository)>,
    repo: &Path,
    revs: &[String],
) -> anyhow::Result<Vec<Commit>> {
    repository(cached, repo)?
        .resolve_commits(revs)
        .context("resolving commits")
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
