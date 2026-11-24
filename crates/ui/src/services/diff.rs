//! Loads and diffs files on one background thread.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::thread;

use anyhow::Context;
use diff::{DiffContent, Settings};
use file_types::{DiffVersion, File};
use vcs::Repository;

use super::{Resource, Writer};

/// Reads file contents and computes diffs on one `"diff"` thread.
pub struct DiffService {
    requests: Sender<(File, Writer<Arc<DiffContent>>)>,
    pending_jobs: Arc<AtomicUsize>,
}

impl DiffService {
    /// Starts the `"diff"` worker thread backed by Git repositories.
    pub fn start(redraw: impl Fn() + Send + Sync + 'static, settings: Settings) -> Self {
        let mut cached: Option<(PathBuf, Repository)> = None;
        Self::spawn(redraw, move |file| load_file(&mut cached, file, settings))
    }

    /// Starts the `"diff"` worker thread with a custom file loader for tests and stories.
    pub fn fake(
        redraw: impl Fn() + Send + Sync + 'static,
        load: impl FnMut(&File) -> Result<DiffContent, String> + Send + 'static,
    ) -> Self {
        Self::spawn(redraw, load)
    }

    fn spawn(
        redraw: impl Fn() + Send + Sync + 'static,
        load: impl FnMut(&File) -> Result<DiffContent, String> + Send + 'static,
    ) -> Self {
        let redraw: Arc<dyn Fn() + Send + Sync> = Arc::new(redraw);
        let pending_jobs = Arc::new(AtomicUsize::new(0));
        let (requests, receiver) = channel();
        let worker_pending = Arc::clone(&pending_jobs);
        thread::Builder::new()
            .name("diff".to_owned())
            .spawn(move || worker(load, receiver, &redraw, &worker_pending))
            .expect("the diff thread starts");
        Self {
            requests,
            pending_jobs,
        }
    }

    /// Starts loading and diffing one file.
    pub fn load(&self, file: File) -> Resource<Arc<DiffContent>> {
        let (resource, out) = Resource::pending();
        self.pending_jobs.fetch_add(1, Ordering::SeqCst);
        self.requests
            .send((file, out))
            .expect("the diff thread is running");
        resource
    }

    /// Loads and diffs `file` again into an existing `resource`.
    ///
    /// Keeps the current `Arc<DiffContent>` untouched when the text on both
    /// sides is unchanged so downstream syntax spans stay valid.
    pub fn refresh(&self, file: File, resource: &Resource<Arc<DiffContent>>) {
        self.pending_jobs.fetch_add(1, Ordering::SeqCst);
        self.requests
            .send((file, resource.writer()))
            .expect("the diff thread is running");
    }

    /// Whether the `"diff"` thread still has work queued or running.
    pub fn is_busy(&self) -> bool {
        self.pending_jobs.load(Ordering::SeqCst) > 0
    }
}

fn worker(
    mut load: impl FnMut(&File) -> Result<DiffContent, String>,
    requests: Receiver<(File, Writer<Arc<DiffContent>>)>,
    redraw: &Arc<dyn Fn() + Send + Sync>,
    pending_jobs: &AtomicUsize,
) {
    for (file, out) in requests {
        out.update(|| load(&file).map(Arc::new), |old, new| same_text(old, new));
        redraw();
        pending_jobs.fetch_sub(1, Ordering::SeqCst);
    }
}

/// Whether two diffs are of the same file with the same text on each side.
fn same_text(old: &DiffContent, new: &DiffContent) -> bool {
    old.file() == new.file()
        && match (old, new) {
            (DiffContent::Diff(old), DiffContent::Diff(new)) => {
                [DiffVersion::Original, DiffVersion::Modified]
                    .into_iter()
                    .all(|side| old.alignment.lines(side) == new.alignment.lines(side))
            }
            (DiffContent::SingleFile(old), DiffContent::SingleFile(new)) => old.lines == new.lines,
            _ => false,
        }
}

fn repository<'a>(
    cached: &'a mut Option<(PathBuf, Repository)>,
    root: &Path,
) -> anyhow::Result<&'a mut Repository> {
    let matches_cached = cached
        .as_ref()
        .is_some_and(|(cached_root, _)| cached_root == root);
    if !matches_cached {
        let opened = Repository::open(root).context("opening a repository")?;
        *cached = Some((root.to_path_buf(), opened));
    }
    Ok(&mut cached.as_mut().expect("just populated").1)
}

fn load_file(
    cached: &mut Option<(PathBuf, Repository)>,
    file: &File,
    settings: Settings,
) -> Result<DiffContent, String> {
    let path = file.path().as_str().to_owned();
    let repo = repository(cached, file.path().root()).map_err(|why| format!("{path}: {why:#}"))?;
    let original = repo
        .get_file_content(file, DiffVersion::Original)
        .context("reading the before side")
        .map_err(|why| format!("{path}: {why:#}"))?;
    let modified = repo
        .get_file_content(file, DiffVersion::Modified)
        .context("reading the after side")
        .map_err(|why| format!("{path}: {why:#}"))?;
    DiffContent::new(file.clone(), &original, &modified, settings).map_err(|why| format!("{why:#}"))
}
