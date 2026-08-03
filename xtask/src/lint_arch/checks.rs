//! Applying the rules.
//!
//! One function per rule in [`super::rules`], each pushing a sentence onto
//! `failures` rather than returning early, so a single run reports everything
//! that is wrong instead of the first thing.

use anyhow::Result;
use std::path::Path;

use super::files::{
    crate_manifests, declares_dependency, inherits_workspace_lints, package_name, rel, rust_files,
};
use super::rules::*;

/// Returns (rules applied, rules whose `from` crate does not exist yet).
pub fn check_edges(root: &Path, failures: &mut Vec<String>) -> Result<(usize, usize)> {
    let mut applied = 0;
    let mut pending = 0;

    // Anywhere in the manifest, tests included.
    const ANY: &[&str] = &["dependencies", "dev-dependencies", "build-dependencies"];
    // What ships. A dev-dependency is not part of it.
    const SHIPPED: &[&str] = &["dependencies", "build-dependencies"];

    for (rules, tables) in [(FORBIDDEN_EDGES, ANY), (FORBIDDEN_SHIPPED_EDGES, SHIPPED)] {
        for (from, to, why) in rules {
            let manifest = root.join("crates").join(from).join("Cargo.toml");
            if !manifest.is_file() {
                pending += 1;
                continue;
            }
            applied += 1;
            if declares_dependency(&manifest, to, tables)? {
                failures.push(format!("`{from}` must not depend on `{to}`: {why}"));
            }
        }
    }
    Ok((applied, pending))
}

/// Crates named in FORBIDDEN_EDGES that have not been created yet.
pub fn pending_names(root: &Path) -> Result<Vec<String>> {
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

/// Refuses a clock where determinism depends on there not being one.
pub fn check_clock_free(root: &Path, failures: &mut Vec<String>) -> Result<()> {
    for (dir, why) in CLOCK_FREE_DIRS {
        let path = root.join(dir);
        // A missing directory is a failure, not a skip. It used to be a skip,
        // and renaming `display` to `ui` therefore switched this whole rule
        // off in silence — which is the exact failure the rule exists to
        // prevent, turned on the rule itself.
        if !path.is_dir() {
            failures.push(format!(
                "{dir} does not exist, so the rule that {why} is checking nothing"
            ));
            continue;
        }
        for file in rust_files(&path)? {
            let text = std::fs::read_to_string(&file)?;
            for (n, line) in text.lines().enumerate() {
                if line.trim_start().starts_with("//") {
                    continue;
                }
                for marker in CLOCK_MARKERS {
                    if line.contains(marker) {
                        failures.push(format!(
                            "{}:{} names `{marker}`, but {why}",
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

pub fn check_purity(root: &Path, failures: &mut Vec<String>) -> Result<()> {
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

pub fn check_unsafe_policy(root: &Path, failures: &mut Vec<String>) -> Result<()> {
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

pub fn check_engine_confinement(root: &Path, failures: &mut Vec<String>) -> Result<()> {
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
