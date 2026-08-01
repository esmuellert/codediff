//! `cargo xtask verify-oracle`
//!
//! Builds upstream's own `diff_tool` from the vendored C, runs it and our Rust
//! binding over the same fixtures, and compares the results structurally.
//!
//! This is the differential test that catches marshalling mistakes the unit
//! tests cannot: arrays passed in the wrong order, an off-by-one in a range, a
//! misread field. The fixtures are upstream's, deliberately — an oracle we
//! chose the questions for would prove much less.

use anyhow::{Context, Result, bail};
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::lock;
use crate::oracle_output::{self, OracleChange, OracleInner};
use vscode_diff::{LinesDiff, Options, compute};

/// Mirrors DIFF_CORE_SOURCES; see crates/vscode-diff-sys/build.rs.
const SOURCES: &[&str] = &[
    "default_lines_diff_computer.c",
    "src/char_level.c",
    "src/line_level.c",
    "src/myers.c",
    "src/optimize.c",
    "src/sequence.c",
    "src/range_mapping.c",
    "src/string_hash_map.c",
    "src/utils.c",
    "src/print_utils.c",
    "src/utf8_utils.c",
    "src/compute_moved_lines.c",
    "vendor/utf8proc.c",
];

pub fn run() -> Result<()> {
    let root = crate::workspace_root();
    lock::require_vendored(&root)?;

    let pairs_dir = lock::test_pairs_dir(&root);
    if !pairs_dir.is_dir() {
        bail!(
            "no oracle fixtures at {}.\nRun: cargo xtask sync-c --tag <tag>",
            pairs_dir.display()
        );
    }

    let tool = build_oracle(&root).context("building upstream diff_tool")?;

    let mut pairs: Vec<PathBuf> = std::fs::read_dir(&pairs_dir)?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    pairs.sort();

    if pairs.is_empty() {
        bail!("no fixture directories under {}", pairs_dir.display());
    }

    let mut failures = 0;
    let name_width = pairs
        .iter()
        .filter_map(|p| p.file_name().map(|n| n.to_string_lossy().len()))
        .max()
        .unwrap_or(20);

    for pair in &pairs {
        let name = pair
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();

        match compare(&tool, pair) {
            Ok(()) => println!("  {name:<name_width$}  PASS"),
            Err(err) => {
                failures += 1;
                println!("  {name:<name_width$}  FAIL");
                for line in format!("{err:?}").lines() {
                    println!("      {line}");
                }
            }
        }
    }

    println!();
    if failures > 0 {
        bail!(
            "{failures} of {} fixture(s) disagree with the oracle",
            pairs.len()
        );
    }
    println!(
        "verify-oracle: {} fixture(s) match upstream diff_tool exactly",
        pairs.len()
    );
    Ok(())
}

fn compare(tool: &Path, pair: &Path) -> Result<()> {
    let original_path = pair.join("original.txt");
    let modified_path = pair.join("modified.txt");
    if !original_path.is_file() || !modified_path.is_file() {
        bail!(
            "expected original.txt and modified.txt in {}",
            pair.display()
        );
    }

    let output = Command::new(tool)
        .arg(&original_path)
        .arg(&modified_path)
        .output()
        .context("running diff_tool")?;
    if !output.status.success() {
        bail!(
            "diff_tool failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    let expected = oracle_output::parse(&String::from_utf8_lossy(&output.stdout))?;

    let original = std::fs::read_to_string(&original_path)?;
    let modified = std::fs::read_to_string(&modified_path)?;
    // diff_tool splits on '\n' only and keeps a trailing empty line, matching
    // JavaScript's String.split. `str::lines` does neither, so it must not be
    // used here or the two sides would see different input.
    let original = vscode_diff::lines(&original);
    let modified = vscode_diff::lines(&modified);

    // diff_tool computes moves and uses a 5s budget; match it exactly.
    let options = Options::default().with_moves().with_time_budget_ms(5_000);
    let actual = compute(&original, &modified, &options)?;

    let mismatch = describe_mismatch(&expected, &actual);
    if let Some(detail) = mismatch {
        bail!("{detail}");
    }
    Ok(())
}

fn describe_mismatch(expected: &oracle_output::OracleDiff, actual: &LinesDiff) -> Option<String> {
    if expected.hit_timeout != actual.hit_timeout {
        return Some(format!(
            "hit_timeout: oracle {} vs ours {}",
            expected.hit_timeout, actual.hit_timeout
        ));
    }
    if expected.changes.len() != actual.changes.len() {
        return Some(format!(
            "change count: oracle {} vs ours {}",
            expected.changes.len(),
            actual.changes.len()
        ));
    }
    if expected.moves.len() != actual.moves.len() {
        return Some(format!(
            "move count: oracle {} vs ours {}",
            expected.moves.len(),
            actual.moves.len()
        ));
    }

    for (i, (want, got)) in expected.changes.iter().zip(&actual.changes).enumerate() {
        if let Some(detail) = change_mismatch(i, want, got) {
            return Some(detail);
        }
    }

    for (i, (want, got)) in expected.moves.iter().zip(&actual.moves).enumerate() {
        let ours = (
            got.original.start_line,
            got.original.end_line,
            got.modified.start_line,
            got.modified.end_line,
        );
        if *want != ours {
            return Some(format!("move {i}: oracle {want:?} vs ours {ours:?}"));
        }
    }

    None
}

fn change_mismatch(
    i: usize,
    want: &OracleChange,
    got: &vscode_diff::DetailedLineRangeMapping,
) -> Option<String> {
    let ours = (got.original.start_line, got.original.end_line);
    if want.original != ours {
        return Some(format!(
            "change {i} original: oracle {:?} vs ours {ours:?}",
            want.original
        ));
    }
    let ours = (got.modified.start_line, got.modified.end_line);
    if want.modified != ours {
        return Some(format!(
            "change {i} modified: oracle {:?} vs ours {ours:?}",
            want.modified
        ));
    }
    if want.inner_changes.len() != got.inner_changes.len() {
        return Some(format!(
            "change {i} inner count: oracle {} vs ours {}",
            want.inner_changes.len(),
            got.inner_changes.len()
        ));
    }
    for (j, (want_inner, got_inner)) in want
        .inner_changes
        .iter()
        .zip(&got.inner_changes)
        .enumerate()
    {
        let ours = OracleInner {
            original: (
                got_inner.original.start_line,
                got_inner.original.start_col,
                got_inner.original.end_line,
                got_inner.original.end_col,
            ),
            modified: (
                got_inner.modified.start_line,
                got_inner.modified.start_col,
                got_inner.modified.end_line,
                got_inner.modified.end_col,
            ),
        };
        if *want_inner != ours {
            return Some(format!(
                "change {i} inner {j}: oracle {want_inner:?} vs ours {ours:?}"
            ));
        }
    }
    None
}

/// Compiles `diff_tool` from the vendored sources into `target/oracle/`.
///
/// Built here rather than by a build script because it is a development tool,
/// not part of any shipped crate.
fn build_oracle(root: &Path) -> Result<PathBuf> {
    let engine = lock::engine_dir(root);
    let out_dir = root.join("target").join("oracle");
    std::fs::create_dir_all(&out_dir)?;

    let version = std::fs::read_to_string(lock::vendor_dir(root).join("VERSION"))?;
    write_version_header(&engine, &out_dir, version.trim())?;

    let exe = out_dir.join(if cfg!(windows) {
        "diff_tool.exe"
    } else {
        "diff_tool"
    });

    let compiler = std::env::var("CC").unwrap_or_else(|_| "cc".to_owned());
    let mut command = Command::new(&compiler);
    command
        .arg("-O1")
        .arg("-w")
        .arg(format!("-I{}", engine.join("include").display()))
        .arg(format!("-I{}", engine.join("vendor").display()))
        .arg(format!("-I{}", out_dir.display()))
        .arg("-DUTF8PROC_STATIC")
        .arg("-o")
        .arg(&exe)
        .arg(engine.join("diff_tool.c"));
    for source in SOURCES {
        command.arg(engine.join(source));
    }
    command.arg("-lm");

    let status = command
        .status()
        .with_context(|| format!("running {compiler}; is a C compiler installed?"))?;
    if !status.success() {
        bail!("compiling diff_tool failed");
    }
    Ok(exe)
}

/// Reproduces the substitution CMake performs on `version.h.in`.
fn write_version_header(engine: &Path, out_dir: &Path, version: &str) -> Result<()> {
    let base: String = version
        .split('.')
        .take(3)
        .map(|part| {
            part.chars()
                .take_while(char::is_ascii_digit)
                .collect::<String>()
        })
        .collect::<Vec<_>>()
        .join(".");
    let template = std::fs::read_to_string(engine.join("include/version.h.in"))?;
    std::fs::write(
        out_dir.join("version.h"),
        template.replace("@PROJECT_VERSION@", &base),
    )?;
    Ok(())
}
