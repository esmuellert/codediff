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

/// Refuses meta-names in types we declare.
///
/// Reads declarations only — `struct X`, `enum X`, `type X`, `trait X` — so a
/// `use std::io::ErrorKind` is untouched. A banned word is matched as a whole
/// word inside the name, so `Kind` and `RowKind` both fail while `Kindle`
/// would not.
pub fn check_type_names(root: &Path, failures: &mut Vec<String>) -> Result<()> {
    for dir in ["crates", "xtask"] {
        let path = root.join(dir);
        if !path.is_dir() {
            continue;
        }
        for file in rust_files(&path)? {
            let text = std::fs::read_to_string(&file)?;
            for (n, line) in text.lines().enumerate() {
                let Some(name) = declared_type(line) else {
                    continue;
                };
                for (word, instead) in BANNED_TYPE_WORDS {
                    if words_of(&name).any(|w| w == *word) {
                        failures.push(format!(
                            "{}:{} declares `{name}`; `{word}` says nothing — use {instead}",
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

/// The name a line declares a type under, if it declares one.
fn declared_type(line: &str) -> Option<String> {
    let line = line.trim_start();
    let rest = [
        "pub struct ",
        "pub enum ",
        "pub trait ",
        "pub type ",
        "struct ",
        "enum ",
        "trait ",
        "type ",
    ]
    .iter()
    .find_map(|prefix| line.strip_prefix(*prefix))?;
    let name: String = rest
        .chars()
        .take_while(|c| c.is_alphanumeric() || *c == '_')
        .collect();
    (!name.is_empty()).then_some(name)
}

/// A CamelCase name split into its words.
fn words_of(name: &str) -> impl Iterator<Item = String> + '_ {
    let mut words = Vec::new();
    let mut current = String::new();
    for c in name.chars() {
        if c.is_uppercase() && !current.is_empty() {
            words.push(std::mem::take(&mut current));
        }
        if c == '_' {
            if !current.is_empty() {
                words.push(std::mem::take(&mut current));
            }
            continue;
        }
        current.push(c);
    }
    if !current.is_empty() {
        words.push(current);
    }
    words.into_iter()
}

/// Refuses a module a directory is not allowed to know about.
/// Refuses a thread anywhere but the workers.
pub fn check_threads(root: &Path, failures: &mut Vec<String>) -> Result<()> {
    let mut allowed = Vec::new();
    for name in THREAD_FILES {
        let path = root.join(name);
        if path.is_file() {
            allowed.push(path);
        } else {
            failures.push(format!("`{name}` is missing, so its rule is dead"));
        }
    }
    if allowed.len() < THREAD_FILES.len() {
        return Ok(());
    }
    for dir in ["crates", "xtask"] {
        let base = root.join(dir);
        if !base.is_dir() {
            continue;
        }
        for file in rust_files(&base)? {
            // Tests may start one: proving that two things do not block each
            // other takes two things. And the rule may name what it forbids.
            let is_test = file
                .components()
                .any(|c| c.as_os_str() == "tests" || c.as_os_str() == "benches");
            if allowed.contains(&file) || is_test || file.ends_with("lint_arch/rules.rs") {
                continue;
            }
            let text = std::fs::read_to_string(&file)?;
            let Some(code) = text.split("#[cfg(test)]").next() else {
                continue;
            };
            for (n, line) in code.lines().enumerate() {
                let trimmed = line.trim_start();
                if trimmed.starts_with("//") {
                    continue;
                }
                for marker in THREAD_MARKERS {
                    if trimmed.contains(marker) {
                        failures.push(format!(
                            "{}:{} starts a thread — only a worker in `THREAD_FILES` may, \
                             so that \"which thread owns this?\" keeps an obvious answer",
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

pub fn check_blind_dirs(root: &Path, failures: &mut Vec<String>) -> Result<()> {
    for (dir, forbidden, why) in BLIND_DIRS {
        let path = root.join(dir);
        if !path.is_dir() {
            failures.push(format!("`{dir}` is missing, so its rule is dead"));
            continue;
        }
        for file in rust_files(&path)? {
            let text = std::fs::read_to_string(&file)?;
            for (n, line) in text.lines().enumerate() {
                let code = line.trim_start();
                if code.starts_with("//") {
                    continue;
                }
                if code.contains(forbidden) {
                    failures.push(format!(
                        "{}:{} names `{forbidden}` — {why}",
                        rel(root, &file),
                        n + 1
                    ));
                }
            }
        }
    }
    Ok(())
}

/// Refuses anything that waits, where the loop can reach it.
///
/// The companion to [`check_threads`]: that one says slow work happens on a
/// worker, this one says the drawing thread never does it. Both are needed —
/// a worker nobody uses prevents nothing.
pub fn check_non_blocking(root: &Path, failures: &mut Vec<String>) -> Result<()> {
    for name in NON_BLOCKING_FILES {
        let file = root.join(name);
        if !file.is_file() {
            failures.push(format!("`{name}` is missing, so its rule is dead"));
            continue;
        }
        blocking_calls(root, &file, name, failures)?;
    }
    for dir in NON_BLOCKING_DIRS {
        let path = root.join(dir);
        if !path.is_dir() {
            failures.push(format!("`{dir}` is missing, so its rule is dead"));
            continue;
        }
        for file in rust_files(&path)? {
            blocking_calls(root, &file, dir, failures)?;
        }
    }
    Ok(())
}

/// Reports every blocking call in one file, naming the rule that caught it.
fn blocking_calls(root: &Path, file: &Path, place: &str, failures: &mut Vec<String>) -> Result<()> {
    let text = std::fs::read_to_string(file)?;
    // Tests may wait: proving that a worker answered means waiting for it.
    // What must not block is what a frame reaches.
    let Some(code) = text.split("#[cfg(test)]").next() else {
        return Ok(());
    };
    for (n, line) in code.lines().enumerate() {
        let line = line.trim_start();
        if line.starts_with("//") {
            continue;
        }
        for marker in BLOCKING_MARKERS {
            if line.contains(marker) {
                failures.push(format!(
                    "{}:{} names `{marker}` — {place} is reached on every key and \
                     every frame, so nothing in it may wait",
                    rel(root, file),
                    n + 1
                ));
            }
        }
    }
    Ok(())
}
