//! Opening a review, and everything it needs before the first frame.
//!
//! ---
//!
//! The entry point. Everything the interface owns is started from here: the
//! file list, the buffers holding it, the workers behind them, and the loop
//! that draws. A caller supplies where to look and what to draw with, and gets
//! control back when the reader quits.
//!
//! **It is a file of its own so that [`app`] holds only the loop.** The two
//! obey opposite rules: this runs once, before the terminal is opened, so it
//! may block — there is nothing to stay responsive with, which is why the file
//! list is read here rather than asked for. The loop may not block at all. A
//! rule cannot say that about a file holding both. See D63 and D64.
//!
//! [`app`]: crate::app

use std::path::PathBuf;

use anyhow::{Context, Result, bail};

use crate::app::{Session, run};
use crate::theme::Theme;
use crate::view::Buffer;

/// Reviews everything that changed under `cwd`, until the reader quits.
///
/// `cwd` is where to start looking, not the repository: git discovers the root
/// from it, and everything below is named relative to that. `pathspec` narrows
/// the list, empty being everything.
pub fn start(cwd: PathBuf, pathspec: Vec<String>, theme: Option<&str>) -> Result<()> {
    let theme = theme_for(theme)?;
    // The working tree, always. What to compare against is a decision the
    // reader makes inside the review, not one they spell in git's revision
    // syntax before it opens. See D62.
    let request = pipeline::list::Request::worktree(cwd).with_pathspec(pathspec);
    let files = pipeline::list::run(&request)?;

    // Refused rather than opened. An empty list on a full screen looks like a
    // tool that failed to load, and there is nothing in it to press; the
    // reader wants to be told, and to get their shell back.
    if files.is_empty() {
        bail!("nothing has changed here — there is nothing to review");
    }

    let mut session = Session::new(Buffer::explorer(files), theme);
    // The first file is asked for before the terminal opens, so it is already
    // being compared while the screen is set up. It arrives a frame or two
    // after the list rather than before it: a comparison runs on a thread of
    // its own now, and the list is usable while it does.
    session.open();
    run(&mut session).context("running the review interface")
}

/// The theme by name, or the one the terminal suggests.
///
/// An unknown name is a mistake worth a sentence: the command line validates
/// the ones it offers, but nothing else that calls this does.
fn theme_for(name: Option<&str>) -> Result<Theme> {
    match name {
        Some(name) => {
            Theme::named(name).with_context(|| format!("{name} is not a theme; see --help"))
        }
        None => Ok(Theme::from_environment()),
    }
}
