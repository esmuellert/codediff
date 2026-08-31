//! Streams repository invalidations as JSON Lines for editor integrations.

use std::env;
use std::ffi::OsStr;
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc;

use anyhow::{Context, bail};
use serde::Serialize;
use watcher::Refresh;

const PROTOCOL_VERSION: u8 = 1;

enum Command {
    Version,
    Watch(PathBuf),
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Message {
    Ready {
        protocol: u8,
        binary_version: &'static str,
    },
    Refresh {
        worktree: bool,
        index: bool,
        head: bool,
        refs: bool,
    },
}

impl From<Refresh> for Message {
    fn from(refresh: Refresh) -> Self {
        Self::Refresh {
            worktree: refresh.worktree,
            index: refresh.index,
            head: refresh.head,
            refs: refresh.refs,
        }
    }
}

fn main() -> anyhow::Result<()> {
    match parse_arguments()? {
        Command::Version => {
            let mut stdout = io::stdout().lock();
            writeln!(stdout, "codediff-watcher {}", env!("CARGO_PKG_VERSION"))?;
            Ok(())
        }
        Command::Watch(repository) => run(&repository),
    }
}

fn parse_arguments() -> anyhow::Result<Command> {
    let mut arguments = env::args_os().skip(1);
    let Some(first) = arguments.next() else {
        bail!("usage: codediff-watcher <repository>");
    };
    if arguments.next().is_some() {
        bail!("usage: codediff-watcher <repository>");
    }
    if first == OsStr::new("--version") {
        Ok(Command::Version)
    } else {
        Ok(Command::Watch(PathBuf::from(first)))
    }
}

fn run(repository: &Path) -> anyhow::Result<()> {
    let (refresh_sender, refresh_receiver) = mpsc::channel();
    let emitter = channel::Emitter::new(refresh_sender, std::convert::identity);
    let _subscription = watcher::subscribe(repository, emitter)
        .with_context(|| format!("failed to watch {}", repository.display()))?;

    let mut stdout = BufWriter::new(io::stdout().lock());
    write_message(
        &mut stdout,
        &Message::Ready {
            protocol: PROTOCOL_VERSION,
            binary_version: env!("CARGO_PKG_VERSION"),
        },
    )?;

    while let Ok(refresh) = refresh_receiver.recv() {
        write_message(&mut stdout, &refresh.into())?;
    }
    bail!("watcher event worker stopped")
}

fn write_message(output: &mut impl Write, message: &Message) -> anyhow::Result<()> {
    serde_json::to_writer(&mut *output, message).context("failed to encode protocol message")?;
    output
        .write_all(b"\n")
        .and_then(|()| output.flush())
        .context("failed to write protocol message")
}
