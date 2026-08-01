//! What can go wrong talking to git.

use std::path::PathBuf;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
    /// git could not be started at all — usually not installed, or not on PATH.
    Spawn {
        command: String,
        source: std::io::Error,
    },
    /// git ran and refused.
    Git {
        command: String,
        code: Option<i32>,
        stderr: String,
    },
    /// The path is not inside a repository.
    NoRepository {
        path: PathBuf,
    },
    UnknownRevision {
        rev: String,
    },
    /// Git's output was not in the shape the format promises.
    Parse {
        what: String,
    },
    /// A path or output that is not UTF-8. Paths are bytes on Unix, and one we
    /// cannot decode is one we could neither display nor hand back to git.
    NotUtf8 {
        command: String,
    },
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Spawn { command, source } => {
                write!(f, "could not run `{command}`: {source}")
            }
            Error::Git {
                command,
                code,
                stderr,
            } => {
                write!(f, "`{command}` failed")?;
                if let Some(code) = code {
                    write!(f, " with status {code}")?;
                }
                if !stderr.is_empty() {
                    write!(f, ": {stderr}")?;
                }
                Ok(())
            }
            Error::NoRepository { path } => {
                write!(f, "{} is not inside a git repository", path.display())
            }
            Error::UnknownRevision { rev } => write!(f, "unknown revision `{rev}`"),
            Error::Parse { what } => write!(f, "could not parse {what}"),
            Error::NotUtf8 { command } => {
                write!(f, "`{command}` produced output that is not valid UTF-8")
            }
            Error::Io { path, source } => write!(f, "reading {}: {source}", path.display()),
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Error::Spawn { source, .. } | Error::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}
