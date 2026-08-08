//! The syntax worker thread.
//!
//! No file is read here, no repository consulted, no character drawn. Text
//! arrives, spans leave.
//!
//! Every request can be answered cold. The worker remembers where it got
//! to in files it has not finished, but only as a shortcut: delete that memory
//! and every answer is identical, only slower. That is the difference between
//! a cache and a session, and it is why the asker may evict whatever it likes
//! without telling anyone.
//!
//! Only one engine can be resumed. The matcher stops where it is asked and
//! carries on later; the parser has no range API and reads a whole file
//! however little was wanted. A memo for a parsed file is therefore never
//! made, because there is never anything left over.

use std::sync::OnceLock;
use std::sync::mpsc::{Receiver, Sender};

use syntax::{Clues, Engine, Highlighted, Palette};

use super::message::{SyntaxRequest, SyntaxResponse};

/// Lines sent back at a time.
///
/// The only reason this is not "everything wanted at once" is that a very long
/// file would otherwise stay plain until all of it was done — sixteen seconds
/// for three hundred thousand lines with the slower engine. Two thousand lines
/// is a hundred milliseconds of that engine's work and five of the other's, so
/// a reader sees colour arrive promptly and in order.
const CHUNK: u32 = 2_000;

/// Unfinished readings kept in case they are wanted again.
///
/// Four, because the memory is only worth having for a file part-read, and a
/// reader moves between a handful of files before coming back. Each holds a
/// grammar's context stack, so this is not free, and there is nothing to be
/// gained from keeping the one for a file abandoned an hour ago.
const MEMOS: usize = 4;

/// Where a file was left off.
struct Memo {
    key: String,
    version: super::message::Version,
    /// What the asker held when this was made. A memo is only usable by
    /// someone who still has everything read so far; anyone else needs the
    /// lines this skipped over.
    reached: u32,
    reading: Highlighted,
}

/// Answers requests until the asker goes away.
pub fn run(requests: &Receiver<SyntaxRequest>, answers: &Sender<SyntaxResponse>) {
    // Blocks. A worker with nothing to do costs nothing at all — no timer, no
    // spin, no wake-ups — which is its ordinary state.
    let mut memos: Vec<Memo> = Vec::new();
    while let Ok(request) = requests.recv() {
        respond(&request, answers, &mut memos);
    }
}

fn respond(request: &SyntaxRequest, answers: &Sender<SyntaxResponse>, memos: &mut Vec<Memo>) {
    let lines = request.text.len() as u32;
    let Some(mut reading) = resume(request, memos) else {
        // Nothing claims this language. Answering once with no spans is how
        // the asker learns there is nothing coming.
        let _ = answers.send(SyntaxResponse {
            key: request.key.clone(),
            version: request.version,
            from: request.have,
            spans: vec![Vec::new(); lines.saturating_sub(request.have) as usize],
            more: false,
        });
        return;
    };

    let mut sent = reading.reached;
    let target = request.last.min(lines.saturating_sub(1));
    let mut spans = Vec::new();
    let mut answered = false;
    while sent <= target && sent < lines {
        let chunk = (sent + CHUNK).min(target);
        reading
            .reading
            .reach(engine(), palette(), chunk, &request.text, &mut spans);

        // What the engine actually reached, which for a parser is the whole
        // file however little was asked for.
        let got = reading.reached + spans.len() as u32;
        if got <= sent {
            // It cannot get any further, so neither can we. Fill the rest
            // plainly rather than leave the asker waiting for ever.
            let _ = answers.send(SyntaxResponse {
                key: request.key.clone(),
                version: request.version,
                from: sent,
                spans: vec![Vec::new(); (lines - sent) as usize],
                more: false,
            });
            return;
        }

        let more = got <= target && got < lines;
        let sending = std::mem::take(&mut spans);
        if answers
            .send(SyntaxResponse {
                key: request.key.clone(),
                version: request.version,
                from: sent,
                spans: sending,
                more,
            })
            .is_err()
        {
            // Nobody is listening any more, which means the review has ended.
            return;
        }
        reading.reached = got;
        sent = got;
        answered = true;
    }

    if !answered {
        // Nothing to read: an empty file, or one already read past what was
        // asked for. Every request must be answered, because the asker
        // holds a request for this file back until this one finishes — so
        // silence here would stop that file being coloured for good.
        let _ = answers.send(SyntaxResponse {
            key: request.key.clone(),
            version: request.version,
            from: sent,
            spans: Vec::new(),
            more: false,
        });
    }

    remember(reading, memos);
}

/// The reading to continue, whether remembered or begun.
///
/// `None` when nothing claims the language, which is not an error: plenty of
/// files in a review are data, and they draw plainly.
fn resume(request: &SyntaxRequest, memos: &mut Vec<Memo>) -> Option<Memo> {
    if let Some(position) = memos.iter().position(|memo| {
        memo.key == request.key && memo.version == request.version && memo.reached == request.have
    }) {
        return Some(memos.remove(position));
    }
    // Either this file was never started, or the asker has thrown away part of
    // what was read — eviction does that — so a bookmark further down answers
    // a question nobody asked. Begin again.
    memos.retain(|memo| memo.key != request.key);

    let clues = Clues::new(&request.path, request.text.first().map(String::as_str));
    let grammar = engine().find(clues, request.text.len())?;
    Some(Memo {
        key: request.key.clone(),
        version: request.version,
        reached: 0,
        reading: Highlighted::new(engine(), grammar, palette(), &request.text),
    })
}

/// Keeps a reading in case the file is scrolled further.
///
/// A finished file is not remembered: there is nothing left to carry, and the
/// asker has every line already.
fn remember(memo: Memo, memos: &mut Vec<Memo>) {
    if memo.reading.finished() {
        return;
    }
    memos.push(memo);
    if memos.len() > MEMOS {
        memos.remove(0);
    }
}

/// Every grammar, unpacked once.
fn engine() -> &'static Engine {
    static ENGINE: OnceLock<Engine> = OnceLock::new();
    ENGINE.get_or_init(Engine::new)
}

/// Both halves of the vocabulary, compiled once.
///
/// Nothing here depends on the theme, because a span names a pen rather than a
/// colour — so changing theme invalidates nothing and re-reads nothing. The
/// tables themselves are the engines' own words, so they live in `syntax`
/// rather than being handed in from a theme that has no opinion about them.
fn palette() -> &'static Palette {
    static PALETTE: OnceLock<Palette> = OnceLock::new();
    PALETTE.get_or_init(Palette::new)
}
