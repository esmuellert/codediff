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
use std::process::ExitCode;

use cli::{Cli, Command};

/// What we exit with when the reader asks for a rebuild; `cargo xtask dev`
/// reads it and starts us again. Only a debug build can produce it — the key
/// that asks for a rebuild is not bound in a release one.
const REBUILD_EXIT_CODE: u8 = 42;

fn main() -> Result<ExitCode> {
    let cli = Cli::parse();

    if let Some(log_path) = &cli.log {
        let file = std::fs::File::create(log_path)
            .with_context(|| format!("opening log file {}", log_path.display()))?;
        tracing_subscriber::fmt()
            .with_writer(file)
            .with_ansi(false)
            .init();
    }

    if cli.self_panic {
        let _screen = ui::Screen::open()?;
        panic!("deliberate panic, to check the terminal is restored");
    }

    match cli.command {
        Some(Command::Doctor) => {
            doctor::run();
            Ok(ExitCode::SUCCESS)
        }
        Some(Command::Debug(command)) => {
            debug::run(command)?;
            Ok(ExitCode::SUCCESS)
        }
        None => {
            let cwd = std::env::current_dir().context("finding the current directory")?;
            let outcome = ui::start(cwd, cli.path.into_iter().collect(), None)?;
            Ok(match outcome {
                ui::Exit::Quit => ExitCode::SUCCESS,
                ui::Exit::Rebuild => ExitCode::from(REBUILD_EXIT_CODE),
            })
        }
    }
}
