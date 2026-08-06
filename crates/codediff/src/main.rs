//! `codediff` — a standalone, read-only terminal diff reviewer.
//!
//! This crate is the composition root: it parses arguments, loads
//! configuration, constructs concrete backends and wires them together. It is
//! the only place in the workspace that names concrete implementations, and
//! nothing depends on it.

mod cli;
mod debug;
mod doctor;
mod text;

use anyhow::{Context, Result};
use clap::Parser;

use cli::{Cli, Command};

fn main() -> Result<()> {
    let cli = Cli::parse();

    if cli.self_panic {
        let _screen = ui::Screen::open()?;
        panic!("deliberate panic, to check the terminal is restored");
    }

    match (cli.command, cli.path) {
        (Some(Command::Doctor), _) => {
            doctor::run();
            Ok(())
        }
        (Some(Command::Debug(command)), _) => debug::run(command),
        // A path is a **pathspec**, not a different mode: `codediff a.rs` is
        // `codediff` narrowed to one file. One code path, so a file reached by
        // naming it and the same file reached by pressing enter on its row are
        // the same comparison — which they were not. See D58.
        (None, path) => {
            let cwd = std::env::current_dir().context("finding the current directory")?;
            ui::start(cwd, path.into_iter().collect(), cli.theme.as_deref())
        }
    }
}
