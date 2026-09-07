use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::process::Command;

use super::DiffLayout;
use super::history::Pair;

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum Side {
    Original,
    Modified,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    Insert,
    Delete,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(deny_unknown_fields)]
pub struct Character {
    pub start: u32,
    pub end: Option<u32>,
    pub fill_to_edge: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
pub enum Record {
    Row {
        index: u32,
        original: Option<u32>,
        modified: Option<u32>,
    },
    Highlight {
        side: Side,
        line: u32,
        line_background: Option<Role>,
        gutter_background: Option<Role>,
        characters: Vec<Character>,
        empty_markers: Vec<u32>,
    },
}

pub struct Files {
    pub original: PathBuf,
    pub modified: PathBuf,
}

pub struct OutputPaths {
    pub workspace: PathBuf,
    pub vscode_results: PathBuf,
    pub mismatches: PathBuf,
}

impl OutputPaths {
    pub fn new(root: &Path, layout: DiffLayout) -> Self {
        let base = root.join("target/vscode-parity");
        let layout_root = base.join(layout.as_str());
        Self {
            workspace: layout_root.join("work"),
            vscode_results: layout_root.join("vscode"),
            mismatches: base.join("mismatches").join(layout.as_str()),
        }
    }
}

pub fn build(root: &Path) -> Result<PathBuf> {
    let jobs = std::thread::available_parallelism().map_or(1, |n| (n.get() / 2).max(1));
    let status = Command::new("cargo")
        .current_dir(root)
        .args(["build", "-j", &jobs.to_string(), "-p", "codediff"])
        .status()?;
    if !status.success() {
        bail!("building codediff failed");
    }
    let target = std::env::var_os("CARGO_TARGET_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| root.join("target"));
    Ok(target.join("debug").join(if cfg!(windows) {
        "codediff.exe"
    } else {
        "codediff"
    }))
}

pub fn materialise(workspace: &Path, pair: &Pair) -> Result<Files> {
    let dir = workspace.join(&pair.id);
    std::fs::create_dir_all(&dir)?;
    let original = dir.join(format!("{}-original.txt", pair.id));
    let modified = dir.join(format!("{}-modified.txt", pair.id));
    std::fs::write(&original, &pair.original)?;
    std::fs::write(&modified, &pair.modified)?;
    Ok(Files { original, modified })
}

pub fn codediff(
    binary: &Path,
    files: &Files,
    layout: DiffLayout,
    ignore_trim_whitespace: bool,
) -> Result<String> {
    let output = Command::new(binary)
        .args(["debug", "parity", "--layout", layout.as_str()])
        .arg(&files.original)
        .arg(&files.modified)
        .arg("--ignore-trim-whitespace")
        .arg(ignore_trim_whitespace.to_string())
        .output()?;
    if !output.status.success() {
        bail!(
            "codediff {} renderer failed: {}",
            layout.as_str(),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(String::from_utf8(output.stdout)?)
}

pub fn parse(text: &str) -> Result<Vec<Record>> {
    let mut records = Vec::new();
    for (index, line) in text.lines().enumerate() {
        let record: Record = serde_json::from_str(line)
            .with_context(|| format!("invalid rendering record on line {}", index + 1))?;
        validate(&record)?;
        records.push(record);
    }
    normalise(&mut records);
    Ok(records)
}

pub fn save_mismatch(
    mismatches: &Path,
    pair: &Pair,
    files: &Files,
    vscode: &str,
    codediff: &str,
) -> Result<PathBuf> {
    let dir = mismatches.join(&pair.id);
    if dir.exists() {
        std::fs::remove_dir_all(&dir)?;
    }
    std::fs::create_dir_all(&dir)?;
    std::fs::copy(&files.original, dir.join("original.txt"))?;
    std::fs::copy(&files.modified, dir.join("modified.txt"))?;
    std::fs::write(dir.join("vscode.jsonl"), vscode)?;
    std::fs::write(dir.join("codediff.jsonl"), codediff)?;
    std::fs::write(dir.join("difference.jsonl"), difference(vscode, codediff)?)?;
    std::fs::write(
        dir.join("revisions.txt"),
        format!(
            "path = {}\nolder = {}\nnewer = {}\n",
            pair.path, pair.older, pair.newer
        ),
    )?;
    Ok(dir)
}

pub fn clear(paths: &OutputPaths) -> Result<()> {
    for path in [&paths.workspace, &paths.vscode_results, &paths.mismatches] {
        if path.exists() {
            std::fs::remove_dir_all(path).context("clearing old parity output")?;
        }
    }
    Ok(())
}

fn validate(record: &Record) -> Result<()> {
    match record {
        Record::Row {
            original: None,
            modified: None,
            ..
        } => {
            bail!("a row cannot contain two fillers")
        }
        Record::Highlight {
            line_background,
            gutter_background,
            characters,
            empty_markers,
            ..
        } => {
            if line_background.is_none()
                && gutter_background.is_none()
                && characters.is_empty()
                && empty_markers.is_empty()
            {
                bail!("an empty highlight record is meaningless");
            }
            for character in characters {
                match (character.end, character.fill_to_edge) {
                    (None, false) => bail!("a character range without an end must fill to edge"),
                    (Some(_), true) => bail!("a character range that fills to edge has no end"),
                    (Some(end), false) if end < character.start => {
                        bail!("a character range ends before it starts")
                    }
                    _ => {}
                }
            }
        }
        _ => {}
    }
    Ok(())
}

fn normalise(records: &mut [Record]) {
    for record in records.iter_mut() {
        if let Record::Highlight {
            characters,
            empty_markers,
            ..
        } = record
        {
            characters.sort_unstable();
            empty_markers.sort_unstable();
        }
    }
    records.sort_unstable();
}

fn difference(expected: &str, actual: &str) -> Result<String> {
    use std::collections::BTreeSet;
    let expected: BTreeSet<_> = parse(expected)?.into_iter().collect();
    let actual: BTreeSet<_> = parse(actual)?.into_iter().collect();
    let mut out = String::new();
    for record in expected.difference(&actual) {
        out.push_str(&serde_json::to_string(&serde_json::json!({
            "only": "vscode",
            "record": record,
        }))?);
        out.push('\n');
    }
    for record in actual.difference(&expected) {
        out.push_str(&serde_json::to_string(&serde_json::json!({
            "only": "codediff",
            "record": record,
        }))?);
        out.push('\n');
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layouts_have_independent_work_and_mismatch_paths() {
        let root = Path::new("/repo");
        let side_by_side = OutputPaths::new(root, DiffLayout::SideBySide);
        let inline = OutputPaths::new(root, DiffLayout::Inline);

        assert_ne!(side_by_side.workspace, inline.workspace);
        assert_ne!(side_by_side.vscode_results, inline.vscode_results);
        assert_ne!(side_by_side.mismatches, inline.mismatches);
        assert!(side_by_side.workspace.ends_with("side-by-side/work"));
        assert!(inline.mismatches.ends_with("mismatches/inline"));
    }

    #[test]
    fn clearing_one_layout_preserves_the_other() {
        let root =
            std::env::temp_dir().join(format!("codediff-parity-paths-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let side_by_side = OutputPaths::new(&root, DiffLayout::SideBySide);
        let inline = OutputPaths::new(&root, DiffLayout::Inline);
        for path in [&side_by_side.workspace, &inline.workspace] {
            std::fs::create_dir_all(path).unwrap();
            std::fs::write(path.join("marker"), "present").unwrap();
        }

        clear(&inline).unwrap();

        assert!(side_by_side.workspace.join("marker").exists());
        assert!(!inline.workspace.exists());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn record_order_does_not_change_the_result() {
        let a = "{\"type\":\"row\",\"index\":1,\"original\":2,\"modified\":2}\n{\"type\":\"row\",\"index\":0,\"original\":1,\"modified\":1}\n";
        let b = "{\"type\":\"row\",\"index\":0,\"original\":1,\"modified\":1}\n{\"type\":\"row\",\"index\":1,\"original\":2,\"modified\":2}\n";
        assert_eq!(parse(a).unwrap(), parse(b).unwrap());
    }

    #[test]
    fn a_row_cannot_have_two_fillers() {
        let input = "{\"type\":\"row\",\"index\":0,\"original\":null,\"modified\":null}\n";
        assert!(parse(input).is_err());
    }

    #[test]
    fn fill_to_edge_requires_a_null_end() {
        let input = "{\"type\":\"highlight\",\"side\":\"modified\",\"line\":1,\"line_background\":\"insert\",\"gutter_background\":\"insert\",\"characters\":[{\"start\":2,\"end\":5,\"fill_to_edge\":true}],\"empty_markers\":[]}\n";
        assert!(parse(input).is_err());
    }
}
