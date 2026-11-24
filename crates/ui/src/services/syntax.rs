//! Highlights diffed files in chunks on one background thread.

#![allow(clippy::type_complexity)]

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex, Weak};
use std::thread;

use diff::DiffContent;
use file_types::DiffVersion;
use syntax::{Clues, Engine, Highlighted, Palette, Span};

/// Lines highlighted before the interface is told.
const CHUNK_LINES: u32 = 2_000;

/// Highlights [`DiffContent`] lines on one `"syntax"` thread.
pub struct SyntaxService {
    requests: Sender<(Arc<DiffContent>, Weak<Mutex<[Vec<Vec<Span>>; 2]>>)>,
    pending_jobs: Arc<AtomicUsize>,
}

impl SyntaxService {
    /// Starts the `"syntax"` worker thread.
    pub fn start(redraw: impl Fn() + Send + Sync + 'static) -> Self {
        let redraw: Arc<dyn Fn() + Send + Sync> = Arc::new(redraw);
        let pending_jobs = Arc::new(AtomicUsize::new(0));
        let (requests, receiver) = channel();
        let worker_pending = Arc::clone(&pending_jobs);
        thread::Builder::new()
            .name("syntax".to_owned())
            .spawn(move || worker(receiver, &redraw, &worker_pending))
            .expect("the syntax thread starts");
        Self {
            requests,
            pending_jobs,
        }
    }

    /// Starts highlighting `content`, returning the shared span slot filled in chunks.
    pub fn highlight(&self, content: Arc<DiffContent>) -> Arc<Mutex<[Vec<Vec<Span>>; 2]>> {
        let spans = Arc::new(Mutex::new([Vec::new(), Vec::new()]));
        self.pending_jobs.fetch_add(1, Ordering::SeqCst);
        self.requests
            .send((content, Arc::downgrade(&spans)))
            .expect("the syntax thread is running");
        spans
    }

    /// Whether the `"syntax"` thread still has work queued or running.
    pub fn is_busy(&self) -> bool {
        self.pending_jobs.load(Ordering::SeqCst) > 0
    }
}

fn worker(
    requests: Receiver<(Arc<DiffContent>, Weak<Mutex<[Vec<Vec<Span>>; 2]>>)>,
    redraw: &Arc<dyn Fn() + Send + Sync>,
    pending_jobs: &AtomicUsize,
) {
    let engine = Engine::new();
    let palette = Palette::new();
    for (content, spans) in requests {
        if spans.strong_count() > 0 {
            for side in [DiffVersion::Original, DiffVersion::Modified] {
                if spans.strong_count() == 0 {
                    break;
                }
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
                highlight_side(&engine, &palette, path.as_str(), &text, |lines| {
                    let Some(spans) = spans.upgrade() else {
                        return false;
                    };
                    spans.lock().unwrap()[side as usize].extend(lines);
                    redraw();
                    true
                });
            }
        }
        redraw();
        pending_jobs.fetch_sub(1, Ordering::SeqCst);
    }
}

fn highlight_side(
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
