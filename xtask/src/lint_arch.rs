//! `cargo xtask lint-arch`
//!
//! Cargo enforces exactly one architectural rule for free: crate dependencies
//! must be acyclic. Every other rule in docs/plan is project-specific and has
//! to be encoded somewhere. This is that somewhere.

use anyhow::{Result, bail};
use std::path::{Path, PathBuf};

/// Edges that must never exist, with the reason reported on failure.
const FORBIDDEN_EDGES: &[(&str, &str, &str)] = &[
    ("display", "vcs", "a renderer must not be able to reach git"),
    (
        "display",
        "vscode-diff",
        "rendering consumes model types, it does not compute diffs",
    ),
    (
        "display",
        "vscode-diff-sys",
        "rendering must never touch the FFI layer",
    ),
    (
        "align",
        "vcs",
        "the aligned model is pure and must not perform IO",
    ),
    (
        "explorer",
        "vcs",
        "the explorer model is pure; obtaining entries belongs to vcs",
    ),
    ("line-index", "vcs", "text measurement must not perform IO"),
    ("syntax", "vcs", "syntactic analysis must not perform IO"),
];

/// Crates that must not perform IO, so that they stay trivially testable.
const PURE_CRATES: &[&str] = &["line-index", "syntax", "align", "explorer", "vscode-diff"];

const IO_MARKERS: &[&str] = &["std::fs", "std::process", "std::net", "std::env::var"];

/// Crates exempt from `unsafe_code = "forbid"`, with the policy they use instead.
const UNSAFE_EXEMPT: &[&str] = &["vscode-diff-sys", "vscode-diff"];

/// A syntax engine may only be named inside this directory, so that swapping
/// engines touches nothing else. See docs/plan/05-decisions.md D17.
const ENGINE_CRATES: &[&str] = &["syntect", "tree_sitter"];
const ENGINE_DIR: &str = "crates/syntax/src/engine";

pub fn run() -> Result<()> {
    let root = crate::workspace_root();
    let mut failures = Vec::new();

    let (applied, pending) = check_edges(&root, &mut failures)?;
    check_purity(&root, &mut failures)?;
    check_unsafe_policy(&root, &mut failures)?;
    check_engine_confinement(&root, &mut failures)?;

    if !failures.is_empty() {
        let mut msg = format!("{} architecture violation(s):\n", failures.len());
        for f in &failures {
            msg.push_str(&format!("  {f}\n"));
        }
        bail!(msg);
    }

    println!("lint-arch: purity, unsafe policy and engine confinement clean");
    println!("  edge rules: {applied} applied, {pending} awaiting their crate");
    if !pending_names(&root)?.is_empty() {
        // Named so that a rule cannot quietly stay dead because of a typo in
        // FORBIDDEN_EDGES.
        println!("  pending:    {}", pending_names(&root)?.join(", "));
    }
    Ok(())
}

/// Returns (rules applied, rules whose `from` crate does not exist yet).
fn check_edges(root: &Path, failures: &mut Vec<String>) -> Result<(usize, usize)> {
    let mut applied = 0;
    let mut pending = 0;

    for (from, to, why) in FORBIDDEN_EDGES {
        let manifest = root.join("crates").join(from).join("Cargo.toml");
        if !manifest.is_file() {
            pending += 1;
            continue;
        }
        applied += 1;
        if declares_dependency(&manifest, to)? {
            failures.push(format!("`{from}` must not depend on `{to}`: {why}"));
        }
    }
    Ok((applied, pending))
}

/// Crates named in FORBIDDEN_EDGES that have not been created yet.
fn pending_names(root: &Path) -> Result<Vec<String>> {
    let mut names: Vec<String> = FORBIDDEN_EDGES
        .iter()
        .map(|(from, _, _)| *from)
        .filter(|from| !root.join("crates").join(from).join("Cargo.toml").is_file())
        .map(str::to_owned)
        .collect();
    names.sort();
    names.dedup();
    Ok(names)
}

fn check_purity(root: &Path, failures: &mut Vec<String>) -> Result<()> {
    for crate_name in PURE_CRATES {
        let src = root.join("crates").join(crate_name).join("src");
        if !src.is_dir() {
            continue;
        }
        for file in rust_files(&src)? {
            let text = std::fs::read_to_string(&file)?;
            for (n, line) in text.lines().enumerate() {
                if line.trim_start().starts_with("//") || line.trim_start().starts_with("//!") {
                    continue;
                }
                for marker in IO_MARKERS {
                    if line.contains(marker) {
                        failures.push(format!(
                            "`{crate_name}` is a pure crate but {}:{} uses `{marker}`",
                            rel(root, &file),
                            n + 1
                        ));
                    }
                }
            }
        }
    }
    Ok(())
}

fn check_unsafe_policy(root: &Path, failures: &mut Vec<String>) -> Result<()> {
    for manifest in crate_manifests(root)? {
        let name = package_name(&manifest)?;
        if UNSAFE_EXEMPT.contains(&name.as_str()) {
            continue;
        }
        if !inherits_workspace_lints(&manifest)? {
            failures.push(format!(
                "`{name}` must declare `[lints] workspace = true`, which forbids unsafe code"
            ));
        }
    }
    Ok(())
}

fn check_engine_confinement(root: &Path, failures: &mut Vec<String>) -> Result<()> {
    let allowed = root.join(ENGINE_DIR);
    for dir in ["crates", "xtask"] {
        let base = root.join(dir);
        if !base.is_dir() {
            continue;
        }
        for file in rust_files(&base)? {
            if file.starts_with(&allowed) {
                continue;
            }
            let text = std::fs::read_to_string(&file)?;
            for engine in ENGINE_CRATES {
                let needle = format!("{engine}::");
                if text.contains(&needle) {
                    failures.push(format!(
                        "{} names `{engine}`; a syntax engine may only appear under {ENGINE_DIR}",
                        rel(root, &file)
                    ));
                }
            }
        }
    }
    Ok(())
}

fn declares_dependency(manifest: &Path, dep: &str) -> Result<bool> {
    let value: toml::Table = std::fs::read_to_string(manifest)?.parse()?;
    for table in ["dependencies", "dev-dependencies", "build-dependencies"] {
        if value
            .get(table)
            .and_then(toml::Value::as_table)
            .is_some_and(|t| t.contains_key(dep))
        {
            return Ok(true);
        }
    }
    Ok(false)
}

fn inherits_workspace_lints(manifest: &Path) -> Result<bool> {
    let value: toml::Table = std::fs::read_to_string(manifest)?.parse()?;
    Ok(value
        .get("lints")
        .and_then(|l| l.get("workspace"))
        .and_then(toml::Value::as_bool)
        .unwrap_or(false))
}

fn package_name(manifest: &Path) -> Result<String> {
    let value: toml::Table = std::fs::read_to_string(manifest)?.parse()?;
    value
        .get("package")
        .and_then(|p| p.get("name"))
        .and_then(toml::Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| anyhow::anyhow!("{} has no package.name", manifest.display()))
}

fn crate_manifests(root: &Path) -> Result<Vec<PathBuf>> {
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

fn rust_files(dir: &Path) -> Result<Vec<PathBuf>> {
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

fn rel(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .into_owned()
}
