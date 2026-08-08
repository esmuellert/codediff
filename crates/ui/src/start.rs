//! Opening a review, and everything it needs before the first frame.
//!
//! Runs once before the terminal opens, so it may block (unlike the loop in
//! [`app`](crate::app)).

use std::path::PathBuf;

use anyhow::{Context, Result, bail};

use crate::app::{Session, run};
use crate::theme::Theme;
use crate::view::Buffer;

/// Reviews everything that changed under `cwd`, until the reader quits.
pub fn start(cwd: PathBuf, pathspec: Vec<String>, theme: Option<&str>) -> Result<()> {
    let theme = theme_for(theme)?;
    let request = pipeline::list::Request::worktree(cwd).with_pathspec(pathspec);
    let files = pipeline::list::run(&request)?;

    if files.is_empty() {
        bail!("nothing has changed here — there is nothing to review");
    }

    let mut session = Session::new(Buffer::explorer(files), theme);
    session.open();
    run(&mut session).context("running the review interface")
}

/// An unknown name is an error; `None` picks from the terminal's capabilities.
fn theme_for(name: Option<&str>) -> Result<Theme> {
    match name {
        Some(name) => {
            Theme::named(name).with_context(|| format!("{name} is not a theme; see --help"))
        }
        None => Ok(Theme::from_environment()),
    }
}
