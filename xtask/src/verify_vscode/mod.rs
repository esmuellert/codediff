mod history;
mod output;
mod vscode;

use anyhow::{Result, bail};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DiffLayout {
    #[default]
    SideBySide,
    Inline,
}

impl DiffLayout {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SideBySide => "side-by-side",
            Self::Inline => "inline",
        }
    }
}

pub fn run(args: &[String]) -> Result<()> {
    let mut repo = None;
    let mut files = 10usize;
    let mut versions = 30usize;
    let mut max_lines = 2_000usize;
    let mut pair_count = None;
    let mut ignore_trim_whitespace = false;
    let mut layout = DiffLayout::default();
    let mut at = 0;
    while at < args.len() {
        match args[at].as_str() {
            "--files" => {
                at += 1;
                files = value(args, at, "--files")?;
            }
            "--versions" => {
                at += 1;
                versions = value(args, at, "--versions")?;
            }
            "--max-lines" => {
                at += 1;
                max_lines = value(args, at, "--max-lines")?;
            }
            "--pairs" => {
                at += 1;
                let count = value(args, at, "--pairs")?;
                if count == 0 {
                    bail!("--pairs must be greater than zero");
                }
                pair_count = Some(count);
            }
            "--ignore-trim-whitespace" => {
                at += 1;
                ignore_trim_whitespace = boolean(args, at, "--ignore-trim-whitespace")?;
            }
            "--layout" => {
                at += 1;
                layout = parse_layout(args, at)?;
            }
            arg if arg.starts_with('-') => bail!("unknown verify-vscode option: {arg}"),
            path if repo.is_none() => repo = Some(path.to_owned()),
            path => bail!("unexpected path: {path}"),
        }
        at += 1;
    }

    let root = crate::workspace_root();
    let repo = history::repository(repo.as_deref());
    let paths = output::OutputPaths::new(&root, layout);
    output::clear(&paths)?;
    let mut pairs = history::pairs(&repo, files, versions, max_lines)?;
    if let Some(count) = pair_count {
        if pairs.len() < count {
            bail!(
                "requested {count} pairs, but only {} were suitable",
                pairs.len()
            );
        }
        pairs.truncate(count);
    }
    let binary = output::build(&root)?;
    let workspace = &paths.workspace;
    let results = &paths.vscode_results;
    let mut materialised = Vec::new();
    let mut manifest = String::new();
    for pair in &pairs {
        let files = output::materialise(workspace, pair)?;
        let original = files
            .original
            .strip_prefix(workspace)?
            .to_string_lossy()
            .replace('\\', "/");
        let modified = files
            .modified
            .strip_prefix(workspace)?
            .to_string_lossy()
            .replace('\\', "/");
        manifest.push_str(&format!(
            "{}\t{}\t{}\t{}\t{}\n",
            pair.id,
            original,
            modified,
            vscode_diff::editor_lines(&pair.original).len(),
            vscode_diff::editor_lines(&pair.modified).len(),
        ));
        materialised.push(files);
    }
    std::fs::write(workspace.join("pairs.txt"), manifest)?;
    std::fs::write(
        workspace.join("options.json"),
        web_options(layout, ignore_trim_whitespace)?,
    )?;
    vscode::render(&root, workspace, results)?;

    let mut failures = Vec::new();
    let mut coverage = Coverage::default();
    println!(
        "verify-vscode: {} historical pair(s) from {} (layout: {}, ignore trim whitespace: {})",
        pairs.len(),
        repo.display(),
        layout.as_str(),
        ignore_trim_whitespace,
    );
    for (index, (pair, files)) in pairs.iter().zip(&materialised).enumerate() {
        let expected = std::fs::read_to_string(results.join(format!("{}.jsonl", pair.id)))?;
        let expected_records = output::parse(&expected)?;
        coverage.read_trim_whitespace(pair)?;
        coverage.read(&expected_records);
        let actual = output::codediff(&binary, files, layout, ignore_trim_whitespace)?;
        if expected_records == output::parse(&actual)? {
            println!("  {:>3}/{}  PASS  {}", index + 1, pairs.len(), pair.path);
        } else {
            let dir = output::save_mismatch(&paths.mismatches, pair, files, &expected, &actual)?;
            println!("  {:>3}/{}  FAIL  {}", index + 1, pairs.len(), pair.path);
            failures.push(dir);
        }
    }

    println!("coverage: {}", coverage.summary());
    if !failures.is_empty() {
        println!("mismatches:");
        for path in &failures {
            println!("  {}", path.display());
        }
        bail!(
            "{} of {} historical pair(s) differ from VS Code",
            failures.len(),
            pairs.len()
        );
    }
    println!("verify-vscode: every historical pair matches");
    Ok(())
}

fn web_options(layout: DiffLayout, ignore_trim_whitespace: bool) -> Result<Vec<u8>> {
    Ok(serde_json::to_vec_pretty(&serde_json::json!({
        "ignore_trim_whitespace": ignore_trim_whitespace,
        "layout": layout.as_str(),
    }))?)
}

fn value(args: &[String], at: usize, option: &str) -> Result<usize> {
    let Some(value) = args.get(at) else {
        bail!("{option} needs a number")
    };
    Ok(value.parse()?)
}

fn parse_layout(args: &[String], at: usize) -> Result<DiffLayout> {
    match args.get(at).map(String::as_str) {
        Some("side-by-side") => Ok(DiffLayout::SideBySide),
        Some("inline") => Ok(DiffLayout::Inline),
        Some(value) => bail!("--layout needs side-by-side or inline, got {value:?}"),
        None => bail!("--layout needs side-by-side or inline"),
    }
}

fn boolean(args: &[String], at: usize, option: &str) -> Result<bool> {
    let Some(value) = args.get(at) else {
        bail!("{option} needs true or false")
    };
    value
        .parse()
        .map_err(|_| anyhow::anyhow!("{option} needs true or false"))
}

#[derive(Default)]
struct Coverage {
    filler: bool,
    line: bool,
    whole: bool,
    range: bool,
    fill: bool,
    empty: bool,
    trim_sensitive: bool,
}

impl Coverage {
    fn read_trim_whitespace(&mut self, pair: &history::Pair) -> Result<()> {
        if self.trim_sensitive {
            return Ok(());
        }
        let original = vscode_diff::editor_lines(&pair.original);
        let modified = vscode_diff::editor_lines(&pair.modified);
        let strict = vscode_diff::compute(
            &original,
            &modified,
            &vscode_diff::Options::default().with_time_budget_ms(0),
        )?;
        let ignored = vscode_diff::compute(
            &original,
            &modified,
            &vscode_diff::Options::default()
                .ignoring_trim_whitespace()
                .with_time_budget_ms(0),
        )?;
        self.trim_sensitive = strict != ignored;
        Ok(())
    }

    fn read(&mut self, records: &[output::Record]) {
        for record in records {
            match record {
                output::Record::Row {
                    original, modified, ..
                } => {
                    self.filler |= original.is_none() || modified.is_none();
                }
                output::Record::Highlight {
                    line_background,
                    characters,
                    empty_markers,
                    ..
                } => {
                    self.line |= line_background.is_some();
                    self.empty |= !empty_markers.is_empty();
                    for character in characters {
                        self.whole |= character.start == 0 && character.fill_to_edge;
                        self.fill |= character.start > 0 && character.fill_to_edge;
                        self.range |= !character.fill_to_edge;
                    }
                }
            }
        }
    }

    fn summary(&self) -> String {
        [
            ("filler", self.filler),
            ("line", self.line),
            ("whole", self.whole),
            ("range", self.range),
            ("line-break", self.fill),
            ("empty", self.empty),
            ("trim-whitespace", self.trim_sensitive),
        ]
        .into_iter()
        .map(|(name, seen)| format!("{name}={}", if seen { "yes" } else { "no" }))
        .collect::<Vec<_>>()
        .join(" ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_names_are_shared_with_the_cli_contract() {
        let args = ["side-by-side".to_owned(), "inline".to_owned()];

        assert_eq!(parse_layout(&args, 0).unwrap(), DiffLayout::SideBySide);
        assert_eq!(parse_layout(&args, 1).unwrap(), DiffLayout::Inline);
        assert_eq!(DiffLayout::default().as_str(), "side-by-side");
    }

    #[test]
    fn web_options_carry_layout_and_whitespace_policy() {
        let value: serde_json::Value =
            serde_json::from_slice(&web_options(DiffLayout::Inline, true).unwrap()).unwrap();

        assert_eq!(value["layout"], "inline");
        assert_eq!(value["ignore_trim_whitespace"], true);
    }

    #[test]
    fn layout_rejects_unknown_and_missing_values() {
        assert!(parse_layout(&["stacked".to_owned()], 0).is_err());
        assert!(parse_layout(&[], 0).is_err());
    }
}
