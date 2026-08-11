//! `cargo xtask dev`
//!
//! Builds `codediff`, runs it, and does the same again whenever it exits with
//! [`REBUILD_EXIT_CODE`] — which is what the debug-only F5 key does. The
//! terminal is handed straight to the child, so what runs under this is the
//! program as it normally is.

use anyhow::{Context, Result, bail};
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

/// Agreed with `crates/codediff/src/main.rs`.
const REBUILD_EXIT_CODE: i32 = 42;

pub fn run(args: &[String]) -> Result<()> {
    let root = crate::workspace_root();
    let (review_dir, forwarded) = split_args(args)?;
    let exe = root.join("target").join("debug").join(if cfg!(windows) {
        "codediff.exe"
    } else {
        "codediff"
    });

    loop {
        if !build(&root)? {
            if retry()? {
                continue;
            }
            return Ok(());
        }

        let exit = Command::new(&exe)
            .args(forwarded)
            .current_dir(&review_dir)
            .status()
            .with_context(|| format!("running {}", exe.display()))?;

        match exit.code() {
            Some(REBUILD_EXIT_CODE) => {
                status("Rebuilding", "F5 pressed");
                continue;
            }
            Some(0) => return Ok(()),
            // Become the child: whoever ran us asked about codediff, not about
            // the supervisor.
            Some(code) => std::process::exit(code),
            None => bail!("codediff was killed by a signal"),
        }
    }
}

/// Where to review, and what to hand to `codediff`.
///
/// A first argument naming a directory says where; it is the supervisor's
/// argument, not the child's, so it is not passed on. Everything else is.
fn split_args(args: &[String]) -> Result<(PathBuf, &[String])> {
    let here = || std::env::current_dir().context("finding the current directory");
    match args.first() {
        Some(first) if Path::new(first).is_dir() => Ok((PathBuf::from(first), &args[1..])),
        _ => Ok((here()?, args)),
    }
}

/// Whether `codediff` now exists. Errors are already on stderr.
fn build(root: &Path) -> Result<bool> {
    status("Building", &format!("codediff (debug, {} cores)", jobs()));
    let start = std::time::Instant::now();
    let status_code = Command::new(cargo())
        .args(["build", "-j", &jobs(), "-p", "codediff"])
        .current_dir(root)
        .status()
        .context("running cargo build")?;
    let elapsed = start.elapsed();
    if status_code.success() {
        status(
            "Launching",
            &format!("codediff ({:.1}s)", elapsed.as_secs_f64()),
        );
    } else {
        status(
            "Failed",
            &format!("build after {:.1}s", elapsed.as_secs_f64()),
        );
    }
    Ok(status_code.success())
}

/// Waits for the reader to fix the build. False means they gave up.
fn retry() -> Result<bool> {
    eprint!("Build failed. Enter=retry, q=quit: ");
    std::io::stderr().flush()?;
    let mut line = String::new();
    // End of input is the same answer as `q`, so a piped stdin cannot spin.
    if std::io::stdin().lock().read_line(&mut line)? == 0 {
        eprintln!();
        return Ok(false);
    }
    Ok(!line.trim().eq_ignore_ascii_case("q"))
}

/// Half the machine's cores, which AGENTS.md asks of every cargo command.
fn jobs() -> String {
    let cores = std::thread::available_parallelism().map_or(2, std::num::NonZero::get);
    (cores / 2).max(1).to_string()
}

/// Prints a dev status line: bold cyan, left-aligned, distinct from cargo's green.
fn status(verb: &str, message: &str) {
    eprintln!("\x1b[1m\x1b[36m{verb}\x1b[0m {message}");
}

/// The cargo that started us, so a toolchain override is kept.
fn cargo() -> String {
    std::env::var("CARGO").unwrap_or_else(|_| "cargo".to_owned())
}

#[cfg(test)]
mod tests {
    use super::split_args;

    #[test]
    fn a_leading_directory_says_where_to_review_and_is_not_forwarded() {
        let dir = crate::workspace_root().join("crates");
        let args = vec![dir.to_string_lossy().into_owned(), "--log".to_owned()];
        let (review_dir, forwarded) = split_args(&args).unwrap();
        assert_eq!(review_dir, dir);
        assert_eq!(forwarded, &args[1..]);
    }

    #[test]
    fn anything_else_belongs_to_the_child_and_the_review_is_here() {
        // Cargo runs a unit test from its own crate directory, so this file is
        // where "src/dev.rs" points.
        let args = vec!["src/dev.rs".to_owned()];
        let (review_dir, forwarded) = split_args(&args).unwrap();
        assert_eq!(review_dir, std::env::current_dir().unwrap());
        assert_eq!(forwarded, &args[..]);
    }
}
