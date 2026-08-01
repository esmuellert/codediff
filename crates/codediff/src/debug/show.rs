//! `codediff debug show <rev>:<path>` — a file's content at a revision.
//!
//! The check is `git show <rev>:<path> | cmp -` : byte for byte, or the diff
//! that follows is of something other than what is in the repository.

use anyhow::{Context, Result, bail};
use vcs::{Git, RelPath};

use crate::text::visible;

pub fn run(spec: &str, raw: bool) -> Result<()> {
    let (rev, path) = spec
        .split_once(':')
        .with_context(|| format!("expected <rev>:<path>, got {spec:?}"))?;

    let cwd = std::env::current_dir().context("finding the current directory")?;
    let mut git = Git::open(&cwd).context("opening a repository")?;

    let bytes = git
        .cat_file(rev, &RelPath::new(path))
        .with_context(|| format!("reading {spec}"))?;

    let Some(bytes) = bytes else {
        bail!("{path} does not exist at {rev}");
    };

    // For comparing against `git show` with `cmp`: exactly the bytes, nothing
    // else on stdout.
    if raw {
        use std::io::Write;
        std::io::stdout().write_all(&bytes)?;
        return Ok(());
    }

    let size = bytes.len();
    // Classified here rather than through the trait: this command names an
    // arbitrary revision, which is not one of the two sides of a change.
    let text = String::from_utf8(bytes).ok();
    println!("{} at {}", visible(path), visible(rev));
    println!("{size} byte(s)");
    println!();

    match text.as_deref().filter(|t| !t.contains('\0')) {
        Some(text) => {
            for (number, line) in text.split('\n').enumerate() {
                // Control characters and bidi overrides are rendered as
                // pictures: a file being reviewed must not be able to steer the
                // terminal showing it.
                println!("{:>5} │ {}", number + 1, visible(line));
            }
        }
        None => println!("(binary — pass --raw to write the bytes to stdout)"),
    }
    Ok(())
}
