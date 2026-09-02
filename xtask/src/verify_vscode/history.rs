use anyhow::{Context, Result, bail};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;

pub struct Pair {
    pub id: String,
    pub path: String,
    pub older: String,
    pub newer: String,
    pub original: String,
    pub modified: String,
}

pub fn pairs(
    repo: &Path,
    file_count: usize,
    versions: usize,
    max_lines: usize,
) -> Result<Vec<Pair>> {
    let repo = repo.canonicalize().context("finding repository")?;
    let base = base_ref(&repo)?;
    let mut counts = HashMap::<String, usize>::new();
    let names = text(&repo, &["log", &base, "--name-only", "--format="])?;
    for path in names.lines().filter(|line| !line.is_empty()) {
        *counts.entry(path.to_owned()).or_default() += 1;
    }

    let mut candidates: Vec<_> = counts
        .into_iter()
        .filter(|(_, count)| *count >= 2)
        .collect();
    candidates.sort_by(|(path_a, count_a), (path_b, count_b)| {
        count_b.cmp(count_a).then_with(|| path_a.cmp(path_b))
    });

    let mut selected = 0usize;
    let mut out = Vec::new();
    for (path, _) in candidates {
        if selected >= file_count {
            break;
        }
        if !exists(&repo, &format!("{base}:{path}")) {
            continue;
        }
        let limit = (versions + 1).to_string();
        let commits = text(
            &repo,
            &["log", &base, "-n", &limit, "--format=%H", "--", &path],
        )?;
        let commits: Vec<_> = commits.lines().collect();
        let Some(newer) = commits.first().copied() else {
            continue;
        };
        let Some(modified) = show(&repo, newer, &path)? else {
            continue;
        };
        if unsuitable(&modified, max_lines) {
            continue;
        }

        let before = out.len();
        for older in commits.iter().skip(1).take(versions) {
            let Some(original) = show(&repo, older, &path)? else {
                continue;
            };
            if unsuitable(&original, max_lines) || original == modified {
                continue;
            }
            let id = format!(
                "{}-{}-{}",
                safe(&path),
                &older[..8.min(older.len())],
                &newer[..8.min(newer.len())],
            );
            out.push(Pair {
                id,
                path: path.clone(),
                older: (*older).to_owned(),
                newer: newer.to_owned(),
                original,
                modified: modified.clone(),
            });
        }
        if out.len() > before {
            selected += 1;
        }
    }

    if out.is_empty() {
        bail!("no suitable historical file pairs found at {base}");
    }
    Ok(out)
}

fn base_ref(repo: &Path) -> Result<String> {
    for reference in ["origin/main", "origin/master", "HEAD"] {
        if exists(repo, reference) {
            return Ok(reference.to_owned());
        }
    }
    bail!("repository has no usable base revision")
}

fn unsuitable(text: &str, max_lines: usize) -> bool {
    text.as_bytes().contains(&0) || vscode_diff::editor_lines(text).len() > max_lines
}

fn show(repo: &Path, commit: &str, path: &str) -> Result<Option<String>> {
    let output = Command::new("git")
        .current_dir(repo)
        .args(["show", &format!("{commit}:{path}")])
        .output()?;
    if !output.status.success() {
        return Ok(None);
    }
    Ok(String::from_utf8(output.stdout).ok())
}

fn exists(repo: &Path, spec: &str) -> bool {
    Command::new("git")
        .current_dir(repo)
        .args(["cat-file", "-e", spec])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

fn text(repo: &Path, args: &[&str]) -> Result<String> {
    let output = Command::new("git").current_dir(repo).args(args).output()?;
    if !output.status.success() {
        bail!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(String::from_utf8(output.stdout)?)
}

fn safe(path: &str) -> String {
    path.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect()
}

pub fn repository(path: Option<&str>) -> PathBuf {
    path.map_or_else(crate::workspace_root, PathBuf::from)
}
