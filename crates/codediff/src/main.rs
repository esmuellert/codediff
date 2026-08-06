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
use explorer::{ExplorerDiffRequest, ExplorerDiffType};

fn main() -> Result<()> {
    let cli = Cli::parse();

    if cli.self_panic {
        let _screen = ui::Screen::open()?;
        panic!("deliberate panic, to check the terminal is restored");
    }

    let diff_type = cli.diff_type();
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
            let pathspec = path.into_iter().collect();
            explore(diff_type, pathspec, cli.theme.as_deref())
        }
    }
}

/// Reviews everything that changed: the list, and whatever it opens.
fn explore(
    diff_type: ExplorerDiffType,
    pathspec: Vec<String>,
    theme: Option<&str>,
) -> Result<()> {
    let theme = theme_for(theme)?;
    let cwd = std::env::current_dir().context("finding the current directory")?;
    let repo = vcs::Git::open(&cwd).context("opening a repository")?;
    let request = ExplorerDiffRequest::new(repo.repo().root.clone(), diff_type)
        .with_pathspec(pathspec);
    let groups = pipeline::list::run(&request)?;

    // Refused rather than opened. An empty list on a full screen looks like a
    // tool that failed to load, and there is nothing in it to press; the
    // reader wants to be told, and to get their shell back.
    if groups.iter().all(explorer::Group::is_empty) {
        bail!("nothing has changed here — there is nothing to review");
    }

    let mut session = ui::Session::new(ui::Buffer::explorer(groups), theme);
    // The first file is opened before the terminal does, so the reader arrives
    // at a diff rather than at a list they must press a key to use.
    session.open(&mut pipeline::file::open);
    ui::run(&mut session, &mut pipeline::file::open).context("running the review interface")
}

/// The theme named on the command line, or the one the terminal suggests.
///
/// Named themes are validated by clap, so an unknown one here would be a bug
/// rather than a mistake by the reader.
fn theme_for(name: Option<&str>) -> Result<Theme> {
    match name {
        Some(name) => {
            Theme::named(name).with_context(|| format!("{name} is not a theme; see --help"))
        }
        None => Ok(Theme::from_environment()),
    }
}


