//! `codediff` — a standalone, read-only terminal diff reviewer.
//!
//! This crate is the composition root: it parses arguments, loads
//! configuration, constructs concrete backends and wires them together. It is
//! the only place in the workspace that names concrete implementations, and
//! nothing depends on it.

mod cli;
mod debug;
mod doctor;
mod pipeline;
mod text;

use anyhow::{Context, Result, bail};
use clap::Parser;
use ui::Theme;

use cli::{Cli, Command};
use pipeline::Request;

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
        (None, Some(path)) => review(&path, cli.theme.as_deref()),
        // The explorer will go here; until then, say so rather than opening an
        // empty screen.
        (None, None) => {
            use clap::CommandFactory;
            Cli::command().print_help()?;
            println!();
            Ok(())
        }
    }
}

/// Reviews one file: the pipeline builds a diff, `ui` draws it.
fn review(path: &str, theme: Option<&str>) -> Result<()> {
    // Named themes are validated by clap, so an unknown one here would be a
    // bug rather than a mistake by the reader.
    let theme = match theme {
        Some(name) => {
            Theme::named(name).with_context(|| format!("{name} is not a theme; see --help"))?
        }
        None => Theme::from_environment(),
    };

    let runner = pipeline::Runner::new(&Request::Worktree { path })?;
    if runner.is_binary() {
        bail!("{path} is binary — there are no lines to review");
    }

    let mut session = ui::Session::new(runner.run()?, theme);
    ui::run(&mut session).context("running the review interface")
}
