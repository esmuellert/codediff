//! What a highlighter may refuse.
//!
//! Every threshold in one file, because the alternative is a constant beside
//! the code that trips over it and no way to see the policy as a whole.
//!
//! The numbers are VS Code's, which are the only ones anybody has tuned
//! against real files. Ours differ in one way: where VS Code disables
//! highlighting for a whole file, we simply stop — the lines already coloured
//! stay coloured, because we colour from the top and a reader is usually
//! there.

/// Bytes above which a file is not highlighted at all.
///
/// VS Code's `LARGE_FILE_SIZE_THRESHOLD`. A file this size is a database dump
/// or a bundle, not something a person reads.
pub const MAX_BYTES: usize = 20 * 1024 * 1024;

/// Lines above which a file is not highlighted at all.
///
/// VS Code's `LARGE_FILE_LINE_COUNT_THRESHOLD`.
pub const MAX_LINES: usize = 300_000;

/// Characters above which one line is left uncoloured.
///
/// VS Code's `editor.maxTokenizationLineLength`. A minified bundle is one line
/// of two hundred thousand characters, and a backtracking regex over it can
/// take longer than the rest of the file put together.
///
/// The line is still **shown** — only its colour is skipped, and the parse
/// state is carried across it unchanged, which is `bat`'s approach rather than
/// `delta`'s. Truncating the text would corrupt the state for every line
/// after it.
pub const MAX_LINE_CHARS: usize = 20_000;

/// Lines one frame may colour before giving the frame back.
///
/// Jumping to the end of a file means colouring everything before it, because
/// the answer for a line depends on every line above. Uncapped, `G` in a
/// 300 000-line file — which [`MAX_LINES`] permits — would freeze the
/// interface for sixteen seconds at the 18 500 lines a second this engine
/// measures.
///
/// So a frame does what it can and draws the rest plainly, and the idle pass
/// finishes the job a few milliseconds later. That is exactly what VS Code
/// does, for exactly this reason; it budgets in milliseconds because it has a
/// clock, and this budgets in lines because it must not.
///
/// Two thousand is about a ninth of a second, and is above the length of
/// almost every file anyone reviews — so in the ordinary case nothing is ever
/// deferred, and the cap costs nothing at all.
pub const LEAP: usize = 2_000;

/// Whether a file is worth highlighting at all.
pub fn worth_highlighting(bytes: usize, lines: usize) -> bool {
    bytes <= MAX_BYTES && lines <= MAX_LINES
}

/// Whether one line is worth colouring.
pub fn worth_colouring(line: &str) -> bool {
    line.len() <= MAX_LINE_CHARS
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_ordinary_file_is_highlighted() {
        assert!(worth_highlighting(50_000, 1_200));
    }

    #[test]
    fn a_database_dump_is_not() {
        assert!(!worth_highlighting(64 * 1024 * 1024, 900_000));
        assert!(!worth_highlighting(1_000, 400_000), "line count alone");
    }

    #[test]
    fn a_minified_bundle_line_is_shown_but_not_coloured() {
        let minified = "x".repeat(MAX_LINE_CHARS + 1);
        assert!(!worth_colouring(&minified));
        assert!(worth_colouring(&"x".repeat(MAX_LINE_CHARS)));
    }
}
