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
        let _screen = loom::Screen::open()?;
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
            ui::main(&cwd, cli.path.into_iter().collect())?;
            Ok(ExitCode::SUCCESS)
        }
    }
}
