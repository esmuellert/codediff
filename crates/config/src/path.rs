//! Finds the user configuration file.

use std::path::{Path, PathBuf};

use crate::{Error, Result};

/// Resolves `--config`, `CODEDIFF_CONFIG`, or the platform default in order.
pub fn resolve(explicit: Option<&Path>) -> Result<PathBuf> {
    if let Some(path) = explicit {
        return Ok(path.to_owned());
    }
    if let Some(path) = std::env::var_os("CODEDIFF_CONFIG")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
    {
        return Ok(path);
    }
    default_path()
}

/// The per-user configuration path.
///
/// Unix follows XDG-style paths. Windows keeps configuration in `%APPDATA%`.
pub fn default_path() -> Result<PathBuf> {
    #[cfg(windows)]
    {
        let app_data = std::env::var_os("APPDATA")
            .filter(|value| !value.is_empty())
            .ok_or(Error::MissingConfigDirectory)?;
        Ok(PathBuf::from(app_data).join("codediff").join("config.json"))
    }

    #[cfg(not(windows))]
    {
        let base = std::env::var_os("XDG_CONFIG_HOME")
            .filter(|value| !value.is_empty())
            .or_else(|| {
                std::env::var_os("HOME")
                    .map(|home| PathBuf::from(home).join(".config").into_os_string())
            })
            .ok_or(Error::MissingConfigDirectory)?;
        Ok(PathBuf::from(base).join("codediff").join("config.json"))
    }
}
