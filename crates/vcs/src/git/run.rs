//! Runs Git commands and returns their stdout or a typed error.

use std::path::Path;
use std::process::{Command, Stdio};

use crate::error::{Error, Result};

/// Runs `git` in `cwd` and returns stdout.
///
/// Read-only commands use `--no-optional-locks` to avoid refreshing the index.
pub fn run(cwd: &Path, args: &[&str]) -> Result<Vec<u8>> {
    let output = Command::new("git")
        .arg("--no-optional-locks")
        .args(args)
        .current_dir(cwd)
        // A pager or a credential prompt would hang us forever.
        .env("GIT_PAGER", "cat")
        .env("GIT_TERMINAL_PROMPT", "0")
        .stdin(Stdio::null())
        .output()
        .map_err(|source| Error::Spawn {
            command: describe(args),
            source,
        })?;

    if !output.status.success() {
        return Err(Error::Git {
            command: describe(args),
            code: output.status.code(),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        });
    }
    Ok(output.stdout)
}

/// The same, for commands whose output is a single line of text.
pub fn run_line(cwd: &Path, args: &[&str]) -> Result<String> {
    let out = run(cwd, args)?;
    let text = String::from_utf8(out).map_err(|_| Error::NotUtf8 {
        command: describe(args),
    })?;
    Ok(text.trim_end_matches(['\n', '\r']).to_owned())
}

fn describe(args: &[&str]) -> String {
    format!("git {}", args.join(" "))
}
