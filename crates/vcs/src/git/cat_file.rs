//! Reads stored Git objects through `git cat-file --batch`.

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
        // `cat-file` expects `rev:path`, with a repository-relative path.
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
        // Do not hide corrupt objects or filter failures as missing files.
        Err(Error::Git { stderr, .. }) if is_missing(&stderr) => Ok(None),
        Err(other) => Err(other),
    }
}

/// Whether Git's error text indicates a missing object.
fn is_missing(stderr: &str) -> bool {
    stderr.contains("does not exist")
        || stderr.contains("Not a valid object name")
        || stderr.contains("unknown revision")
        || stderr.ends_with("missing")
        || stderr.contains("exists on disk, but not in")
}
