//! Loading, validating, and atomically saving preferences.

#[cfg(unix)]
use std::fs::File;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use crate::Config;

pub type Result<T> = std::result::Result<T, Error>;

/// A failure that prevents configuration storage from completing.
#[derive(Debug)]
pub enum Error {
    Io {
        path: PathBuf,
        source: io::Error,
    },
    Json {
        path: PathBuf,
        source: serde_json::Error,
    },
    Invalid {
        path: PathBuf,
        message: String,
    },
    MissingConfigDirectory,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io { path, source } => write!(f, "{}: {source}", path.display()),
            Self::Json { path, source } => {
                write!(f, "could not parse {}: {source}", path.display())
            }
            Self::Invalid { path, message } => {
                write!(f, "invalid configuration {}: {message}", path.display())
            }
            Self::MissingConfigDirectory => {
                f.write_str("could not find a user configuration directory")
            }
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Json { source, .. } => Some(source),
            Self::Invalid { .. } | Self::MissingConfigDirectory => None,
        }
    }
}

/// The loaded preferences and their destination.
#[derive(Debug)]
pub struct ConfigStore {
    path: PathBuf,
    config: Config,
    warning: Option<String>,
}

impl ConfigStore {
    /// Opens a specific file. A missing file means the built-in defaults.
    pub fn open(path: PathBuf) -> Result<Self> {
        let (config, warning) = load(&path)?;
        Ok(Self {
            path,
            config,
            warning,
        })
    }

    /// Opens the path selected by the command-line override, environment, or
    /// platform default.
    pub fn open_default(explicit: Option<&Path>) -> Result<Self> {
        Self::open(crate::resolve(explicit)?)
    }

    pub fn get(&self) -> &Config {
        &self.config
    }

    /// Returns a non-fatal warning for a corrupt or unsupported file.
    pub fn warning(&self) -> Option<&str> {
        self.warning.as_deref()
    }

    /// Applies an edit and persists it before changing the in-memory value.
    pub fn update(&mut self, edit: impl FnOnce(&mut Config)) -> Result<()> {
        let mut next = self.config.clone();
        edit(&mut next);
        next.validate().map_err(|message| Error::Invalid {
            path: self.path.clone(),
            message,
        })?;
        save(&self.path, &next)?;
        self.config = next;
        self.warning = None;
        Ok(())
    }

    /// Re-reads the file, retaining defaults if it is malformed.
    pub fn reload(&mut self) -> Result<()> {
        let (config, warning) = load(&self.path)?;
        self.config = config;
        self.warning = warning;
        Ok(())
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

fn load(path: &Path) -> Result<(Config, Option<String>)> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Ok((Config::default(), None));
        }
        Err(source) => {
            return Err(Error::Io {
                path: path.to_owned(),
                source,
            });
        }
    };

    let config: Config = match serde_json::from_str(&text) {
        Ok(config) => config,
        Err(source) => {
            return Ok((
                Config::default(),
                Some(format!(
                    "ignoring malformed configuration {}: {source}",
                    path.display()
                )),
            ));
        }
    };
    if let Err(message) = config.validate() {
        return Ok((
            Config::default(),
            Some(format!(
                "ignoring invalid configuration {}: {message}",
                path.display()
            )),
        ));
    }
    Ok((config, None))
}

fn save(path: &Path, config: &Config) -> Result<()> {
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty());
    if let Some(parent) = parent {
        fs::create_dir_all(parent).map_err(|source| Error::Io {
            path: parent.to_owned(),
            source,
        })?;
    }

    let parent = parent.unwrap_or_else(|| Path::new("."));
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("config.json");
    let temporary = parent.join(format!(".{name}.{}.tmp", std::process::id()));
    let _ = fs::remove_file(&temporary);

    let result = write_temporary(&temporary, config)
        .and_then(|()| {
            fs::rename(&temporary, path).map_err(|source| Error::Io {
                path: path.to_owned(),
                source,
            })
        })
        .and_then(|()| sync_parent_after_rename(parent));
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

fn write_temporary(path: &Path, config: &Config) -> Result<()> {
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(path)
        .map_err(|source| Error::Io {
            path: path.to_owned(),
            source,
        })?;
    let text = serde_json::to_string_pretty(config).map_err(|source| Error::Json {
        path: path.to_owned(),
        source,
    })?;
    file.write_all(text.as_bytes())
        .and_then(|()| file.write_all(b"\n"))
        .and_then(|()| file.sync_all())
        .map_err(|source| Error::Io {
            path: path.to_owned(),
            source,
        })?;
    drop(file);

    Ok(())
}

#[cfg(unix)]
fn sync_parent_after_rename(path: &Path) -> Result<()> {
    sync_parent(path)
}

#[cfg(not(unix))]
fn sync_parent_after_rename(_path: &Path) -> Result<()> {
    Ok(())
}

#[cfg(unix)]
fn sync_parent(path: &Path) -> Result<()> {
    File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|source| Error::Io {
            path: path.to_owned(),
            source,
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn path(name: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("codediff-config-{name}-{nonce}.json"))
    }

    #[test]
    fn missing_files_open_with_defaults_without_writing() {
        let path = path("missing");
        let store = ConfigStore::open(path.clone()).unwrap();
        assert_eq!(store.get(), &Config::default());
        assert!(!path.exists());
    }

    #[test]
    fn update_writes_pretty_json_and_reload_reads_it() {
        let path = path("round-trip");
        let mut store = ConfigStore::open(path.clone()).unwrap();
        store
            .update(|config| config.ui.wrap = false)
            .expect("saving config");
        assert!(
            std::fs::read_to_string(&path)
                .unwrap()
                .contains("\n  \"ui\":")
        );

        store.update(|config| config.ui.wrap = true).unwrap();
        store.reload().unwrap();
        assert!(store.get().ui.wrap);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn malformed_files_fall_back_without_replacing_the_original() {
        let path = path("malformed");
        std::fs::write(&path, "not json").unwrap();
        let store = ConfigStore::open(path.clone()).unwrap();
        assert_eq!(store.get(), &Config::default());
        assert!(store.warning().is_some());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "not json");
        let _ = std::fs::remove_file(path);
    }
}
