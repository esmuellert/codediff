//! Reading, writing and verifying `vendor/UPSTREAM.lock`.
//!
//! The lock records where the vendored C came from and a content hash of the
//! extracted tree, so that a local edit to `libvscode-diff/` becomes a CI failure
//! rather than a silent fork. See docs/plan/05-decisions.md D3.

use anyhow::{Context, Result, bail};
use sha2::{Digest, Sha256};
use std::fmt::Write as _;
use std::path::{Path, PathBuf};

pub const LOCK_NAME: &str = "UPSTREAM.lock";

#[derive(Debug, Clone)]
pub struct Lock {
    pub repository: String,
    pub tag: String,
    pub commit: String,
    pub version: String,
    pub tree_sha256: String,
}

impl Lock {
    pub fn render(&self) -> String {
        let mut s = String::new();
        s.push_str(
            "# Provenance of the vendored C diff engine.\n\
             # Written by `cargo xtask sync-c`; checked by `cargo xtask verify-c`.\n\
             # Do not edit libvscode-diff by hand — patch upstream instead.\n\n",
        );
        let _ = writeln!(s, "repository = \"{}\"", self.repository);
        let _ = writeln!(s, "tag = \"{}\"", self.tag);
        let _ = writeln!(s, "commit = \"{}\"", self.commit);
        let _ = writeln!(s, "version = \"{}\"", self.version);
        let _ = writeln!(s, "tree_sha256 = \"{}\"", self.tree_sha256);
        s
    }

    pub fn read(path: &Path) -> Result<Self> {
        let text =
            std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        let value: toml::Table = text.parse().context("parsing UPSTREAM.lock as TOML")?;

        let get = |key: &str| -> Result<String> {
            value
                .get(key)
                .and_then(toml::Value::as_str)
                .map(str::to_owned)
                .with_context(|| format!("UPSTREAM.lock is missing `{key}`"))
        };

        Ok(Self {
            repository: get("repository")?,
            tag: get("tag")?,
            commit: get("commit")?,
            version: get("version")?,
            tree_sha256: get("tree_sha256")?,
        })
    }
}

/// Hash every file under `dir`, in sorted path order.
///
/// Line endings are normalised to LF before hashing so that the result does not
/// depend on the platform's checkout settings; without this, `verify-c` would
/// fail spuriously on Windows.
pub fn hash_tree(dir: &Path) -> Result<String> {
    let mut files = Vec::new();
    collect(dir, dir, &mut files)?;
    files.sort();

    let mut outer = Sha256::new();
    for rel in &files {
        let bytes = std::fs::read(dir.join(rel))?;
        let normalised: Vec<u8> = if bytes.contains(&b'\r') {
            let mut out = Vec::with_capacity(bytes.len());
            let mut iter = bytes.iter().peekable();
            while let Some(&b) = iter.next() {
                if b == b'\r' && iter.peek() == Some(&&b'\n') {
                    continue;
                }
                out.push(b);
            }
            out
        } else {
            bytes
        };

        let mut inner = Sha256::new();
        inner.update(&normalised);
        outer.update(rel.as_bytes());
        outer.update([0u8]);
        outer.update(inner.finalize());
    }

    Ok(hex(&outer.finalize()))
}

/// Lowercase hex, two digits per byte.
fn hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        let _ = write!(s, "{b:02x}");
    }
    s
}

fn collect(root: &Path, dir: &Path, out: &mut Vec<String>) -> Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            collect(root, &path, out)?;
        } else {
            let rel = path
                .strip_prefix(root)
                .expect("walked path is under root")
                .to_string_lossy()
                .replace('\\', "/");
            out.push(rel);
        }
    }
    Ok(())
}

pub fn vendor_dir(root: &Path) -> PathBuf {
    root.join("vendor")
}

pub fn engine_dir(root: &Path) -> PathBuf {
    root.join("libvscode-diff")
}

/// Upstream's diff fixtures, used by `verify-oracle`.
pub fn test_pairs_dir(root: &Path) -> PathBuf {
    test_pairs_dir_in(&vendor_dir(root))
}

pub fn test_pairs_dir_in(vendor: &Path) -> PathBuf {
    vendor.join("test-pairs")
}

pub fn require_vendored(root: &Path) -> Result<()> {
    let dir = engine_dir(root);
    if !dir.is_dir() {
        bail!(
            "libvscode-diff is missing.\n\
             Run: cargo xtask sync-c --tag <tag>"
        );
    }
    Ok(())
}
