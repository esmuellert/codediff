//! Finding crates and reading manifests.
//!
//! Deliberately string-matching rather than parsing: the questions asked are
//! narrow enough that a TOML parser would be a dependency bought for four
//! lookups, and a rule that silently stops matching is caught by the tests
//! that sabotage it.

use anyhow::Result;
use std::path::{Path, PathBuf};

pub fn declares_dependency(manifest: &Path, dep: &str, tables: &[&str]) -> Result<bool> {
    let value: toml::Table = std::fs::read_to_string(manifest)?.parse()?;
    for table in tables {
        if value
            .get(*table)
            .and_then(toml::Value::as_table)
            .is_some_and(|t| t.contains_key(dep))
        {
            return Ok(true);
        }
    }
    Ok(false)
}

pub fn inherits_workspace_lints(manifest: &Path) -> Result<bool> {
    let value: toml::Table = std::fs::read_to_string(manifest)?.parse()?;
    Ok(value
        .get("lints")
        .and_then(|l| l.get("workspace"))
        .and_then(toml::Value::as_bool)
        .unwrap_or(false))
}

pub fn package_name(manifest: &Path) -> Result<String> {
    let value: toml::Table = std::fs::read_to_string(manifest)?.parse()?;
    value
        .get("package")
        .and_then(|p| p.get("name"))
        .and_then(toml::Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| anyhow::anyhow!("{} has no package.name", manifest.display()))
}

pub fn crate_manifests(root: &Path) -> Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    for base in ["crates", "xtask"] {
        let dir = root.join(base);
        if dir.join("Cargo.toml").is_file() {
            out.push(dir.join("Cargo.toml"));
            continue;
        }
        if !dir.is_dir() {
            continue;
        }
        for entry in std::fs::read_dir(&dir)? {
            let manifest = entry?.path().join("Cargo.toml");
            if manifest.is_file() {
                out.push(manifest);
            }
        }
    }
    out.sort();
    Ok(out)
}

pub fn rust_files(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    walk(dir, &mut out)?;
    out.sort();
    Ok(out)
}

fn walk(dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            if path.file_name().is_some_and(|n| n == "target") {
                continue;
            }
            walk(&path, out)?;
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
    Ok(())
}

pub fn rel(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .into_owned()
}
