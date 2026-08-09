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

    match cli.command {
        Some(Command::Doctor) => {
            doctor::run();
            Ok(())
        }
        Some(Command::Debug(command)) => debug::run(command),
        None => {
            let cwd = std::env::current_dir().context("finding the current directory")?;
            ui::start(cwd, cli.path.into_iter().collect(), None)
        }
    }
}
