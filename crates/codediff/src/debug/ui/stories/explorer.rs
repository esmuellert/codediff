use anyhow::Result;
use loom::crokey::{KeyCombination, key};

use super::super::definition::{LiveRepo, StoryComponent, StoryDefinition, StoryFixture};
use super::super::fixtures::explorer::ExplorerFixture;

const LIST_INITIAL_KEYS: &[KeyCombination] = &[key!(i)];
const FOLDED_INITIAL_KEYS: &[KeyCombination] = &[key!(j), key!(enter)];
const SELECTED_INITIAL_KEYS: &[KeyCombination] = &[key!(j), key!(j), key!(j)];
const DIRECTORY_INITIAL_KEYS: &[KeyCombination] = &[key!(j)];
const COMMIT_RANGE_INITIAL_KEYS: &[KeyCombination] = &[key!(j), key!(j), key!(j)];

pub const STORIES: &[StoryDefinition] = &[
    story("explorer/empty", "No changed files", &[], empty),
    story(
        "explorer/tree",
        "Nested changed files in tree mode",
        &[],
        standard_files,
    ),
    story(
        "explorer/list",
        "The same files in flat-list mode",
        LIST_INITIAL_KEYS,
        standard_files,
    ),
    story(
        "explorer/folded",
        "A directory collapsed through Enter",
        FOLDED_INITIAL_KEYS,
        standard_files,
    ),
    story(
        "explorer/selected",
        "A file selected through keyboard navigation",
        SELECTED_INITIAL_KEYS,
        standard_files,
    ),
    StoryDefinition {
        id: "explorer/cards",
        description: "Select a directory to review its changed files as cards",
        component: StoryComponent::ExplorerCards,
        compact: false,
        snapshot_size: (100, 24),
        initial_keys: DIRECTORY_INITIAL_KEYS,
        make_fixture: directory_cards,
    },
    story(
        "explorer/long-list",
        "Enough files to exercise viewport scrolling",
        &[],
        long_list,
    ),
    story(
        "explorer/mixed-status",
        "Every change type across working-tree and staged groups",
        &[],
        mixed_status,
    ),
    story(
        "explorer/awkward-paths",
        "Deep, repeated, spaced, long, and Unicode paths",
        &[],
        awkward_paths,
    ),
    StoryDefinition {
        id: "explorer/commit-range",
        description: "Commit-to-commit review across a real Git repository",
        component: StoryComponent::ExplorerCards,
        compact: false,
        snapshot_size: (120, 28),
        initial_keys: COMMIT_RANGE_INITIAL_KEYS,
        make_fixture: commit_range,
    },
];

const fn story(
    id: &'static str,
    description: &'static str,
    initial_keys: &'static [KeyCombination],
    make_fixture: fn() -> Result<StoryFixture>,
) -> StoryDefinition {
    StoryDefinition {
        id,
        description,
        component: StoryComponent::Explorer,
        compact: false,
        snapshot_size: (100, 24),
        initial_keys,
        make_fixture,
    }
}

fn empty() -> Result<StoryFixture> {
    Ok(StoryFixture::Explorer(ExplorerFixture::new().build()))
}

fn standard_files() -> Result<StoryFixture> {
    let files = ExplorerFixture::new()
        .changes(|group| {
            group
                .modified("src/app.rs", 8, 3)
                .modified("src/components/button.rs", 4, 1)
                .modified("tests/app_test.rs", 12, 0)
                .modified("README.md", 2, 2);
        })
        .build();
    Ok(StoryFixture::Explorer(files))
}

fn directory_cards() -> Result<StoryFixture> {
    let files = ExplorerFixture::new()
        .changes(|group| {
            group
                .modified("src/app.rs", 2, 1)
                .modified("src/button.rs", 1, 1)
                .modified("src/panel.rs", 3, 0);
        })
        .build();
    let mut diffs = Vec::with_capacity(files.len());
    for file in &files {
        let (original, modified): (&[&str], &[&str]) = match file.path().as_str() {
            "src/app.rs" => (
                &["fn app() {", "    let old_name = 1;", "}"],
                &["fn app() {", "    let new_name = 2;", "}"],
            ),
            "src/button.rs" => (
                &["pub fn button() {", "    old_button();", "}"],
                &["pub fn button() {", "    new_button();", "}"],
            ),
            "src/panel.rs" => (
                &["struct Panel {", "    old_field: bool,", "}"],
                &[
                    "struct Panel {",
                    "    new_field: bool,",
                    "    ready: bool,",
                    "}",
                ],
            ),
            _ => unreachable!("the directory-card fixture has a known file set"),
        };
        let changed = diff_service::diff::compute(
            original,
            modified,
            diff_service::diff::Settings::default(),
        )?;
        let alignment = diff_service::diff::align(changed, original, modified)?;
        diffs.push(diff_service::diff::DiffContent::Diff(
            diff_service::diff::Diff {
                file: file.clone(),
                alignment,
            },
        ));
    }
    Ok(StoryFixture::ExplorerCards { files, diffs })
}

fn long_list() -> Result<StoryFixture> {
    let files = ExplorerFixture::new()
        .changes(|group| {
            for number in 1..=40 {
                group.modified(&format!("story-{number:02}.rs"), number, number / 2);
            }
        })
        .build();
    Ok(StoryFixture::Explorer(files))
}

fn mixed_status() -> Result<StoryFixture> {
    let files = ExplorerFixture::new()
        .changes(|group| {
            group
                .modified("src/app.rs", 18, 7)
                .added("new feature.rs", 42)
                .deleted("docs/obsolete.md", 19)
                .untracked("notes/untracked draft.txt")
                .renamed("src/old_name.rs", "src/new_name.rs", 3, 2)
                .conflicted("src/conflict.rs");
        })
        .staged(|group| {
            group
                .modified("Cargo.toml", 1, 1)
                .added("migrations/20260904.sql", 120);
        })
        .build();
    Ok(StoryFixture::Explorer(files))
}

fn awkward_paths() -> Result<StoryFixture> {
    let files = ExplorerFixture::new()
        .changes(|group| {
            group
                .modified("src/中文.rs", 3, 1)
                .modified("docs/with spaces.rs", 7, 2)
                .modified("alpha/same-name.rs", 1, 1)
                .modified("beta/same-name.rs", 2, 2)
                .modified("one/two/three/four/five/deeply_nested.rs", 9, 4)
                .modified(
                    "a-very-long-directory-name/another-long-directory-name/a-very-long-file-name.rs",
                    15,
                    6,
                );
        })
        .build();
    Ok(StoryFixture::Explorer(files))
}

fn commit_range() -> Result<StoryFixture> {
    use std::process::Command;
    use std::sync::atomic::{AtomicUsize, Ordering};

    use anyhow::Context;
    use vcs::RevisionRange;

    static NEXT_ID: AtomicUsize = AtomicUsize::new(0);
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!(
        "codediff-story-commit-range-{}-{id}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("src")).context("creating commit-range story directory")?;
    let repo = LiveRepo::new(dir);

    let git = |args: &[&str], date: &str| -> Result<String> {
        let out = Command::new("git")
            .args(args)
            .current_dir(repo.path())
            .env("GIT_AUTHOR_NAME", "codediff fixtures")
            .env("GIT_AUTHOR_EMAIL", "fixture@codediff.test")
            .env("GIT_AUTHOR_DATE", date)
            .env("GIT_COMMITTER_NAME", "codediff fixtures")
            .env("GIT_COMMITTER_EMAIL", "fixture@codediff.test")
            .env("GIT_COMMITTER_DATE", date)
            .output()
            .with_context(|| format!("running git {}", args.join(" ")))?;
        if !out.status.success() {
            anyhow::bail!(
                "git {} failed: {}",
                args.join(" "),
                String::from_utf8_lossy(&out.stderr).trim()
            );
        }
        Ok(String::from_utf8_lossy(&out.stdout).trim().to_owned())
    };

    let write = |rel: &str, body: &str| -> Result<()> {
        let path = repo.path().join(rel);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, body)?;
        Ok(())
    };

    git(&["init", "-q", "-b", "main"], "2026-01-01T00:00:00Z")?;
    git(
        &["config", "core.autocrlf", "false"],
        "2026-01-01T00:00:00Z",
    )?;
    git(&["config", "diff.renames", "true"], "2026-01-01T00:00:00Z")?;

    let rename_before = r#"pub fn format_range(first: &str, second: &str) -> String {
    // Shared lines so Git scores this move above the rename threshold.
    let label = format!("{first}..{second}");
    let separator = "..";
    let has_separator = label.contains(separator);
    debug_assert!(has_separator);
    let count = label.len();
    format!("{label} ({count})")
}
"#;
    let rename_after = r#"pub fn format_range(first: &str, second: &str) -> String {
    // Shared lines so Git scores this move above the rename threshold.
    let label = format!("{first}..{second}");
    let separator = "..";
    let has_separator = label.contains(separator);
    debug_assert!(has_separator);
    let count = label.chars().count();
    format!("{label} ({count})")
}
"#;

    write(
        "src/engine.rs",
        r#"pub fn review_commits(limit: usize) -> usize {
    let before_count = 1;
    let window = limit + before_count;
    window
}
"#,
    )?;
    write("src/legacy.rs", rename_before)?;
    write(
        "README.md",
        "# Commit Review

Initial notes.
",
    )?;
    git(&["add", "-A"], "2026-01-01T00:00:00Z")?;
    git(
        &["commit", "-qm", "initial revision"],
        "2026-01-01T00:00:00Z",
    )?;
    let first = git(&["rev-parse", "HEAD"], "2026-01-01T00:00:00Z")?;

    write(
        "src/engine.rs",
        r#"pub fn review_commits(limit: usize) -> usize {
    let after_count = 2;
    let window = limit.saturating_add(after_count);
    window
}
"#,
    )?;
    write(
        "src/added.rs",
        r#"pub fn summarize_range(files: usize) -> String {
    format!("{files} changed files")
}
"#,
    )?;
    git(
        &["mv", "src/legacy.rs", "src/range.rs"],
        "2026-01-02T00:00:00Z",
    )?;
    write("src/range.rs", rename_after)?;
    git(&["rm", "-q", "README.md"], "2026-01-02T00:00:00Z")?;
    git(&["add", "-A"], "2026-01-02T00:00:00Z")?;
    git(
        &["commit", "-qm", "second revision"],
        "2026-01-02T00:00:00Z",
    )?;
    let second = git(&["rev-parse", "HEAD"], "2026-01-02T00:00:00Z")?;

    // Uncommitted working-tree edits stay absent from the commit-to-commit view.
    write(
        "dirty.rs",
        "pub fn uncommitted() {}
",
    )?;

    Ok(StoryFixture::Repository {
        repo,
        revision_range: RevisionRange::Between(first, second),
    })
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use file_types::ChangeType;

    use super::*;

    #[test]
    fn mixed_status_contains_every_change_type_and_both_groups() {
        let StoryFixture::Explorer(files) = mixed_status().unwrap() else {
            unreachable!()
        };
        let changes: Vec<ChangeType> = files
            .iter()
            .map(file_types::File::get_change_type)
            .collect();
        let headings: HashSet<&str> = files.iter().map(|file| file.revs().heading()).collect();

        for expected in [
            ChangeType::Modified,
            ChangeType::Added,
            ChangeType::Deleted,
            ChangeType::Untracked,
            ChangeType::Moved,
            ChangeType::Conflicted,
        ] {
            assert!(changes.contains(&expected), "missing {expected:?}");
        }
        assert_eq!(headings, HashSet::from(["Changes", "Staged Changes"]));
    }
}
