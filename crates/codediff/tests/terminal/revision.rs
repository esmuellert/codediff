//! End-to-end terminal and `git diff` parity coverage for `codediff [REV]`.

use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime};

use portable_pty::{CommandBuilder, PtySize, native_pty_system};

use super::common::{screen_text, send_and_expect, wait_for_screen};
use super::pty::{ENTER_ALT, LEAVE_ALT, Output, collect, on_a_terminal, written};

const SIZE: (u16, u16) = (44, 180);

struct Repo {
    dir: PathBuf,
    config: PathBuf,
}

impl Repo {
    fn new(name: &str) -> Self {
        let base = std::env::temp_dir().join(format!(
            "codediff-revision-{name}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        let dir = base.join("repo");
        fs::create_dir_all(&dir).expect("creating test repository");
        let config = base.join("config.json");
        fs::write(
            &config,
            r#"{"version":1,"ui":{"explorer_width":86,"explorer_mode":"list","theme":"basic-dark"}}"#,
        )
        .expect("writing test config");
        let repo = Self { dir, config };
        repo.git(&["init", "-q", "-b", "main"]);
        repo.git(&["config", "user.email", "codediff@example.com"]);
        repo.git(&["config", "user.name", "codediff e2e"]);
        repo.git(&["config", "diff.renames", "true"]);
        repo
    }

    fn git(&self, args: &[&str]) -> String {
        let out = Command::new("git")
            .args(args)
            .current_dir(&self.dir)
            .output()
            .expect("running git");
        assert!(
            out.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8_lossy(&out.stdout).trim().to_owned()
    }

    fn git_bytes(&self, args: &[&str]) -> Vec<u8> {
        let out = Command::new("git")
            .args(args)
            .current_dir(&self.dir)
            .output()
            .expect("running git");
        assert!(
            out.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        out.stdout
    }

    fn write(&self, path: &str, text: &str) {
        let full = self.dir.join(path);
        if let Some(parent) = full.parent() {
            fs::create_dir_all(parent).expect("creating parent directory");
        }
        fs::write(full, text).expect("writing test file");
    }

    fn remove(&self, path: &str) {
        fs::remove_file(self.dir.join(path)).expect("removing test file");
    }
}

impl Drop for Repo {
    fn drop(&mut self) {
        if let Some(base) = self.dir.parent() {
            let _ = fs::remove_dir_all(base);
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ExpectedEntry {
    letter: &'static str,
    path: String,
    previous: Option<String>,
    added: u32,
    removed: u32,
}

/// Computes ground-truth file entries directly from `git diff` (`--name-status` + `--numstat`).
fn git_ground_truth(repo: &Repo, spec: &str) -> Vec<ExpectedEntry> {
    let diff_args: Vec<String> = if let Some((left, right)) = spec.split_once("...") {
        let left = if left.is_empty() { "HEAD" } else { left };
        let right = if right.is_empty() { "HEAD" } else { right };
        let merge_base = repo.git(&["merge-base", "--end-of-options", left, right]);
        vec![merge_base, right.to_owned()]
    } else if let Some((left, right)) = spec.split_once("..") {
        let left = if left.is_empty() { "HEAD" } else { left };
        let right = if right.is_empty() { "HEAD" } else { right };
        vec![left.to_owned(), right.to_owned()]
    } else {
        vec![spec.to_owned()]
    };

    let mut numstat_cmd = vec![
        "diff",
        "--numstat",
        "-z",
        "--find-renames",
        "--end-of-options",
    ];
    numstat_cmd.extend(diff_args.iter().map(String::as_str));
    let numstat_bytes = repo.git_bytes(&numstat_cmd);
    let num_fields: Vec<String> = numstat_bytes
        .split(|&b| b == 0)
        .filter(|field| !field.is_empty())
        .map(|field| String::from_utf8_lossy(field).into_owned())
        .collect();

    let mut stats: HashMap<String, (u32, u32)> = HashMap::new();
    let mut index = 0;
    while index < num_fields.len() {
        let record = &num_fields[index];
        index += 1;
        let mut parts = record.splitn(3, '\t');
        let (Some(added), Some(removed), Some(path)) = (parts.next(), parts.next(), parts.next())
        else {
            continue;
        };
        let path = if path.is_empty() {
            let new_path = num_fields[index + 1].clone();
            index += 2;
            new_path
        } else {
            path.to_owned()
        };
        if let (Ok(added), Ok(removed)) = (added.parse::<u32>(), removed.parse::<u32>()) {
            stats.insert(path, (added, removed));
        }
    }

    let mut status_cmd = vec![
        "diff",
        "--name-status",
        "-z",
        "--find-renames",
        "--end-of-options",
    ];
    status_cmd.extend(diff_args.iter().map(String::as_str));
    let status_bytes = repo.git_bytes(&status_cmd);
    let status_fields: Vec<String> = status_bytes
        .split(|&b| b == 0)
        .filter(|field| !field.is_empty())
        .map(|field| String::from_utf8_lossy(field).into_owned())
        .collect();

    let mut entries = Vec::new();
    let mut index = 0;
    while index < status_fields.len() {
        let code = &status_fields[index];
        index += 1;
        let kind = code.chars().next().expect("status code");
        let (letter, previous, path) = match kind {
            'R' | 'C' => {
                let old = status_fields[index].clone();
                let new = status_fields[index + 1].clone();
                index += 2;
                ("R", Some(old), new)
            }
            'A' => ("A", None, {
                let p = status_fields[index].clone();
                index += 1;
                p
            }),
            'D' => ("D", None, {
                let p = status_fields[index].clone();
                index += 1;
                p
            }),
            'U' => ("!", None, {
                let p = status_fields[index].clone();
                index += 1;
                p
            }),
            _ => ("M", None, {
                let p = status_fields[index].clone();
                index += 1;
                p
            }),
        };
        let (added, removed) = stats.get(&path).copied().unwrap_or((0, 0));
        entries.push(ExpectedEntry {
            letter,
            path,
            previous,
            added,
            removed,
        });
    }
    entries.sort_by(|a, b| a.path.cmp(&b.path));
    entries
}

fn expected_heading(entries: &[ExpectedEntry]) -> String {
    let count = entries.len();
    let added: u32 = entries.iter().map(|e| e.added).sum();
    let removed: u32 = entries.iter().map(|e| e.removed).sum();
    if added == 0 && removed == 0 {
        format!("Changes ({count})")
    } else {
        let mut stats = Vec::new();
        if added > 0 {
            stats.push(format!("+{added}"));
        }
        if removed > 0 {
            stats.push(format!("-{removed}"));
        }
        format!("Changes ({count} · {})", stats.join(" "))
    }
}

fn expected_tail(entry: &ExpectedEntry) -> String {
    let mut parts = Vec::new();
    if entry.added > 0 {
        parts.push(format!("+{}", entry.added));
    }
    if entry.removed > 0 {
        parts.push(format!("-{}", entry.removed));
    }
    parts.push(entry.letter.to_owned());
    parts.join(" ")
}

#[allow(clippy::type_complexity)]
fn spawn_session(
    repo: &Repo,
    extra_args: &[&str],
) -> (
    Box<dyn portable_pty::Child + Send + Sync>,
    Box<dyn portable_pty::MasterPty + Send>,
    Box<dyn Write + Send>,
    std::thread::JoinHandle<()>,
    Arc<Mutex<Output>>,
) {
    let pty = native_pty_system()
        .openpty(PtySize {
            rows: SIZE.0,
            cols: SIZE.1,
            pixel_width: 0,
            pixel_height: 0,
        })
        .expect("opening a pty");

    let mut command = CommandBuilder::new(env!("CARGO_BIN_EXE_codediff"));
    command.arg("--config");
    command.arg(&repo.config);
    command.args(extra_args);
    command.cwd(&repo.dir);
    command.env("TERM", "xterm-256color");

    let child = pty.slave.spawn_command(command).expect("spawning codediff");
    drop(pty.slave);

    let reader = pty.master.try_clone_reader().expect("reading the pty");
    let (collector, output) = collect(reader);
    let writer = pty.master.take_writer().expect("writing to the pty");
    (child, pty.master, writer, collector, output)
}

fn finish_session(
    mut child: Box<dyn portable_pty::Child + Send + Sync>,
    master: Box<dyn portable_pty::MasterPty + Send>,
    mut writer: Box<dyn Write + Send>,
    collector: std::thread::JoinHandle<()>,
    output: Arc<Mutex<Output>>,
) -> String {
    writer.write_all(b"q").expect("sending quit");
    writer.flush().expect("flushing quit");
    drop(writer);

    let deadline = Instant::now() + Duration::from_secs(20);
    let status = loop {
        if let Some(status) = child.try_wait().expect("waiting for codediff") {
            break status;
        }
        assert!(Instant::now() < deadline, "codediff never exited");
        std::thread::sleep(Duration::from_millis(25));
    };

    drop(master);
    let raw = written(collector, &output);
    assert!(status.success(), "codediff failed:\n{raw}");
    assert!(raw.contains(ENTER_ALT));
    assert!(raw.contains(LEAVE_ALT));
    raw
}

/// Asserts that `codediff <cli_args>` renders an Explorer list and first-file
/// `DiffHeader` matching `git diff` ground truth for `git_spec`.
#[track_caller]
fn assert_git_diff_parity(repo: &Repo, cli_args: &[&str], git_spec: &str) {
    let expected = git_ground_truth(repo, git_spec);
    let (child, master, mut writer, collector, output) = spawn_session(repo, cli_args);

    if expected.is_empty() {
        // Wait for initial frame, then exercise navigation/open/stage keys on an empty list.
        wait_for_screen(&output, SIZE, &["Select a file to review."]);
        writer
            .write_all(b"jk\r ")
            .expect("exercising keys on empty Explorer");
        writer.flush().expect("flushing keys");
        std::thread::sleep(Duration::from_millis(200));
        let screen = screen_text(&output, SIZE.0, SIZE.1);
        assert!(
            !screen.contains("Changes ("),
            "expected empty Explorer for {cli_args:?}, got:\n{screen}"
        );
        assert!(
            !screen.contains("Cannot list files"),
            "unexpected error for {cli_args:?}:\n{screen}"
        );
    } else {
        let heading = expected_heading(&expected);
        let screen = wait_for_screen(&output, SIZE, &[&heading]);
        for entry in &expected {
            let matching_lines: Vec<&str> = screen
                .lines()
                .filter(|line| line.contains(&entry.path))
                .collect();
            assert!(
                !matching_lines.is_empty(),
                "{cli_args:?}: missing {} on screen:\n{screen}",
                entry.path
            );
            if let Some(previous) = &entry.previous {
                let rename_label = format!("{} ← {previous}", entry.path);
                assert!(
                    matching_lines
                        .iter()
                        .any(|line| line.contains(&rename_label)),
                    "{cli_args:?}: missing rename {rename_label:?} in {matching_lines:?}"
                );
            }
            let tail = expected_tail(entry);
            assert!(
                matching_lines.iter().any(|line| line.contains(&tail)),
                "{cli_args:?}: for {}, expected {tail:?} in {matching_lines:?}",
                entry.path
            );
        }

        // Open the first file and verify DiffHeader status + path (+ rename source if any).
        let first = &expected[0];
        let header = match &first.previous {
            Some(prev) => format!(" {}  {} ← {prev}", first.letter, first.path),
            None => format!(" {}  {}", first.letter, first.path),
        };
        send_and_expect(&mut writer, &output, SIZE, b"j", &[&header]);
    }

    finish_session(child, master, writer, collector, output);
}

#[test]
fn revision_matrix_matches_git_diff_across_ranges_branches_tags_renames_and_dirty_worktree() {
    let repo = Repo::new("matrix");

    let rename_seed = (1..=12)
        .map(|n| format!("shared rename context line {n}\n"))
        .collect::<String>();

    // Commit 1 (`v1.0`): base files.
    repo.write("src/keep.rs", "fn keep() -> i32 {\n    1\n}\n");
    repo.write("src/mod_me.rs", "fn value() -> i32 {\n    10\n}\n");
    repo.write("src/old_name.rs", &format!("{rename_seed}fn old() {{}}\n"));
    repo.write("src/remove_me.rs", "fn obsolete() {}\n");
    repo.git(&["add", "-A"]);
    repo.git(&["commit", "-qm", "c1: initial"]);
    repo.git(&["tag", "-a", "v1.0", "-m", "annotated release 1.0"]);
    let c1_short = repo.git(&["rev-parse", "--short", "HEAD"]);

    // Branch `side` diverges from `v1.0`.
    repo.git(&["checkout", "-qb", "side"]);
    repo.write(
        "src/side_only.rs",
        "pub fn on_side() -> &'static str {\n    \"side\"\n}\n",
    );
    repo.write("src/mod_me.rs", "fn value() -> i32 {\n    99\n}\n");
    repo.git(&["add", "-A"]);
    repo.git(&["commit", "-qm", "side1: side changes"]);

    // Also create a branch named `doctor` at `side` to test `--` subcommand escape.
    repo.git(&["branch", "doctor"]);

    // Back on `main`: commit 2 (`c2`) with rename + deletion + modification.
    repo.git(&["checkout", "-q", "main"]);
    repo.git(&["mv", "src/old_name.rs", "src/new_name.rs"]);
    repo.write(
        "src/new_name.rs",
        &format!("{rename_seed}fn renamed_fn() {{}}\n"),
    );
    repo.git(&["rm", "-q", "src/remove_me.rs"]);
    repo.write("src/mod_me.rs", "fn value() -> i32 {\n    20\n}\n");
    repo.git(&["add", "-A"]);
    repo.git(&["commit", "-qm", "c2: rename, delete, modify"]);

    // Commit 3 (`v2.0` / `HEAD` on `main`): add a file and tweak `src/keep.rs`.
    repo.write(
        "src/added.rs",
        "pub fn added_in_c3() -> bool {\n    true\n}\n",
    );
    repo.write("src/keep.rs", "fn keep() -> i32 {\n    2\n}\n");
    repo.git(&["add", "-A"]);
    repo.git(&["commit", "-qm", "c3: add and update"]);
    repo.git(&["tag", "v2.0"]);

    // Leave dirty unstaged, staged, and untracked changes in the working tree:
    // - `Against(...)` must include tracked worktree/staged deltas (matching `git diff <rev>`)
    //   while ignoring untracked files.
    // - `Between(...)` and `MergeBase(...)` must completely ignore all dirty state!
    repo.write("src/staged_dirty.rs", "pub fn staged_only() {}\n");
    repo.git(&["add", "src/staged_dirty.rs"]);
    repo.write("src/keep.rs", "fn keep() -> i32 {\n    333\n}\n");
    repo.write("untracked_scratch.txt", "never in git diff\n");

    let index_before = repo.git(&["diff", "--cached", "--name-only"]);

    for (cli_args, git_spec) in [
        (&["HEAD"][..], "HEAD"),
        (&["HEAD~1"][..], "HEAD~1"),
        (&["HEAD~2"][..], "HEAD~2"),
        (&["v1.0"][..], "v1.0"),
        (&[c1_short.as_str()][..], c1_short.as_str()),
        (&["--", "doctor"][..], "doctor"),
        (&["HEAD~1..HEAD"][..], "HEAD~1..HEAD"),
        (&["HEAD~2..HEAD"][..], "HEAD~2..HEAD"),
        (&["HEAD~2.."][..], "HEAD~2.."),
        (&["..side"][..], "..side"),
        (&["HEAD..HEAD~2"][..], "HEAD..HEAD~2"),
        (&["v1.0..v2.0"][..], "v1.0..v2.0"),
        (&["v2.0..v1.0"][..], "v2.0..v1.0"),
        (&["HEAD..HEAD"][..], "HEAD..HEAD"),
        (&[".."][..], ".."),
        (&["main...side"][..], "main...side"),
        (&["side...main"][..], "side...main"),
        (&["side..."][..], "side..."),
        (&["...side"][..], "...side"),
        (&["..."][..], "..."),
    ] {
        assert_git_diff_parity(&repo, cli_args, git_spec);
    }

    // Verify renamed, added, deleted, and dirty-against-commit file contents in DiffViewer,
    // AND verify pressing `Space` in `Against` mode never mutates `.git/index`!
    {
        let (child, master, mut writer, collector, output) = spawn_session(&repo, &["v1.0..v2.0"]);
        wait_for_screen(&output, SIZE, &["Changes (5 · +6 -4)"]);
        send_and_expect(
            &mut writer,
            &output,
            SIZE,
            b"j",
            &[" A  src/added.rs", "pub fn added_in_c3()"],
        );
        send_and_expect(
            &mut writer,
            &output,
            SIZE,
            b"jjj",
            &[
                " R  src/new_name.rs ← src/old_name.rs",
                "fn old()",
                "fn renamed_fn()",
            ],
        );
        send_and_expect(
            &mut writer,
            &output,
            SIZE,
            b"j",
            &[" D  src/remove_me.rs", "fn obsolete()"],
        );
        // Switch to tree mode (`i`), navigate up to `src`, and verify directory cards render
        // all 5 committed files (added, modified, renamed, deleted) in `v1.0..v2.0`!
        send_and_expect(&mut writer, &output, SIZE, b"i", &["└"]);
        send_and_expect(
            &mut writer,
            &output,
            SIZE,
            b"kkkk",
            &[
                "src (5)",
                " A src/added.rs",
                " M src/keep.rs",
                " M src/mod_me.rs",
                " R src/new_name.rs",
            ],
        );
        // Focus the cards pane and scroll down to bring the 5th card () into view.
        send_and_expect(
            &mut writer,
            &output,
            SIZE,
            b"[C[<65;130;15M[<65;130;15M[<65;130;15M",
            &[" D src/remove_me.rs", "fn obsolete()"],
        );
        finish_session(child, master, writer, collector, output);
    }

    // Verify directory cards in tree mode (`i`) under `v1.0..v2.0`, and verify `Space`
    // in `Against("HEAD~2")` does not stage dirty worktree edits into `.git/index`.
    {
        let (child, master, mut writer, collector, output) = spawn_session(&repo, &["HEAD~2"]);
        wait_for_screen(&output, SIZE, &["src/keep.rs"]);
        // Select `src/keep.rs` (which has unstaged edit `333` vs `1` in `HEAD~2`) and press Space.
        send_and_expect(
            &mut writer,
            &output,
            SIZE,
            b"jj",
            &[" M  src/keep.rs", "333"],
        );
        writer
            .write_all(b" ")
            .expect("pressing Space in Against mode");
        writer.flush().expect("flushing Space");
        std::thread::sleep(Duration::from_millis(250));

        // Switch to tree mode (`i`), select `src` directory, and verify directory cards load!
        send_and_expect(&mut writer, &output, SIZE, b"i", &["└"]);
        send_and_expect(&mut writer, &output, SIZE, b"k", &["src ("]);
        finish_session(child, master, writer, collector, output);

        let index_after = repo.git(&["diff", "--cached", "--name-only"]);
        assert_eq!(
            index_after, index_before,
            "pressing Space in `codediff HEAD~2` unexpectedly mutated the Git index"
        );
    }

    // Verify deleted-on-disk file in `Against("HEAD~1")` renders cleanly as Deleted.
    repo.remove("src/added.rs");
    assert_git_diff_parity(&repo, &["HEAD~1"], "HEAD~1");
}

#[test]
fn invalid_revisions_and_unrelated_merge_bases_fail_without_entering_alternate_screen() {
    let repo = Repo::new("invalid-cases");
    repo.write("file.txt", "hello\n");
    repo.git(&["add", "-A"]);
    repo.git(&["commit", "-qm", "initial"]);

    // Create an unrelated orphan branch with no common ancestor with `main`.
    repo.git(&["checkout", "--orphan", "orphan"]);
    repo.git(&["rm", "-rf", "."]);
    repo.write("orphan.txt", "separate root\n");
    repo.git(&["add", "-A"]);
    repo.git(&["commit", "-qm", "orphan root"]);
    repo.git(&["checkout", "-q", "main"]);

    for bad_args in [
        &["nonexistent-rev"][..],
        &["HEAD~999..HEAD"][..],
        &["main...no-such-branch"][..],
        &["...--all"][..],
        &["HEAD:file.txt"][..],
        &["HEAD^{tree}"][..],
        &["main...orphan"][..],
    ] {
        let (output, ok) = on_a_terminal(bad_args, Some(&repo.dir), &[]);
        assert!(
            !ok,
            "expected codediff {bad_args:?} to fail, got:\n{output}"
        );
        assert!(
            !output.contains(ENTER_ALT),
            "codediff {bad_args:?} entered the alternate screen on error:\n{output:?}"
        );
    }
}

#[test]
fn default_worktree_mode_renders_unstaged_staged_double_entry_and_conflicted_files() {
    let repo = Repo::new("worktree-parity");
    repo.write("both.txt", "base\n");
    repo.write("conflict.txt", "base\n");
    repo.write("deleted.txt", "gone soon\n");
    repo.git(&["add", "-A"]);
    repo.git(&["commit", "-qm", "base"]);

    repo.git(&["checkout", "-qb", "theirs"]);
    repo.write("conflict.txt", "theirs line\n");
    repo.git(&["add", "conflict.txt"]);
    repo.git(&["commit", "-qm", "theirs"]);

    repo.git(&["checkout", "-q", "main"]);
    repo.write("conflict.txt", "ours line\n");
    repo.git(&["add", "conflict.txt"]);
    repo.git(&["commit", "-qm", "ours"]);

    // Trigger an unresolved merge conflict on `conflict.txt`.
    let merge = Command::new("git")
        .args(["merge", "theirs"])
        .current_dir(&repo.dir)
        .output()
        .expect("running git merge");
    assert!(!merge.status.success(), "expected merge conflict");

    // Stage `both.txt` and `staged_only.txt`, then edit `both.txt` again on disk,
    // delete `deleted.txt`, and create `untracked.txt`.
    repo.write("both.txt", "staged version\n");
    repo.write("staged_only.txt", "staged new\n");
    repo.git(&["add", "both.txt", "staged_only.txt"]);
    repo.write("both.txt", "worktree version\n");
    repo.remove("deleted.txt");
    repo.write("untracked.txt", "brand new\n");

    let (child, master, writer, collector, output) = spawn_session(&repo, &[]);
    let screen = wait_for_screen(
        &output,
        SIZE,
        &[
            "Changes (",
            "Staged Changes (",
            "conflict.txt",
            "deleted.txt",
            "untracked.txt",
            "staged_only.txt",
        ],
    );
    // `both.txt` appears in both groups (`Changes` and `Staged Changes`).
    assert_eq!(
        screen
            .lines()
            .filter(|line| line.contains("both.txt"))
            .count(),
        2,
        "expected both.txt in both unstaged and staged groups:\n{screen}"
    );
    assert!(
        screen
            .lines()
            .any(|line| line.contains("conflict.txt") && line.contains("!")),
        "expected `!` marker on conflicted file:\n{screen}"
    );
    finish_session(child, master, writer, collector, output);
}
