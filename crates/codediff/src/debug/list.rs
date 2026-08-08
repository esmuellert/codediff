//! `codediff debug list` — the groups a request produces.
//!
//! The list pipeline, printed. `debug status` shows what git said; this shows
//! what the interface will be handed, grouped the way the interface groups it,
//! which is the only place the two can be compared.
//!
//! Machine-readable on purpose: one line per group and one per file, so a test
//! can assert on it without a terminal.

use anyhow::Result;
use vcs::DiffType;

/// The words `debug list` takes, as a diff type.
///
/// It lives here because **only this subcommand reaches it**. The command line
/// has no way to say any of it, and is not going to grow one: a reviewer
/// should not have to know git's revision syntax to open a review. `a...b` is
/// exact and almost nobody knows it. What compares against what is a decision
/// made *inside* the review, where it can be shown and changed, the way
/// lazygit does it. See D62.
///
/// The types themselves are real and reachable — the list pipeline resolves
/// all five — so the interface has something to switch between when the keys
/// for it arrive. This is how a test reaches them without a terminal.
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

/// Prints every group, and every file in it.
///
/// The pipeline answers flat, so the grouping happens here — by the revision
/// pair each file carries, which is the same read the interface makes and the
/// reason neither can disagree with the other.
pub fn run(diff_type: DiffType, pathspec: Vec<String>) -> Result<()> {
    let cwd = std::env::current_dir()?;
    let root = vcs::Repository::open(&cwd)?.repo_path().root.clone();
    let request = pipeline::list::Request::new(root, diff_type).with_pathspec(pathspec);

    let mut groups: Vec<(file_types::Revs, Vec<file_types::File>)> = Vec::new();
    for file in pipeline::list::run(&request)? {
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
