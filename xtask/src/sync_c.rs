//! `cargo xtask sync-c --tag <tag> [--from <path>]`
//!
//! Replaces `libvscode-diff/` and its vendor metadata from an upstream tag.
//! This is the only sanctioned way to update the copied C source; doing it by
//! hand is how a silent fork begins.

use anyhow::{Context, Result, bail};
use std::path::Path;
use std::process::Command;

use crate::lock::{self, Lock};
use crate::workspace_root;

const DEFAULT_REMOTE: &str = "https://github.com/esmuellert/codediff.nvim.git";

pub fn run(args: &[String]) -> Result<()> {
    let mut tag = None;
    let mut from = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--tag" => {
                tag = args.get(i + 1).cloned();
                i += 2;
            }
            "--from" => {
                from = args.get(i + 1).cloned();
                i += 2;
            }
            other => bail!("sync-c: unexpected argument `{other}`"),
        }
    }

    let Some(tag) = tag else {
        bail!("sync-c: --tag <tag> is required, e.g. --tag v2.60.0");
    };

    let root = workspace_root();
    let source = match &from {
        Some(path) => path.clone(),
        None => clone_upstream(&tag)?,
    };
    let source = Path::new(&source);

    let commit = git(source, &["rev-parse", &format!("{tag}^{{commit}}")])
        .with_context(|| format!("resolving tag {tag}; is it fetched?"))?;
    let version = git(source, &["show", &format!("{tag}:VERSION")])?;

    let vendor = lock::vendor_dir(&root);
    if vendor.exists() {
        std::fs::remove_dir_all(&vendor).context("clearing vendor/")?;
    }
    std::fs::create_dir_all(&vendor)?;

    let engine = lock::engine_dir(&root);
    if engine.exists() {
        std::fs::remove_dir_all(&engine).context("clearing libvscode-diff/")?;
    }
    extract(source, &tag, "libvscode-diff", &root)?;
    adapt_to_repository_layout(&engine)?;
    // The Rust build and oracle generate version.h from the imported version.
    std::fs::write(vendor.join("VERSION"), format!("{version}\n"))?;
    // Upstream's own diff fixtures, used by `verify-oracle`. Taking their cases
    // rather than inventing ours is the point: an oracle we chose the questions
    // for proves much less.
    extract_test_pairs(source, &tag, &vendor)?;

    let tree_sha256 = lock::hash_tree(&engine)?;

    let lock = Lock {
        repository: DEFAULT_REMOTE.to_owned(),
        tag: tag.clone(),
        commit,
        version: version.clone(),
        tree_sha256,
    };
    std::fs::write(vendor.join(lock::LOCK_NAME), lock.render())?;

    println!("vendored libvscode-diff {version} from {tag}");
    println!("  commit      {}", lock.commit);
    println!("  tree sha256 {}", lock.tree_sha256);
    println!("  fixtures    {} oracle pair(s)", count_pairs(&vendor)?);

    report_bundled_dependencies(&engine)?;
    Ok(())
}

fn adapt_to_repository_layout(engine: &Path) -> Result<()> {
    replace_once(
        &engine.join("CMakeLists.txt"),
        "${CMAKE_CURRENT_SOURCE_DIR}/../VERSION",
        "${CMAKE_CURRENT_SOURCE_DIR}/../vendor/VERSION",
    )?;
    replace_once(
        &engine.join("build.sh.in"),
        "cat ../VERSION",
        "cat ../vendor/VERSION",
    )?;
    replace_once(
        &engine.join("build.cmd.in"),
        "<..\\VERSION",
        "<..\\vendor\\VERSION",
    )
}

fn replace_once(path: &Path, from: &str, to: &str) -> Result<()> {
    let text = std::fs::read_to_string(path)?;
    if text.matches(from).count() != 1 {
        bail!("expected exactly one `{from}` in {}", path.display());
    }
    std::fs::write(path, text.replacen(from, to, 1))?;
    Ok(())
}

/// Extracts `scripts/test_pairs/` and flattens it to `vendor/test-pairs/`.
fn extract_test_pairs(source: &Path, tag: &str, vendor: &Path) -> Result<()> {
    let staging = vendor.join(".staging");
    std::fs::create_dir_all(&staging)?;
    extract(source, tag, "scripts/test_pairs", &staging)?;

    let from = staging.join("scripts").join("test_pairs");
    let to = lock::test_pairs_dir_in(vendor);
    if from.is_dir() {
        std::fs::rename(&from, &to)?;
    }
    std::fs::remove_dir_all(&staging)?;
    Ok(())
}

fn count_pairs(vendor: &Path) -> Result<usize> {
    let dir = lock::test_pairs_dir_in(vendor);
    if !dir.is_dir() {
        return Ok(0);
    }
    Ok(std::fs::read_dir(dir)?
        .filter_map(Result::ok)
        .filter(|e| e.path().is_dir())
        .count())
}

/// Lists the third-party sources bundled inside the C engine.
///
/// ATTRIBUTION.md is maintained by hand, so this is the reminder that its list
/// still matches reality after a sync.
fn report_bundled_dependencies(engine: &Path) -> Result<()> {
    let bundled = engine.join("vendor");
    if !bundled.is_dir() {
        return Ok(());
    }

    let mut names = Vec::new();
    for entry in std::fs::read_dir(&bundled)? {
        names.push(entry?.file_name().to_string_lossy().into_owned());
    }

    let unexpected = unattributed(&names);
    if unexpected.is_empty() {
        println!("  bundled     utf8proc only — ATTRIBUTION.md is up to date");
    } else {
        println!();
        println!("  New bundled sources in libvscode-diff/vendor/:");
        for name in &unexpected {
            println!("    {name}");
        }
        println!("  Add them to ATTRIBUTION.md.");
    }
    Ok(())
}

/// Names in the engine's bundled directory that ATTRIBUTION.md does not cover.
fn unattributed(names: &[String]) -> Vec<String> {
    const ATTRIBUTED_PREFIX: &str = "utf8proc";

    let mut out: Vec<String> = names
        .iter()
        .filter(|name| !name.starts_with(ATTRIBUTED_PREFIX) && name.as_str() != "README.md")
        .cloned()
        .collect();
    out.sort();
    out
}

fn clone_upstream(tag: &str) -> Result<String> {
    let dir = std::env::temp_dir().join(format!("codediff-sync-{tag}"));
    if dir.exists() {
        std::fs::remove_dir_all(&dir)?;
    }
    let status = Command::new("git")
        .args([
            "clone",
            "--quiet",
            "--depth",
            "1",
            "--branch",
            tag,
            DEFAULT_REMOTE,
        ])
        .arg(&dir)
        .status()
        .context("running git clone")?;
    if !status.success() {
        bail!("git clone of {DEFAULT_REMOTE} at {tag} failed");
    }
    Ok(dir.to_string_lossy().into_owned())
}

/// `git archive <tag> <subdir> | tar -x -C <dest>`, without a shell.
fn extract(source: &Path, tag: &str, subdir: &str, dest: &Path) -> Result<()> {
    let archive = Command::new("git")
        .current_dir(source)
        .args(["archive", tag, subdir])
        .output()
        .context("running git archive")?;
    if !archive.status.success() {
        bail!(
            "git archive {tag} {subdir} failed: {}",
            String::from_utf8_lossy(&archive.stderr).trim()
        );
    }

    let mut tar = Command::new("tar")
        .arg("-x")
        .arg("-C")
        .arg(dest)
        .stdin(std::process::Stdio::piped())
        .spawn()
        .context("running tar")?;
    {
        use std::io::Write as _;
        let stdin = tar.stdin.as_mut().expect("stdin was piped");
        stdin.write_all(&archive.stdout)?;
    }
    if !tar.wait()?.success() {
        bail!("tar extraction failed");
    }
    Ok(())
}

fn git(dir: &Path, args: &[&str]) -> Result<String> {
    let out = Command::new("git")
        .current_dir(dir)
        .args(args)
        .output()
        .context("running git")?;
    if !out.status.success() {
        bail!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(String::from_utf8(out.stdout)?.trim().to_owned())
}

#[cfg(test)]
mod tests {
    use super::unattributed;

    fn names(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| (*s).to_owned()).collect()
    }

    #[test]
    fn the_current_bundle_is_fully_attributed() {
        let actual = names(&[
            "README.md",
            "utf8proc.c",
            "utf8proc.h",
            "utf8proc_LICENSE.md",
            "utf8proc_data.c",
        ]);
        assert!(unattributed(&actual).is_empty());
    }

    #[test]
    fn a_new_bundled_library_is_reported() {
        let actual = names(&["utf8proc.c", "zlib.c", "zlib.h"]);
        assert_eq!(unattributed(&actual), names(&["zlib.c", "zlib.h"]));
    }
}
