//! The background threads and the one entry point to them.
//!
//! Three threads: `files` lists changed files, `diff` reads and diffs one file
//! at a time, `syntax` colours files that have been diffed. Each writes its
//! result into the shared slot it was given, then asks the interface to
//! redraw. A slot nobody holds any more is skipped.

// The slots are spelled out in full rather than hidden behind new names.
#![allow(clippy::type_complexity)]

use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex, Weak};
use std::thread;

use file_types::{DiffVersion, File};
use syntax::{Clues, Engine, Highlighted, Palette, Span};

use crate::diff::{DiffContent, Runner, Settings};
use crate::files::{self, Request};

/// Lines coloured before the interface is told.
const CHUNK_LINES: u32 = 2_000;

/// Lists changed files and diffs and colours them away from the interface.
pub struct DiffService {
    lists: Sender<(Request, Weak<Mutex<Option<Result<Vec<File>, String>>>>)>,
    diffs: Sender<(
        File,
        Weak<Mutex<Option<Result<Arc<DiffContent>, String>>>>,
        Weak<Mutex<[Vec<Vec<Span>>; 2]>>,
    )>,
    /// Jobs queued or running on any thread.
    pending_jobs: Arc<AtomicUsize>,
}

impl DiffService {
    /// Starts the threads on a Git repository.
    pub fn start(redraw: impl Fn() + Send + Sync + 'static, settings: Settings) -> Self {
        Self::new(
            redraw,
            |request| files::get_files(request).map_err(|why| format!("{why:#}")),
            move |file| compare(file, settings),
        )
    }

    /// Starts the threads with the given ways to list and read files.
    pub fn new(
        redraw: impl Fn() + Send + Sync + 'static,
        list: impl FnMut(&Request) -> Result<Vec<File>, String> + Send + 'static,
        read: impl FnMut(&File) -> Result<DiffContent, String> + Send + 'static,
    ) -> Self {
        let redraw: Arc<dyn Fn() + Send + Sync> = Arc::new(redraw);
        let pending_jobs = Arc::new(AtomicUsize::new(0));
        let (lists, list_requests) = channel();
        let (diffs, diff_requests) = channel();
        let (colours, colour_requests) = channel();

        let (redraw_after_list, pending_after_list) =
            (Arc::clone(&redraw), Arc::clone(&pending_jobs));
        thread::Builder::new()
            .name("files".to_owned())
            .spawn(move || list_files(list, list_requests, &redraw_after_list, &pending_after_list))
            .expect("the files thread starts");

        let (redraw_after_diff, pending_after_diff) =
            (Arc::clone(&redraw), Arc::clone(&pending_jobs));
        thread::Builder::new()
            .name("diff".to_owned())
            .spawn(move || {
                diff_files(
                    read,
                    diff_requests,
                    &colours,
                    &redraw_after_diff,
                    &pending_after_diff,
                )
            })
            .expect("the diff thread starts");

        let pending_after_colour = Arc::clone(&pending_jobs);
        thread::Builder::new()
            .name("syntax".to_owned())
            .spawn(move || colour_files(colour_requests, &redraw, &pending_after_colour))
            .expect("the syntax thread starts");

        Self {
            lists,
            diffs,
            pending_jobs,
        }
    }

    /// Starts listing the uncommitted changes under `repo`. The slot holds
    /// `None` until they are listed.
    pub fn list_files(&self, repo: &Path) -> Arc<Mutex<Option<Result<Vec<File>, String>>>> {
        let files = Arc::new(Mutex::new(None));
        let request = Request::worktree(repo);
        self.pending_jobs.fetch_add(1, Ordering::SeqCst);
        self.lists
            .send((request, Arc::downgrade(&files)))
            .expect("the files thread is running");
        files
    }

    /// Starts reading, diffing and colouring one file.
    ///
    /// The first slot holds `None` until the file is diffed. The second holds
    /// the syntax spans of each line, original side first, as they arrive.
    pub fn open(
        &self,
        file: File,
    ) -> (
        Arc<Mutex<Option<Result<Arc<DiffContent>, String>>>>,
        Arc<Mutex<[Vec<Vec<Span>>; 2]>>,
    ) {
        let diff = Arc::new(Mutex::new(None));
        let colours = Arc::new(Mutex::new([Vec::new(), Vec::new()]));
        self.refresh(file, &diff, &colours);
        (diff, colours)
    }

    /// Reads, diffs and colours `file` again, into the diff and colours that
    /// `open` returned for it.
    ///
    /// They keep what they hold until the new result is ready, so the file
    /// never looks missing. If the text is the same as before, they keep it.
    pub fn refresh(
        &self,
        file: File,
        diff: &Arc<Mutex<Option<Result<Arc<DiffContent>, String>>>>,
        colours: &Arc<Mutex<[Vec<Vec<Span>>; 2]>>,
    ) {
        self.pending_jobs.fetch_add(1, Ordering::SeqCst);
        self.diffs
            .send((file, Arc::downgrade(diff), Arc::downgrade(colours)))
            .expect("the diff thread is running");
    }

    /// Whether any thread still has work queued or running.
    pub fn is_busy(&self) -> bool {
        self.pending_jobs.load(Ordering::SeqCst) > 0
    }
}

fn list_files(
    mut list: impl FnMut(&Request) -> Result<Vec<File>, String>,
    requests: Receiver<(Request, Weak<Mutex<Option<Result<Vec<File>, String>>>>)>,
    redraw: &Arc<dyn Fn() + Send + Sync>,
    pending_jobs: &AtomicUsize,
) {
    for (request, files) in requests {
        if files.strong_count() > 0 {
            let result = list(&request);
            if let Some(files) = files.upgrade() {
                *files.lock().unwrap() = Some(result);
            }
        }
        finish(redraw, pending_jobs);
    }
}

fn diff_files(
    mut read: impl FnMut(&File) -> Result<DiffContent, String>,
    requests: Receiver<(
        File,
        Weak<Mutex<Option<Result<Arc<DiffContent>, String>>>>,
        Weak<Mutex<[Vec<Vec<Span>>; 2]>>,
    )>,
    to_colour: &Sender<(
        Arc<DiffContent>,
        Weak<Mutex<Option<Result<Arc<DiffContent>, String>>>>,
        Weak<Mutex<[Vec<Vec<Span>>; 2]>>,
    )>,
    redraw: &Arc<dyn Fn() + Send + Sync>,
    pending_jobs: &AtomicUsize,
) {
    for (file, diff, colours) in requests {
        // Nobody holds the file any more, so nobody would see the result.
        if diff.strong_count() > 0 {
            let result = read(&file).map(Arc::new);
            if let (Some(diff), Some(colours)) = (diff.upgrade(), colours.upgrade()) {
                replace_diff(result, &diff, &colours, to_colour, pending_jobs);
            }
        }
        finish(redraw, pending_jobs);
    }
}

/// Puts `result` in place of the file's diff and starts colouring it, unless
/// the text is the same as the diff already holds.
fn replace_diff(
    result: Result<Arc<DiffContent>, String>,
    diff: &Arc<Mutex<Option<Result<Arc<DiffContent>, String>>>>,
    colours: &Arc<Mutex<[Vec<Vec<Span>>; 2]>>,
    to_colour: &Sender<(
        Arc<DiffContent>,
        Weak<Mutex<Option<Result<Arc<DiffContent>, String>>>>,
        Weak<Mutex<[Vec<Vec<Span>>; 2]>>,
    )>,
    pending_jobs: &AtomicUsize,
) {
    // The diff is locked before the colours, here and in the syntax thread.
    let mut current = diff.lock().unwrap();
    if let (Some(Ok(old)), Ok(new)) = (current.as_ref(), &result)
        && same_text(old, new)
    {
        return;
    }
    // The old colours belong to the old text.
    *colours.lock().unwrap() = [Vec::new(), Vec::new()];
    if let Ok(content) = &result {
        pending_jobs.fetch_add(1, Ordering::SeqCst);
        to_colour
            .send((
                Arc::clone(content),
                Arc::downgrade(diff),
                Arc::downgrade(colours),
            ))
            .expect("the syntax thread is running");
    }
    *current = Some(result);
}

/// Whether two diffs are of the same file with the same text on each side.
/// The rest of a diff follows from its text.
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

fn colour_files(
    requests: Receiver<(
        Arc<DiffContent>,
        Weak<Mutex<Option<Result<Arc<DiffContent>, String>>>>,
        Weak<Mutex<[Vec<Vec<Span>>; 2]>>,
    )>,
    redraw: &Arc<dyn Fn() + Send + Sync>,
    pending_jobs: &AtomicUsize,
) {
    let engine = Engine::new();
    let palette = Palette::new();
    for (content, diff, colours) in requests {
        for side in [DiffVersion::Original, DiffVersion::Modified] {
            let text = match content.as_ref() {
                DiffContent::Diff(diff) => diff.alignment.text(side),
                DiffContent::SingleFile(single) if single.side() == side => {
                    Arc::clone(&single.lines)
                }
                DiffContent::SingleFile(_) => continue,
            };
            let Some(path) = content.file().path_of_version(side) else {
                continue;
            };
            colour(&engine, &palette, path.as_str(), &text, |lines| {
                let (Some(diff), Some(colours)) = (diff.upgrade(), colours.upgrade()) else {
                    return false;
                };
                // Stop when a refresh has replaced the text being coloured.
                let current = diff.lock().unwrap();
                if !matches!(current.as_ref(), Some(Ok(current_content)) if Arc::ptr_eq(current_content, &content))
                {
                    return false;
                }
                colours.lock().unwrap()[side as usize].extend(lines);
                drop(current);
                redraw();
                true
            });
        }
        finish(redraw, pending_jobs);
    }
}

/// Asks for a redraw, then counts the job as done. In this order, a caller
/// that finds nothing busy has already been sent every redraw.
fn finish(redraw: &Arc<dyn Fn() + Send + Sync>, pending_jobs: &AtomicUsize) {
    redraw();
    pending_jobs.fetch_sub(1, Ordering::SeqCst);
}

/// Colours a file from its first line, handing over each piece in order.
/// Stops when `deliver` returns false.
fn colour(
    engine: &Engine,
    palette: &Palette,
    path: &str,
    text: &[String],
    mut deliver: impl FnMut(Vec<Vec<Span>>) -> bool,
) {
    let Some(last_line) = (text.len() as u32).checked_sub(1) else {
        return;
    };
    let clues = Clues::new(path, text.first().map(String::as_str));
    let Some(grammar) = engine.find(clues, text.len()) else {
        return;
    };
    let mut reading = Highlighted::new(engine, grammar, palette, text);
    while !reading.finished() {
        let from = reading.get_lines_coloured();
        let to = from.saturating_add(CHUNK_LINES - 1).min(last_line);
        let mut lines = Vec::new();
        reading.read_colours_to_line(engine, palette, to, text, &mut lines);
        if lines.is_empty() || !deliver(lines) {
            return;
        }
    }
}

/// Runs the four stages without caching between calls.
fn compare(file: &File, settings: Settings) -> Result<DiffContent, String> {
    let path = file.path().as_str().to_owned();
    let runner = Runner::new(file, settings).map_err(|why| format!("{path}: {why:#}"))?;
    if runner.is_binary() {
        return Err(format!("{path} is binary — there are no lines to review"));
    }
    runner
        .compute_diff()
        .map_err(|why| format!("{path}: {why:#}"))
}
