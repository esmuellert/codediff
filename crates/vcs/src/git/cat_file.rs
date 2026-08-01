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

use crate::change::{RelPath, Repo};
use crate::error::{Error, Result};

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

    /// Reads `path` as it exists at `rev`.
    ///
    /// Returns `Ok(None)` when the object does not exist — a file added since
    /// the revision, or deleted before it. That is an ordinary answer, not an
    /// error: a diff against `HEAD` asks for both sides of every file and one
    /// of them is routinely absent.
    pub fn read(&mut self, rev: &str, path: &RelPath) -> Result<Option<Vec<u8>>> {
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
