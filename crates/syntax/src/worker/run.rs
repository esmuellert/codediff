//! The syntax worker thread and its resumable cache.

use channel::Emitter;
use std::sync::OnceLock;
use std::sync::mpsc::Receiver;

use crate::{Clues, Engine, Highlighted, Palette};

use super::message::{SyntaxRequest, SyntaxResponse};

/// Maximum lines returned in one response.
const CHUNK: u32 = 2_000;

/// Maximum unfinished readings retained.
const MEMOS: usize = 4;

/// Where a file was left off.
struct Memo {
    key: String,
    version: super::message::Version,
    /// Lines already returned to the caller.
    reached: u32,
    reading: Highlighted,
}

/// Answers requests until the asker goes away.
pub fn run(requests: &Receiver<SyntaxRequest>, answers: &Emitter<SyntaxResponse>) {
    let mut memos: Vec<Memo> = Vec::new();
    while let Ok(request) = requests.recv() {
        respond(&request, answers, &mut memos);
    }
}

fn respond(request: &SyntaxRequest, answers: &Emitter<SyntaxResponse>, memos: &mut Vec<Memo>) {
    tracing::info!(path = %request.key, from = request.have, "colouring");
    let lines = request.text.len() as u32;
    let Some(mut reading) = resume(request, memos) else {
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
            .read_colours_to_line(engine(), palette(), chunk, &request.text, &mut spans);

        let got = reading.reached + spans.len() as u32;
        if got <= sent {
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
        if !answers.send(SyntaxResponse {
            key: request.key.clone(),
            version: request.version,
            from: sent,
            spans: sending,
            more,
        }) {
            return;
        }
        reading.reached = got;
        sent = got;
        answered = true;
    }

    if !answered {
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

/// Resumes a matching memo or starts the file from the top.
fn resume(request: &SyntaxRequest, memos: &mut Vec<Memo>) -> Option<Memo> {
    if let Some(position) = memos.iter().position(|memo| {
        memo.key == request.key && memo.version == request.version && memo.reached == request.have
    }) {
        return Some(memos.remove(position));
    }
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

/// Retains unfinished engine state.
fn remember(memo: Memo, memos: &mut Vec<Memo>) {
    if memo.reading.finished() {
        return;
    }
    memos.push(memo);
    if memos.len() > MEMOS {
        memos.remove(0);
    }
}

/// The shared syntax engine.
fn engine() -> &'static Engine {
    static ENGINE: OnceLock<Engine> = OnceLock::new();
    ENGINE.get_or_init(Engine::new)
}

/// The shared engine palette.
fn palette() -> &'static Palette {
    static PALETTE: OnceLock<Palette> = OnceLock::new();
    PALETTE.get_or_init(Palette::new)
}
