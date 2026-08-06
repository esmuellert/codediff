//! Git repositories in known states, for tests and for looking at by hand.
//!
//! Built by `cargo xtask fixture-repo <dir>`, and used directly by `vcs`'s
//! tests. It lives in its own crate with **no workspace dependencies** so that
//! any crate can dev-depend on it without forming a cycle.
//!
//! Emits the repository *and* a manifest of what git should say about it, so a
//! test compares parsed output against a file a human wrote rather than against
//! output the code produced.
//!
//! Every case here is one that has broken a real diff tool: a rename that looks
//! like an add plus a delete, a path with a layout, a path outside ASCII, a file
//! both staged and edited again, an unresolved merge, CRLF, and a file with no
//! trailing newline.

use std::path::Path;
use std::process::Command;

use std::io::{Error, Result};

pub fn repo(dir: &Path) -> Result<()> {
    if dir.exists() {
        std::fs::remove_dir_all(dir)?;
    }
    std::fs::create_dir_all(dir)?;

    git(dir, &["init", "-q", "-b", "main"])?;
    git(dir, &["config", "user.email", "fixture@codediff.test"])?;
    git(dir, &["config", "user.name", "codediff fixtures"])?;
    // Renames are only reported when git looks for them.
    git(dir, &["config", "diff.renames", "true"])?;

    // ---- the committed state -------------------------------------------
    write(dir, "unchanged.txt", "this file never changes\n")?;
    write(dir, "modified.txt", "one\ntwo\nthree\n")?;
    write(dir, "deleted.txt", "goes away\n")?;
    write(dir, "renamed-from.txt", RENAME_BODY)?;
    write(dir, "staged-then-edited.txt", "first\n")?;
    write(dir, "with spaces.txt", "a path containing spaces\n")?;
    write(dir, "ünïcodé-ファイル.txt", "a path outside ASCII\n")?;
    write(dir, "nested/deep/file.txt", "nested\n")?;
    write(dir, "crlf.txt", "one\r\ntwo\r\n")?;
    write(dir, "no-trailing-newline.txt", "last line has no newline")?;
    write(dir, "conflict.txt", "base\n")?;
    // A file that is not text: `before`/`after` hand back bytes, and a picture
    // has no lines to align.
    write_bytes(dir, "picture.png", PNG)?;
    write(dir, "gains-a-line.txt", "one\ntwo\n")?;
    write(
        dir,
        ".gitignore",
        "ignored.txt\nignored-dir/\nMANIFEST.txt\n",
    )?;
    git(dir, &["add", "-A"])?;
    git(dir, &["commit", "-qm", "the committed state"])?;

    // ---- a merge left unresolved ---------------------------------------
    git(dir, &["checkout", "-q", "-b", "other"])?;
    write(dir, "conflict.txt", "theirs\n")?;
    git(dir, &["commit", "-qam", "theirs"])?;
    git(dir, &["checkout", "-q", "main"])?;
    write(dir, "conflict.txt", "ours\n")?;
    git(dir, &["commit", "-qam", "ours"])?;
    // Expected to fail: that is the point.
    let _ = Command::new("git")
        .args(["merge", "other", "-q"])
        .current_dir(dir)
        .output();

    // ---- the working state ----------------------------------------------
    write(dir, "modified.txt", "one\nTWO\nthree\n")?;
    git(dir, &["rm", "-q", "deleted.txt"])?;
    git(dir, &["mv", "renamed-from.txt", "renamed-to.txt"])?;

    // Staged, then edited again: the one file with two different status codes.
    write(dir, "staged-then-edited.txt", "second\n")?;
    git(dir, &["add", "staged-then-edited.txt"])?;
    write(dir, "staged-then-edited.txt", "second\nand third\n")?;

    // The awkward paths have to be *changed* to appear in status at all —
    // an unchanged file proves nothing about parsing NUL-separated output.
    write(
        dir,
        "with spaces.txt",
        "a path containing spaces, now edited\n",
    )?;
    write(
        dir,
        "ünïcodé-ファイル.txt",
        "a path outside ASCII, now edited\n",
    )?;
    write(dir, "crlf.txt", "one\r\ntwo\r\nthree\r\n")?;
    write(
        dir,
        "no-trailing-newline.txt",
        "last line still has no newline",
    )?;

    // Added and deleted files are where the engine's empty-side handling is
    // exercised for real: one side has no lines at all.
    write(dir, "gains-a-line.txt", "one\ntwo\nthree\n")?;
    write_bytes(dir, "picture.png", &{
        let mut edited = PNG.to_vec();
        edited.extend_from_slice(&[0x00, 0x99, 0x88]);
        edited
    })?;

    write(dir, "untracked.txt", "never added\n")?;
    write(
        dir,
        "untracked-dir/inside.txt",
        "in an untracked directory\n",
    )?;
    // A chain of directories with nothing to choose between, so a flattened
    // tree and an unflattened one are different pictures. Without it the two
    // cannot be told apart, and a broken flattener passes every test.
    write(
        dir,
        "deep/only/one/chain/leaf.txt",
        "at the end of a chain\n",
    )?;
    // A directory with two children, which must *not* flatten.
    write(dir, "nest/a/one.txt", "one\n")?;
    write(dir, "nest/b/two.txt", "two\n")?;
    write(dir, "nest/b/three.txt", "three\n")?;
    write(dir, "ignored.txt", "should not appear\n")?;
    write(dir, "ignored-dir/inside.txt", "should not appear either\n")?;

    manifest(dir)?;
    println!("fixture repository written to {}", dir.display());
    println!("manifest: {}", dir.join(MANIFEST).display());
    Ok(())
}

pub const MANIFEST: &str = "MANIFEST.txt";

/// Ninety percent similar to its source, so git reports a rename rather than an
/// unrelated add and delete.
const RENAME_BODY: &str = "\
fn moved() {
    // this body is long enough that git scores the move as a rename
    let a = 1;
    let b = 2;
    let c = 3;
    println!(\"{a} {b} {c}\");
}
";

/// What `git status --porcelain=v2` should report, written by hand.
///
/// Hand-written on purpose: a manifest generated from our own output would only
/// prove the parser is consistent with itself.
fn manifest(dir: &Path) -> Result<()> {
    let text = "\
# What `codediff debug status` must print for this repository.
#
#   index  worktree  path  [<- original]
#
# Written by hand from `git-status(1)`, not recorded from our own output.
# Sorted by path.

.  M  crlf.txt
.  M  gains-a-line.txt
U  U  conflict.txt
D  .  deleted.txt
.  M  modified.txt
.  M  picture.png
.  M  no-trailing-newline.txt
R  .  renamed-to.txt <- renamed-from.txt
M  M  staged-then-edited.txt
.  ?  deep/only/one/chain/leaf.txt
.  ?  nest/a/one.txt
.  ?  nest/b/three.txt
.  ?  nest/b/two.txt
.  ?  untracked-dir/inside.txt
.  ?  untracked.txt
.  M  with spaces.txt
.  M  ünïcodé-ファイル.txt

# Absent on purpose:
#   unchanged.txt          identical to HEAD
#   ignored.txt            matched by .gitignore
#   ignored-dir/inside.txt matched by .gitignore
#   nested/deep/file.txt   unchanged
#   MANIFEST.txt           this file, ignored so it does not report itself
";
    std::fs::write(dir.join(MANIFEST), text)?;
    Ok(())
}

/// The first bytes of a real PNG, including the zero byte that makes every
/// tool call it binary.
const PNG: &[u8] = &[
    0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a, 0x00, 0x00, 0x00, 0x0d, b'I', b'H', b'D', b'R',
];

fn write_bytes(dir: &Path, path: &str, body: &[u8]) -> Result<()> {
    let full = dir.join(path);
    if let Some(parent) = full.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&full, body)?;
    Ok(())
}

fn write(dir: &Path, path: &str, body: &str) -> Result<()> {
    let full = dir.join(path);
    if let Some(parent) = full.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&full, body)?;
    Ok(())
}

fn git(dir: &Path, args: &[&str]) -> Result<()> {
    let out = Command::new("git").args(args).current_dir(dir).output()?;
    if !out.status.success() {
        return Err(Error::other(format!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        )));
    }
    Ok(())
}
