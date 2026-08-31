//! Loads repository ignore rules and detects events that require reloading them.

use std::path::Path;

use ignore::WalkBuilder;
use ignore::gitignore::{Gitignore, GitignoreBuilder};

pub(super) fn requires_reload(
    event: &notify::Event,
    repo_root: &Path,
    common_git_dir: &Path,
) -> bool {
    if matches!(event.kind, notify::EventKind::Access(_)) {
        return false;
    }
    let exclude_path = common_git_dir.join("info/exclude");
    event.paths.iter().any(|path| {
        path.strip_prefix(repo_root).is_ok_and(|relative| {
            relative
                .file_name()
                .is_some_and(|name| name == ".gitignore")
        }) || path == &exclude_path
    })
}

pub(super) fn build_matcher(repo_root: &Path, common_git_dir: &Path) -> Gitignore {
    let mut builder = GitignoreBuilder::new(repo_root);
    let root_rules = repo_root.join(".gitignore");
    if root_rules.exists() {
        let _ = builder.add(&root_rules);
    }

    for entry in WalkBuilder::new(repo_root)
        .hidden(false)
        .git_ignore(true)
        .git_global(true)
        .git_exclude(true)
        .parents(true)
        .filter_entry(|entry| entry.file_name() != ".git")
        .build()
        .filter_map(Result::ok)
    {
        let path = entry.path();
        if path != root_rules && path.file_name().is_some_and(|name| name == ".gitignore") {
            let _ = builder.add(path);
        }
    }

    let exclude_path = common_git_dir.join("info/exclude");
    if exclude_path.exists() {
        let _ = builder.add(&exclude_path);
    }
    builder.build().unwrap_or_else(|_| {
        let (matcher, _) = Gitignore::new(root_rules);
        matcher
    })
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn nested_gitignore_change_requires_reload() {
        let event = notify::Event {
            kind: notify::EventKind::Any,
            paths: vec![PathBuf::from("/repo/nested/.gitignore")],
            attrs: Default::default(),
        };
        assert!(requires_reload(
            &event,
            Path::new("/repo"),
            Path::new("/repo/.git")
        ));
    }

    #[test]
    fn reading_gitignore_does_not_require_reload() {
        let event = notify::Event {
            kind: notify::EventKind::Access(notify::event::AccessKind::Open(
                notify::event::AccessMode::Any,
            )),
            paths: vec![PathBuf::from("/repo/.gitignore")],
            attrs: Default::default(),
        };
        assert!(!requires_reload(
            &event,
            Path::new("/repo"),
            Path::new("/repo/.git")
        ));
    }

    #[test]
    fn ordinary_file_change_does_not_require_ignore_reload() {
        let event = notify::Event {
            kind: notify::EventKind::Any,
            paths: vec![PathBuf::from("/repo/file.txt")],
            attrs: Default::default(),
        };
        assert!(!requires_reload(
            &event,
            Path::new("/repo"),
            Path::new("/repo/.git")
        ));
    }
}
