//! VS Code's size limits for syntax highlighting.

/// VS Code's `LARGE_FILE_SIZE_THRESHOLD`.
pub const MAX_BYTES: usize = 20 * 1024 * 1024;

/// VS Code's `LARGE_FILE_LINE_COUNT_THRESHOLD`.
pub const MAX_LINES: usize = 300_000;

/// VS Code's `editor.maxTokenizationLineLength`.
pub const MAX_LINE_CHARS: usize = 20_000;

/// Whether a file is below both limits.
pub fn worth_highlighting(bytes: usize, lines: usize) -> bool {
    bytes <= MAX_BYTES && lines <= MAX_LINES
}

/// Whether one line is short enough to colour.
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
