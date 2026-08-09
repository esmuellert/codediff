//! `git cat-file --batch` — reading file content out of the object store.
//!
//! Blobs come from one long-lived `git cat-file --batch` child rather than a
//! process per file. Opening a sixty-file diff means a hundred and twenty
//! reads, and at a few milliseconds of spawn each that is most of a second
//! spent on `fork`.
//!
//! The child is stateful — you write a request to its stdin and read the
//! response from its stdout — so it gets its own thread rather than a slot in a
//! pool sized for computation.

use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use crate::error::{Error, Result};
use crate::git::run;
use crate::repo::Repo;
use file_types::RepoPath;

/// A `git cat-file --batch` process, kept open.
#[derive(Debug)]
pub struct Batch {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

impl Batch {
    pub fn open(repo: &Repo) -> Result<Self> {
        let mut child = Command::new("git")
            .arg("--no-optional-locks")
            .args(["cat-file", "--batch"])
            .current_dir(&repo.root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|source| Error::Spawn {
                command: "git cat-file --batch".to_owned(),
                source,
            })?;

        let stdin = child.stdin.take().expect("stdin was piped");
        let stdout = BufReader::new(child.stdout.take().expect("stdout was piped"));
        Ok(Self {
            child,
            stdin,
            stdout,
        })
    }

    /// Reads `path` at `rev`. Returns `Ok(None)` if the object doesn't exist
    /// (file added or deleted relative to that revision).
    pub fn read(&mut self, rev: &str, path: &RepoPath) -> Result<Option<Vec<u8>>> {
        // The `rev:path` spelling is what cat-file expects for a path inside a
        // tree, and the path is relative to the root.
        writeln!(self.stdin, "{rev}:{path}").map_err(Self::broken)?;
        self.stdin.flush().map_err(Self::broken)?;

        let mut header = String::new();
        if self.stdout.read_line(&mut header).map_err(Self::broken)? == 0 {
            return Err(Self::broken(std::io::Error::from(
                std::io::ErrorKind::UnexpectedEof,
            )));
        }
        let header = header.trim_end();

        // "<oid> missing" for anything that does not resolve.
        if header.ends_with(" missing") || header.ends_with(" ambiguous") {
            return Ok(None);
        }

        // "<oid> <type> <size>"
        let size: usize = header
            .rsplit(' ')
            .next()
            .and_then(|n| n.parse().ok())
            .ok_or_else(|| Error::Parse {
                what: format!("cat-file header {header:?}"),
            })?;

        let mut content = vec![0u8; size];
        std::io::Read::read_exact(&mut self.stdout, &mut content).map_err(Self::broken)?;
        // Every object is followed by a newline the caller did not ask for.
        let mut newline = [0u8; 1];
        std::io::Read::read_exact(&mut self.stdout, &mut newline).map_err(Self::broken)?;

        Ok(Some(content))
    }

    fn broken(source: std::io::Error) -> Error {
        Error::Spawn {
            command: "git cat-file --batch".to_owned(),
            source,
        }
    }
}

impl Drop for Batch {
    fn drop(&mut self) {
        // Closing stdin makes cat-file exit; reaping it stops a zombie.
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// Reads a blob through checkout filters (CRLF, smudge).
///
/// Runs `cat-file --filters`. Returns `None` if the object doesn't exist.
/// Not batched — `--batch --filters` reports pre-filter size, which breaks
/// stream framing.
pub fn read_filtered(repo: &Repo, rev: &str, path: &RepoPath) -> Result<Option<Vec<u8>>> {
    let spec = format!("{rev}:{path}");
    match run::run(&repo.root, &["cat-file", "--filters", &spec]) {
        Ok(bytes) => Ok(Some(bytes)),
        // Only match git's "object not found" message. Treating all errors as
        // "missing" would hide real failures (corrupt objects, broken filters).
        Err(Error::Git { stderr, .. }) if is_missing(&stderr) => Ok(None),
        Err(other) => Err(other),
    }
}

/// Whether git's complaint means the object does not exist.
///
/// Matched on the message because `cat-file` exits 128 for everything. The
/// wordings are git's own, and a wording we do not know is treated as a real
/// failure — the safe way round, since the cost is an error the reader can
/// read rather than a diff that quietly lies.
fn is_missing(stderr: &str) -> bool {
    stderr.contains("does not exist")
        || stderr.contains("Not a valid object name")
        || stderr.contains("unknown revision")
        || stderr.ends_with("missing")
        || stderr.contains("exists on disk, but not in")
}
