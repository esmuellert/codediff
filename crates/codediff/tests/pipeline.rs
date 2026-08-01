//! The whole pipeline on a real repository: git finds a file, reads its two
//! sides, the engine compares them, `align` pairs them up.
//!
//! Runs the built binary, so what is tested is what ships.

use std::path::{Path, PathBuf};
use std::process::Command;

struct Fixture {
    dir: PathBuf,
}

impl Fixture {
    fn new(name: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("codediff-s6-{name}-{}", std::process::id()));
        fixtures::repo(&dir).expect("building the fixture repository");
        Self { dir }
    }

    /// Runs the binary inside the fixture, as a user would.
    fn run(&self, args: &[&str]) -> String {
        let out = Command::new(env!("CARGO_BIN_EXE_codediff"))
            .args(args)
            .current_dir(&self.dir)
            .output()
            .expect("running codediff");
        assert!(
            out.status.success(),
            "codediff {args:?} failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8(out.stdout).expect("output is utf-8")
    }

    fn run_bytes(&self, args: &[&str]) -> Vec<u8> {
        Command::new(env!("CARGO_BIN_EXE_codediff"))
            .args(args)
            .current_dir(&self.dir)
            .output()
            .expect("running codediff")
            .stdout
    }

    fn git(&self, args: &[&str]) -> Vec<u8> {
        Command::new("git")
            .args(args)
            .current_dir(&self.dir)
            .output()
            .expect("running git")
            .stdout
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

#[test]
fn blob_content_matches_git_show_byte_for_byte() {
    // If this is wrong, every diff that follows is of something other than
    // what the repository holds.
    let fixture = Fixture::new("show");
    for path in [
        "modified.txt",
        "crlf.txt",
        "no-trailing-newline.txt",
        "picture.png",
    ] {
        let ours = fixture.run_bytes(&["debug", "show", &format!("HEAD:{path}"), "--raw"]);
        let theirs = fixture.git(&["show", &format!("HEAD:{path}")]);
        assert_eq!(ours, theirs, "{path} differs from git show");
    }
}

#[test]
fn a_modified_file_reports_the_same_changed_lines_as_git() {
    let fixture = Fixture::new("modified");
    let out = fixture.run(&["debug", "diff-file", "modified.txt"]);

    // "one / TWO / three" against "one / two / three": one line differs.
    assert!(out.contains("1 change(s)"), "{out}");
    assert!(out.contains("two"), "{out}");
    assert!(out.contains("TWO"), "{out}");
}

#[test]
fn crlf_produces_no_phantom_diff() {
    // Mishandled line endings make every line look changed, which would bury
    // the one real edit.
    let fixture = Fixture::new("crlf");
    let out = fixture.run(&["debug", "diff-file", "crlf.txt"]);

    assert!(
        out.contains("1 change(s)"),
        "only the added line changed:\n{out}"
    );
    // Carriage returns are shown, not silently eaten: a file that gains CRLF
    // endings must not look unchanged.
    assert!(out.contains('\u{240d}'), "the CR should be visible:\n{out}");
}

#[test]
fn a_file_with_no_trailing_newline_is_handled() {
    let fixture = Fixture::new("nonewline");
    let out = fixture.run(&["debug", "diff-file", "no-trailing-newline.txt"]);
    assert!(out.contains("change(s)"), "{out}");
    assert!(!out.contains("panicked"), "{out}");
}

#[test]
fn a_binary_file_is_reported_rather_than_diffed() {
    // `before`/`after` hand back bytes, and a picture has no lines. Saying so
    // is the answer; feeding it to the engine is not.
    let fixture = Fixture::new("binary");
    let out = fixture.run(&["debug", "diff-file", "picture.png"]);

    assert!(out.contains("binary"), "{out}");
    assert!(out.contains("no line diff"), "{out}");
}

#[test]
fn a_deleted_file_shows_every_line_as_gone() {
    // The side that does not exist has *no* lines, and the engine cannot say
    // that — it models an empty file as one empty line. Handled before the
    // engine, or the file would report as modified with a phantom blank line.
    let fixture = Fixture::new("deleted");
    let out = fixture.run(&["debug", "diff-file", "deleted.txt"]);

    assert!(out.contains("after    absent"), "{out}");
    assert!(out.contains("every line is gone"), "{out}");
    assert!(out.contains("- goes away"), "{out}");
    assert!(
        !out.contains('~'),
        "a one-sided file is not a modification:\n{out}"
    );
}

#[test]
fn an_untracked_file_shows_every_line_as_new() {
    let fixture = Fixture::new("untracked");
    let out = fixture.run(&["debug", "diff-file", "untracked.txt"]);

    assert!(out.contains("before   absent"), "{out}");
    assert!(out.contains("every line is new"), "{out}");
    assert!(out.contains("+ never added"), "{out}");
}

#[test]
fn a_moved_file_is_found_by_either_of_its_paths() {
    // The before side lives at the old path; a reviewer should be able to name
    // either one.
    let fixture = Fixture::new("moved");
    for path in ["renamed-to.txt", "renamed-from.txt"] {
        let out = fixture.run(&["debug", "diff-file", path]);
        assert!(out.contains("Moved"), "{path}:\n{out}");
        assert!(
            out.contains("moved from renamed-from.txt"),
            "{path}:\n{out}"
        );
    }
}

#[test]
fn an_unchanged_file_diffs_to_nothing_rather_than_failing() {
    let fixture = Fixture::new("unchanged");
    let out = fixture.run(&["debug", "diff-file", "unchanged.txt"]);
    assert!(out.contains("0 change(s)"), "{out}");
}

#[test]
fn a_path_that_does_not_exist_is_an_error_not_a_panic() {
    let fixture = Fixture::new("missing");
    let out = Command::new(env!("CARGO_BIN_EXE_codediff"))
        .args(["debug", "diff-file", "no/such/file.txt"])
        .current_dir(&fixture.dir)
        .output()
        .expect("running codediff");

    assert!(!out.status.success());
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("neither changed nor present"), "{stderr}");
    assert!(!stderr.contains("panicked"), "{stderr}");
}

#[test]
fn awkward_paths_work_end_to_end() {
    let fixture = Fixture::new("awkward");
    for path in ["with spaces.txt", "ünïcodé-ファイル.txt"] {
        let out = fixture.run(&["debug", "diff-file", path]);
        assert!(out.contains("change(s)"), "{path}:\n{out}");
    }
}

#[test]
fn every_changed_file_can_be_diffed_without_failing() {
    // The blunt check: whatever git reports, we can open. A file we cannot
    // handle is one the reviewer cannot see.
    let fixture = Fixture::new("all");
    let status = fixture.run(&["debug", "status"]);

    let paths: Vec<String> = status
        .lines()
        .filter(|l| l.starts_with("  ") && l.len() > 8)
        .filter_map(|l| l.get(8..).map(str::to_owned))
        .filter(|p| !p.is_empty() && !p.contains(" <- "))
        .collect();
    assert!(
        paths.len() >= 8,
        "expected the fixture's files, got {paths:?}"
    );

    for path in paths {
        let out = Command::new(env!("CARGO_BIN_EXE_codediff"))
            .args(["debug", "diff-file", &path])
            .current_dir(&fixture.dir)
            .output()
            .expect("running codediff");
        assert!(
            out.status.success(),
            "{path} could not be diffed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
}

/// Not a fixture test: this repository, which is a real one.
#[test]
fn it_works_on_this_repository() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("the workspace root");
    let out = Command::new(env!("CARGO_BIN_EXE_codediff"))
        .args(["debug", "status"])
        .current_dir(root)
        .output()
        .expect("running codediff");

    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    let text = String::from_utf8_lossy(&out.stdout);
    assert!(text.contains("root"), "{text}");
}
