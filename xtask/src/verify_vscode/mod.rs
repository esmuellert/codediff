mod history;
mod output;
mod vscode;

use anyhow::{Result, bail};

pub fn run(args: &[String]) -> Result<()> {
    let mut repo = None;
    let mut files = 10usize;
    let mut versions = 30usize;
    let mut max_lines = 2_000usize;
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
            arg if arg.starts_with('-') => bail!("unknown verify-vscode option: {arg}"),
            path if repo.is_none() => repo = Some(path.to_owned()),
            path => bail!("unexpected path: {path}"),
        }
        at += 1;
    }

    let root = crate::workspace_root();
    let repo = history::repository(repo.as_deref());
    output::clear(&root)?;
    let pairs = history::pairs(&repo, files, versions, max_lines)?;
    let binary = output::build(&root)?;
    let workspace = root.join("target/vscode-parity/work");
    let results = root.join("target/vscode-parity/vscode");
    let mut materialised = Vec::new();
    let mut manifest = String::new();
    for pair in &pairs {
        let files = output::materialise(&root, pair)?;
        let original = files.original.strip_prefix(&workspace)?.to_string_lossy().replace('\\', "/");
        let modified = files.modified.strip_prefix(&workspace)?.to_string_lossy().replace('\\', "/");
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
    vscode::render(&root, &workspace, &results)?;

    let mut failures = Vec::new();
    let mut coverage = Coverage::default();
    println!("verify-vscode: {} historical pair(s) from {}", pairs.len(), repo.display());
    for (index, (pair, files)) in pairs.iter().zip(&materialised).enumerate() {
        let expected = std::fs::read_to_string(results.join(format!("{}.jsonl", pair.id)))?;
        let expected_records = output::parse(&expected)?;
        coverage.read(&expected_records);
        let actual = output::codediff(&binary, files)?;
        if expected_records == output::parse(&actual)? {
            println!("  {:>3}/{}  PASS  {}", index + 1, pairs.len(), pair.path);
        } else {
            let dir = output::save_mismatch(&root, pair, files, &expected, &actual)?;
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
        bail!("{} of {} historical pair(s) differ from VS Code", failures.len(), pairs.len());
    }
    println!("verify-vscode: every historical pair matches");
    Ok(())
}

fn value(args: &[String], at: usize, option: &str) -> Result<usize> {
    let Some(value) = args.get(at) else { bail!("{option} needs a number") };
    Ok(value.parse()?)
}

#[derive(Default)]
struct Coverage {
    filler: bool,
    line: bool,
    whole: bool,
    range: bool,
    fill: bool,
    empty: bool,
}

impl Coverage {
    fn read(&mut self, records: &[output::Record]) {
        for record in records {
            match record {
                output::Record::Row { original, modified, .. } => {
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
        ]
        .into_iter()
        .map(|(name, seen)| format!("{name}={}", if seen { "yes" } else { "no" }))
        .collect::<Vec<_>>()
        .join(" ")
    }
}
