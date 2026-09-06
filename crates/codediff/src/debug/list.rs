//! Prints the groups and files produced by a list request.

use anyhow::Result;
use vcs::DiffType;

/// Converts list flags and revisions into a VCS comparison type.
pub fn diff_type(rev: &[String], staged: bool) -> DiffType {
    use DiffType as Type;
    match (staged, rev.first().cloned(), rev.get(1).cloned()) {
        // `--staged` with no revision means against the last commit, which is
        // what `git diff --cached` means with no revision.
        (true, rev, _) => Type::Staged(rev.unwrap_or_else(|| "HEAD".to_owned())),
        (false, None, _) => Type::Worktree,
        (false, Some(a), Some(b)) => Type::Between(a, b),
        (false, Some(a), None) => match a.split_once("...") {
            Some((base, target)) => Type::MergeBase(base.to_owned(), target.to_owned()),
            None => Type::Against(a),
        },
    }
}

/// Prints each revision group and its files.
pub fn run(diff_type: DiffType, pathspec: Vec<String>) -> Result<()> {
    let cwd = std::env::current_dir()?;
    let root = vcs::Repository::open(&cwd)?.repo_path().root.clone();
    let request = pipeline::files::Request::new(root, diff_type).with_pathspec(pathspec);

    let mut groups: Vec<(file_types::Revs, Vec<file_types::File>)> = Vec::new();
    for file in pipeline::files::get_files(&request)? {
        let revs = file.revs();
        match groups.iter_mut().find(|(seen, _)| *seen == revs) {
            Some((_, files)) => files.push(file),
            None => groups.push((revs, vec![file])),
        }
    }

    for (revs, files) in groups {
        // The revisions, not only the name: a name is a label a human reads,
        // and what the group *is* is the pair.
        println!(
            "group {:?} {} -> {}",
            revs.heading(),
            revs.before,
            revs.after
        );
        for file in &files {
            let stats = match file.get_stats() {
                Some(stats) => format!(" +{} -{}", stats.added, stats.removed),
                None => String::new(),
            };
            println!(
                "  {} {}{stats}",
                super::status::letter(file.get_change_type()),
                file.path()
            );
        }
    }
    Ok(())
}
