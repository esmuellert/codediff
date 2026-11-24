//! Computes and aligns file diffs from loaded file contents.

use std::sync::Arc;

use align::Alignment;
use anyhow::{Context, Result, bail};
use diff_types::LinesDiff;
use file_types::{DiffVersion, File, FileContent};
use vscode_diff::editor_lines;

/// User-visible options needed by the file diff engine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Settings {
    pub ignore_trim_whitespace: bool,
}

/// What diffing produces for one file.
#[derive(Debug, Clone)]
pub enum DiffContent {
    SingleFile(SingleFile),
    Diff(Diff),
}

// A new diff response should always trigger a re-render.
impl PartialEq for DiffContent {
    fn eq(&self, _: &Self) -> bool {
        false
    }
}

/// A file with only one version (added, untracked, or deleted).
#[derive(Debug, Clone)]
pub struct SingleFile {
    pub file: File,
    pub lines: Arc<Vec<String>>,
}

/// Two versions of a file, paired line by line.
#[derive(Debug, Clone)]
pub struct Diff {
    pub file: File,
    pub alignment: Alignment,
}

impl DiffContent {
    /// Builds a [`DiffContent`] from a file and its loaded contents on both sides.
    pub fn new(
        file: File,
        original: &FileContent,
        modified: &FileContent,
        settings: Settings,
    ) -> Result<Self> {
        if original.is_binary() || modified.is_binary() {
            bail!("{} is binary — there are no lines to review", file.path());
        }
        match file.is_one_sided() {
            // Nothing to compare against, so neither two columns nor an
            // interleaving has anything to say.
            Some(version) => {
                let content = match version {
                    DiffVersion::Original => original,
                    DiffVersion::Modified => modified,
                };
                let lines = content
                    .text()
                    .map(editor_lines)
                    .unwrap_or_default()
                    .into_iter()
                    .map(str::to_owned)
                    .collect();
                Ok(Self::SingleFile(SingleFile {
                    file,
                    lines: Arc::new(lines),
                }))
            }
            None => {
                let before = original.text().map(editor_lines).unwrap_or_default();
                let after = modified.text().map(editor_lines).unwrap_or_default();
                tracing::info!(path = %file.path(), lines = after.len(), "computing diff");
                let changed =
                    compute(&before, &after, settings).with_context(|| file.path().to_string())?;
                let alignment =
                    align(changed, &before, &after).with_context(|| file.path().to_string())?;
                Ok(Self::Diff(Diff { file, alignment }))
            }
        }
    }

    /// Which file this is, whichever it is.
    pub fn file(&self) -> &File {
        match self {
            Self::SingleFile(single) => &single.file,
            Self::Diff(diff) => &diff.file,
        }
    }

    pub fn alignment(&self) -> Option<&Alignment> {
        match self {
            Self::Diff(diff) => Some(&diff.alignment),
            Self::SingleFile(_) => None,
        }
    }
}

impl SingleFile {
    /// Which side the file exists on.
    pub fn side(&self) -> DiffVersion {
        self.file.is_one_sided().unwrap_or(DiffVersion::Modified)
    }
}

/// Calls the C engine with application settings. Only for files with two sides.
pub fn compute(before: &[&str], after: &[&str], settings: Settings) -> Result<LinesDiff> {
    let options = vscode_diff::Options {
        ignore_trim_whitespace: settings.ignore_trim_whitespace,
        ..Default::default()
    };
    vscode_diff::compute(before, after, &options).context("computing the diff")
}

/// Pairs lines up from a diff result.
pub fn align(diff: LinesDiff, before: &[&str], after: &[&str]) -> Result<Alignment> {
    Alignment::try_new(diff, before, after)
        .map_err(|_| anyhow::anyhow!("the diff does not describe these two files"))
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use file_types::{CommitHash, DiffVersion, File, FileContent, RepoPath, Revs};

    use super::{DiffContent, Settings, compute};

    fn revs() -> Revs {
        Revs::worktree_against(CommitHash::new("base"))
    }

    fn path(rel: &str) -> RepoPath {
        RepoPath::new(rel, Path::new("/repo"))
    }

    #[test]
    fn production_keeps_leading_and_trailing_whitespace_changes() {
        let diff = compute(&["  value"], &["value  "], Settings::default()).unwrap();

        assert!(!diff.is_empty());
    }

    #[test]
    fn new_rejects_binary_content_on_either_side() {
        let file = File::unchanged_path(path("picture.png"), revs());
        let binary = FileContent::Binary { bytes: 16 };
        let text = FileContent::Text("plain\n".to_owned());

        let err = DiffContent::new(file.clone(), &binary, &text, Settings::default()).unwrap_err();
        assert_eq!(
            err.to_string(),
            "picture.png is binary — there are no lines to review"
        );

        let err = DiffContent::new(file, &text, &binary, Settings::default()).unwrap_err();
        assert_eq!(
            err.to_string(),
            "picture.png is binary — there are no lines to review"
        );
    }

    #[test]
    fn new_produces_single_file_for_added_and_deleted_files() {
        let added = File::added(path("added.rs"), revs());
        let content = DiffContent::new(
            added,
            &FileContent::Absent,
            &FileContent::Text("fn added() {}\r\n".to_owned()),
            Settings::default(),
        )
        .unwrap();
        let DiffContent::SingleFile(single) = content else {
            panic!("expected SingleFile");
        };
        assert_eq!(single.side(), DiffVersion::Modified);
        assert_eq!(*single.lines, vec!["fn added() {}", ""]);

        let deleted = File::deleted(path("deleted.rs"), revs());
        let content = DiffContent::new(
            deleted,
            &FileContent::Text("gone".to_owned()),
            &FileContent::Absent,
            Settings::default(),
        )
        .unwrap();
        let DiffContent::SingleFile(single) = content else {
            panic!("expected SingleFile");
        };
        assert_eq!(single.side(), DiffVersion::Original);
        assert_eq!(*single.lines, vec!["gone"]);
    }

    #[test]
    fn new_aligns_two_sided_files_and_handles_empty_text() {
        let file = File::unchanged_path(path("modified.rs"), revs());
        let content = DiffContent::new(
            file,
            &FileContent::Text("one\ntwo\n".to_owned()),
            &FileContent::Text("one\nTWO\n".to_owned()),
            Settings::default(),
        )
        .unwrap();
        let alignment = content.alignment().expect("two-sided alignment");
        assert_eq!(alignment.changes().len(), 1);
        assert_eq!(alignment.line(DiffVersion::Original, 2), Some("two"));
        assert_eq!(alignment.line(DiffVersion::Modified, 2), Some("TWO"));
    }
}
