//! `debug align` output, pinned for all twelve pairs.
//!
//! The human-readable artifact and the regression fixture are the same file: if
//! the snapshot reads correctly to a person, the format is doing its job, and
//! any later change to the pairing shows up as a diff of the diff.
//!
//! Regenerate after a deliberate change:
//!
//! ```sh
//! UPDATE_GOLDEN=1 cargo test -p codediff --test golden
//! ```

use std::path::{Path, PathBuf};
use std::process::Command;

const PAIRS: &[&str] = &[
    "adjacent_move",
    "block_moved_down",
    "comprehensive_move",
    "duplicate_not_move",
    "empty_files",
    "large_file_move",
    "long_distance_move",
    "moved_with_edit",
    "multi_move",
    "no_moves_control",
    "simple_swap",
    "single_line_move",
];

/// Columns before the text begins: five for the line number, then the marker.
/// All ASCII, so character positions are display columns there.
const GUTTER: usize = 8;

fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("the crate sits two levels below the workspace root")
        .to_path_buf()
}

fn render(pair: &str) -> String {
    let dir = workspace().join("libvscode-diff/tests/oracle").join(pair);
    let output = Command::new(env!("CARGO_BIN_EXE_codediff"))
        .arg("debug")
        .arg("align")
        .arg(dir.join("original.txt"))
        .arg(dir.join("modified.txt"))
        .arg("--verbose")
        .current_dir(workspace())
        .output()
        .expect("running the binary");

    assert!(
        output.status.success(),
        "{pair}: exited {:?}\n{}",
        output.status.code(),
        String::from_utf8_lossy(&output.stderr)
    );

    let text = String::from_utf8(output.stdout).expect("output is utf-8");
    // The first line holds absolute paths, which differ per machine.
    let body = text.split_once('\n').map(|(_, rest)| rest).unwrap_or(&text);
    format!("{pair}\n{body}")
}

#[test]
fn the_rendered_pairs_match_their_snapshots() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/golden");
    std::fs::create_dir_all(&dir).expect("creating the snapshot directory");
    let updating = std::env::var_os("UPDATE_GOLDEN").is_some();
    let mut stale = Vec::new();

    for pair in PAIRS {
        let actual = render(pair);
        let path = dir.join(format!("{pair}.txt"));

        if updating {
            std::fs::write(&path, &actual).expect("writing the snapshot");
            continue;
        }

        let expected = std::fs::read_to_string(&path).unwrap_or_else(|_| {
            panic!(
                "no snapshot for {pair}; run \
                 UPDATE_GOLDEN=1 cargo test -p codediff --test golden"
            )
        });
        if expected != actual {
            stale.push((*pair).to_owned());
        }
    }

    assert!(
        stale.is_empty(),
        "snapshots no longer match: {}\nreview the change, then run \
         UPDATE_GOLDEN=1 cargo test -p codediff --test golden",
        stale.join(", ")
    );
}

/// The property the snapshots exist to protect, checked against the files
/// themselves so that a wrong snapshot cannot be blessed into place.
///
/// Both columns, every row, no skipping. An earlier version checked only the
/// original column and skipped any line long enough to be clipped, so a change
/// that clipped everything, or corrupted only the modified side, would have
/// passed.
#[test]
fn both_rendered_columns_read_back_as_their_files() {
    for pair in PAIRS {
        let dir = workspace().join("libvscode-diff/tests/oracle").join(pair);
        let original = std::fs::read_to_string(dir.join("original.txt")).expect("fixture exists");
        let modified = std::fs::read_to_string(dir.join("modified.txt")).expect("fixture exists");
        let rendered = render(pair);

        let lines: Vec<ViewLine> = rendered.lines().filter_map(parse).collect();
        assert!(!lines.is_empty(), "{pair}: nothing was rendered");

        check_column(
            pair,
            "original",
            &lines,
            |r| (r.original_line, &r.original),
            &original,
        );
        check_column(
            pair,
            "modified",
            &lines,
            |r| (r.modified_line, &r.modified),
            &modified,
        );
    }
}

struct ViewLine {
    original_line: Option<u32>,
    original: String,
    modified_line: Option<u32>,
    modified: String,
}

/// Splits a rendered row on its divider.
///
/// The gutters are fixed-width ASCII, so character positions are display
/// columns there; the text runs to the end of its half, which avoids having to
/// slice a column containing wide characters.
fn parse(line: &str) -> Option<ViewLine> {
    let (left, right) = line.split_once(" \u{2502} ")?;
    let number = |half: &str| -> Option<u32> {
        half.chars().take(5).collect::<String>().trim().parse().ok()
    };
    let text = |half: &str| -> String {
        let body: String = half.chars().skip(GUTTER).collect();
        // A move note is appended after the right-hand text.
        match body
            .split_once("   \u{2193} moved")
            .or_else(|| body.split_once("   \u{2191} moved"))
        {
            Some((before, _)) => before.trim_end().to_owned(),
            None => body.trim_end().to_owned(),
        }
    };
    Some(ViewLine {
        original_line: number(left),
        original: text(left),
        modified_line: number(right),
        modified: text(right),
    })
}

fn check_column(
    pair: &str,
    version: &str,
    rows: &[ViewLine],
    pick: impl Fn(&ViewLine) -> (Option<u32>, &String),
    file: &str,
) {
    let expected: Vec<&str> = file.split('\n').collect();
    let mut next = 1u32;

    for row in rows {
        let (number, shown) = pick(row);
        let Some(number) = number else {
            assert!(
                shown.chars().all(|c| c == '\u{2571}'),
                "{pair}/{version}: a row with no line number should be filler, got {shown:?}"
            );
            continue;
        };
        assert_eq!(
            number, next,
            "{pair}/{version}: line numbers jumped to {number}"
        );
        next += 1;

        let expected = expand_tabs(expected[number as usize - 1]);
        // Long lines are clipped with an ellipsis; the visible part must still
        // be a prefix of the real one.
        match shown.strip_suffix('\u{2026}') {
            Some(prefix) => assert!(
                expected.starts_with(prefix),
                "{pair}/{version}: line {number} was clipped to {prefix:?}, which is not a prefix of {expected:?}"
            ),
            None => assert_eq!(
                shown.as_str(),
                expected.trim_end(),
                "{pair}/{version}: line {number} does not match the file"
            ),
        }
    }

    assert_eq!(
        next as usize - 1,
        expected.len(),
        "{pair}/{version}: rendered {} lines, the file has {}",
        next - 1,
        expected.len()
    );
}

/// Expands tabs the way the renderer does — to the next stop, not four spaces
/// each. A tab starting at column 3 is one column wide, not four.
fn expand_tabs(text: &str) -> String {
    let mut out = String::new();
    for g in line_index::graphemes(text, line_index::DEFAULT_TAB_WIDTH) {
        if g.is_tab() {
            out.extend(std::iter::repeat_n(' ', g.width as usize));
        } else {
            out.push_str(g.text);
        }
    }
    out.trim_end().to_owned()
}
