//! Turns raw filesystem events into a Refresh — pure logic, no IO.

use std::path::Path;

use ignore::gitignore::Gitignore;
use notify::EventKind;

use crate::Refresh;

pub struct Context {
    pub repo_root: std::path::PathBuf,
    pub git_dir: std::path::PathBuf,
    pub common_dir: std::path::PathBuf,
    pub ignorer: Gitignore,
}

/// Given a batch of debounced events, returns what needs refreshing.
pub fn get_refresh(events: &[notify::Event], ctx: &Context) -> Refresh {
    let mut out = Refresh::default();
    for event in events {
        for path in &event.paths {
            let r = refresh_for_path(path, event.kind, ctx);
            out = out.union(r);
        }
    }
    out
}

fn refresh_for_path(path: &Path, kind: EventKind, ctx: &Context) -> Refresh {
    // Read-only access (IN_OPEN, IN_CLOSE_NOWRITE) is not a change.
    if matches!(kind, EventKind::Access(_)) {
        tracing::trace!(?path, "skipped: read-only access");
        return Refresh::default();
    }

    // Skip .lock files — git renames foo.lock → foo atomically.
    if is_lock_file(path) {
        tracing::trace!(?path, "skipped: lock file");
        return Refresh::default();
    }

    // Path inside a git dir? The private one is asked first: in a worktree it
    // sits inside the common one, at .git/worktrees/<name>.
    if let Ok(rel) = path.strip_prefix(&ctx.git_dir) {
        return refresh_for_git_path(rel, path);
    }
    if let Ok(rel) = path.strip_prefix(&ctx.common_dir) {
        return refresh_for_git_path(rel, path);
    }

    // Path in worktree — check if ignored.
    if let Ok(rel) = path.strip_prefix(&ctx.repo_root) {
        let is_dir = path.is_dir();
        if ctx.ignorer.matched(rel, is_dir).is_ignore() {
            tracing::trace!(?path, "skipped: gitignored");
            return Refresh::default();
        }
        // Also check if any parent component is ignored (e.g. target/foo/bar
        // when target/ is in .gitignore).
        for ancestor in rel.ancestors().skip(1) {
            if ancestor == Path::new("") {
                break;
            }
            if ctx.ignorer.matched(ancestor, true).is_ignore() {
                tracing::trace!(?path, "skipped: parent gitignored");
                return Refresh::default();
            }
        }
        tracing::debug!(?path, "worktree change");
        return Refresh {
            worktree: true,
            ..Default::default()
        };
    }

    // Path outside repo — shouldn't happen, ignore.
    Refresh::default()
}

fn refresh_for_git_path(rel: &Path, full: &Path) -> Refresh {
    let first = rel.components().next().map(|c| c.as_os_str());

    // Skip internals that don't affect status.
    if matches!(
        first.and_then(|s| s.to_str()),
        Some("objects" | "logs" | "hooks" | "lfs" | "fsmonitor--daemon" | "info")
    ) {
        tracing::trace!(?full, "skipped: git internal");
        return Refresh::default();
    }

    let rel_str = rel.to_string_lossy();

    if rel_str == "index" {
        tracing::debug!("git index changed");
        return Refresh {
            index: true,
            ..Default::default()
        };
    }

    if rel_str == "HEAD" {
        tracing::debug!("git HEAD changed");
        return Refresh {
            head: true,
            ..Default::default()
        };
    }

    if rel_str.starts_with("refs") || rel_str == "packed-refs" {
        tracing::debug!(path = %rel_str, "git refs changed");
        return Refresh {
            refs: true,
            ..Default::default()
        };
    }

    // MERGE_HEAD, CHERRY_PICK_HEAD, REVERT_HEAD, REBASE_HEAD, etc.
    if rel_str.ends_with("_HEAD") || rel_str.starts_with("rebase-") {
        tracing::debug!(path = %rel_str, "git operation state changed");
        return Refresh {
            index: true,
            ..Default::default()
        };
    }

    tracing::trace!(?full, "skipped: unrecognised git path");
    Refresh::default()
}

fn is_lock_file(path: &Path) -> bool {
    path.extension().is_some_and(|e| e == "lock")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn ctx(root: &str) -> Context {
        let repo_root = PathBuf::from(root);
        let git_dir = repo_root.join(".git");
        let (ignorer, _) = Gitignore::new(repo_root.join(".gitignore"));
        Context {
            repo_root,
            common_dir: git_dir.clone(),
            git_dir,
            ignorer,
        }
    }

    /// A linked worktree at /wt, whose main repository is /repo.
    fn worktree_ctx() -> Context {
        let repo_root = PathBuf::from("/wt");
        let common_dir = PathBuf::from("/repo/.git");
        let (ignorer, _) = Gitignore::new(repo_root.join(".gitignore"));
        Context {
            repo_root,
            git_dir: common_dir.join("worktrees/wt"),
            common_dir,
            ignorer,
        }
    }

    fn ctx_with_ignore(root: &str, patterns: &[&str]) -> Context {
        let repo_root = PathBuf::from(root);
        let git_dir = repo_root.join(".git");
        let mut builder = ignore::gitignore::GitignoreBuilder::new(&repo_root);
        for p in patterns {
            builder.add_line(None, p).unwrap();
        }
        let ignorer = builder.build().unwrap();
        Context {
            repo_root,
            common_dir: git_dir.clone(),
            git_dir,
            ignorer,
        }
    }

    fn event(kind: EventKind, path: &str) -> notify::Event {
        notify::Event {
            kind,
            paths: vec![PathBuf::from(path)],
            attrs: Default::default(),
        }
    }

    #[test]
    fn worktree_file_change_sets_worktree() {
        let c = ctx("/repo");
        let r = get_refresh(
            &[event(
                EventKind::Modify(notify::event::ModifyKind::Data(
                    notify::event::DataChange::Any,
                )),
                "/repo/src/main.rs",
            )],
            &c,
        );
        assert!(r.worktree);
        assert!(!r.index && !r.head && !r.refs);
    }

    #[test]
    fn lock_file_is_ignored() {
        let c = ctx("/repo");
        let r = get_refresh(
            &[event(
                EventKind::Create(notify::event::CreateKind::File),
                "/repo/.git/index.lock",
            )],
            &c,
        );
        assert!(r.is_empty());
    }

    #[test]
    fn git_index_sets_index() {
        let c = ctx("/repo");
        let r = get_refresh(
            &[event(
                EventKind::Modify(notify::event::ModifyKind::Any),
                "/repo/.git/index",
            )],
            &c,
        );
        assert!(r.index);
        assert!(!r.worktree);
    }

    #[test]
    fn git_head_sets_head() {
        let c = ctx("/repo");
        let r = get_refresh(
            &[event(
                EventKind::Modify(notify::event::ModifyKind::Any),
                "/repo/.git/HEAD",
            )],
            &c,
        );
        assert!(r.head);
    }

    #[test]
    fn git_refs_sets_refs() {
        let c = ctx("/repo");
        let r = get_refresh(
            &[event(
                EventKind::Modify(notify::event::ModifyKind::Any),
                "/repo/.git/refs/heads/main",
            )],
            &c,
        );
        assert!(r.refs);
    }

    #[test]
    fn packed_refs_sets_refs() {
        let c = ctx("/repo");
        let r = get_refresh(
            &[event(
                EventKind::Modify(notify::event::ModifyKind::Any),
                "/repo/.git/packed-refs",
            )],
            &c,
        );
        assert!(r.refs);
    }

    #[test]
    fn git_objects_is_skipped() {
        let c = ctx("/repo");
        let r = get_refresh(
            &[event(
                EventKind::Create(notify::event::CreateKind::File),
                "/repo/.git/objects/ab/cdef1234",
            )],
            &c,
        );
        assert!(r.is_empty());
    }

    #[test]
    fn git_logs_is_skipped() {
        let c = ctx("/repo");
        let r = get_refresh(
            &[event(
                EventKind::Modify(notify::event::ModifyKind::Any),
                "/repo/.git/logs/HEAD",
            )],
            &c,
        );
        assert!(r.is_empty());
    }

    #[test]
    fn git_hooks_is_skipped() {
        let c = ctx("/repo");
        let r = get_refresh(
            &[event(
                EventKind::Modify(notify::event::ModifyKind::Any),
                "/repo/.git/hooks/pre-commit",
            )],
            &c,
        );
        assert!(r.is_empty());
    }

    #[test]
    fn gitignored_path_is_skipped() {
        let c = ctx_with_ignore("/repo", &["target/"]);
        let r = get_refresh(
            &[event(
                EventKind::Modify(notify::event::ModifyKind::Any),
                "/repo/target/debug/binary",
            )],
            &c,
        );
        assert!(r.is_empty());
    }

    #[test]
    fn multiple_events_coalesce() {
        let c = ctx("/repo");
        let events = vec![
            event(
                EventKind::Modify(notify::event::ModifyKind::Any),
                "/repo/src/lib.rs",
            ),
            event(
                EventKind::Modify(notify::event::ModifyKind::Any),
                "/repo/.git/index",
            ),
        ];
        let r = get_refresh(&events, &c);
        assert!(r.worktree && r.index);
    }

    #[test]
    fn merge_head_sets_index() {
        let c = ctx("/repo");
        let r = get_refresh(
            &[event(
                EventKind::Create(notify::event::CreateKind::File),
                "/repo/.git/MERGE_HEAD",
            )],
            &c,
        );
        assert!(r.index);
    }

    #[test]
    fn read_only_access_triggers_nothing() {
        let c = ctx("/repo");
        // Opening a file for reading (IN_OPEN) must not trigger a refresh.
        let r = get_refresh(
            &[event(
                EventKind::Access(notify::event::AccessKind::Open(
                    notify::event::AccessMode::Any,
                )),
                "/repo/.git/HEAD",
            )],
            &c,
        );
        assert!(r.is_empty(), "read-only access should not refresh, got {r}");
    }

    #[test]
    fn read_only_access_on_worktree_triggers_nothing() {
        let c = ctx("/repo");
        let r = get_refresh(
            &[event(
                EventKind::Access(notify::event::AccessKind::Open(
                    notify::event::AccessMode::Any,
                )),
                "/repo/src/main.rs",
            )],
            &c,
        );
        assert!(r.is_empty(), "read-only access should not refresh, got {r}");
    }

    // === Linked worktrees ===

    #[test]
    fn worktree_private_index_sets_index() {
        let c = worktree_ctx();
        let r = get_refresh(
            &[event(
                EventKind::Modify(notify::event::ModifyKind::Any),
                "/repo/.git/worktrees/wt/index",
            )],
            &c,
        );
        assert!(r.index, "the private index is this worktree's, got {r}");
        assert!(!r.worktree);
    }

    #[test]
    fn worktree_private_head_sets_head() {
        let c = worktree_ctx();
        let r = get_refresh(
            &[event(
                EventKind::Modify(notify::event::ModifyKind::Any),
                "/repo/.git/worktrees/wt/HEAD",
            )],
            &c,
        );
        assert!(r.head, "the private HEAD is this worktree's, got {r}");
    }

    #[test]
    fn worktree_shared_refs_sets_refs() {
        let c = worktree_ctx();
        let r = get_refresh(
            &[event(
                EventKind::Modify(notify::event::ModifyKind::Any),
                "/repo/.git/refs/heads/main",
            )],
            &c,
        );
        assert!(r.refs, "refs live in the shared dir, got {r}");
    }

    #[test]
    fn worktree_shared_packed_refs_sets_refs() {
        let c = worktree_ctx();
        let r = get_refresh(
            &[event(
                EventKind::Modify(notify::event::ModifyKind::Any),
                "/repo/.git/packed-refs",
            )],
            &c,
        );
        assert!(r.refs, "packed-refs lives in the shared dir, got {r}");
    }

    #[test]
    fn linked_worktree_file_change_sets_worktree() {
        let c = worktree_ctx();
        let r = get_refresh(
            &[event(
                EventKind::Modify(notify::event::ModifyKind::Any),
                "/wt/src/main.rs",
            )],
            &c,
        );
        assert!(r.worktree);
        assert!(!r.index && !r.head && !r.refs);
    }

    #[test]
    fn worktree_shared_objects_are_skipped() {
        let c = worktree_ctx();
        let r = get_refresh(
            &[event(
                EventKind::Create(notify::event::CreateKind::File),
                "/repo/.git/objects/ab/cdef1234",
            )],
            &c,
        );
        assert!(r.is_empty());
    }
}
