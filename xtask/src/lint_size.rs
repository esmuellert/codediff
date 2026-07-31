//! `cargo xtask lint-size`
//!
//! Fails if a source file exceeds the hard cap. Without this the line limit in
//! docs/plan is a sentence nobody enforces, and files grow into junk drawers —
//! the failure that produced a 674-line `explorer/render.lua` upstream.
//!
//! Test code is not counted. Otherwise the cap would punish writing tests, and
//! the natural response would be to move tests out of the file to stay under
//! it, defeating both rules at once.

use anyhow::{Result, bail};
use std::path::{Path, PathBuf};

const SOFT_CAP: usize = 300;
const HARD_CAP: usize = 500;

pub fn run() -> Result<()> {
    let root = crate::workspace_root();
    let mut files = Vec::new();
    for dir in ["crates", "xtask"] {
        let path = root.join(dir);
        if path.is_dir() {
            collect_rs(&path, &mut files)?;
        }
    }
    files.sort();

    let mut over_soft = Vec::new();
    let mut over_hard = Vec::new();

    for file in &files {
        let text = std::fs::read_to_string(file)?;
        let lines = code_lines(&text);
        let shown = file
            .strip_prefix(&root)
            .unwrap_or(file)
            .to_string_lossy()
            .into_owned();
        if lines > HARD_CAP {
            over_hard.push((shown, lines));
        } else if lines > SOFT_CAP {
            over_soft.push((shown, lines));
        }
    }

    for (file, lines) in &over_soft {
        println!("  soft cap ({SOFT_CAP}): {file} has {lines} lines — consider splitting");
    }

    if !over_hard.is_empty() {
        let mut msg = format!(
            "{} file(s) exceed the hard cap of {HARD_CAP}:\n",
            over_hard.len()
        );
        for (file, lines) in &over_hard {
            msg.push_str(&format!("  {file}: {lines} lines\n"));
        }
        msg.push_str("\nSplit by noun — a type and its behaviour — never by verb.");
        bail!(msg);
    }

    println!(
        "lint-size: {} file(s) checked, none over {HARD_CAP} lines ({} over the {SOFT_CAP} soft cap)",
        files.len(),
        over_soft.len()
    );
    Ok(())
}

/// Non-test, non-blank lines.
///
/// Unit tests live at the bottom of the file they test, in `#[cfg(test)] mod
/// tests`, so that they can reach private items. Counting stops there.
fn code_lines(text: &str) -> usize {
    text.lines()
        .take_while(|line| !line.trim_start().starts_with("#[cfg(test)]"))
        .filter(|line| !line.trim().is_empty())
        .count()
}

fn collect_rs(dir: &Path, out: &mut Vec<PathBuf>) -> Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            if path.file_name().is_some_and(|n| n == "target") {
                continue;
            }
            collect_rs(&path, out)?;
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::code_lines;

    #[test]
    fn ignores_blank_lines() {
        assert_eq!(code_lines("a\n\n\nb\n"), 2);
    }

    #[test]
    fn stops_at_the_test_module() {
        let src = "a\nb\n#[cfg(test)]\nmod tests {\n    fn x() {}\n}\n";
        assert_eq!(code_lines(src), 2);
    }

    #[test]
    fn stops_at_an_indented_test_module() {
        let src = "a\n    #[cfg(test)]\n    mod tests {}\n";
        assert_eq!(code_lines(src), 1);
    }
}
