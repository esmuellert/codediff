//! Parses `diff_tool`'s human-readable output into comparable values.
//!
//! Comparing text would couple our output format to upstream's. Comparing
//! structure asks the question that matters: given the same input, does the
//! engine report the same changes through our binding as through theirs?

use anyhow::{Context, Result, bail};

/// A line range: 1-based, end exclusive.
pub type LineSpan = (u32, u32);

/// A character range as `(start_line, start_col, end_line, end_col)`:
/// 1-based, end column exclusive, columns in UTF-16 code units.
pub type CharSpan = (u32, u32, u32, u32);

/// A change as reported by `diff_tool`, with ranges normalised to the
/// end-exclusive convention used everywhere else.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OracleChange {
    pub original: LineSpan,
    pub modified: LineSpan,
    pub inner: Vec<OracleInner>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OracleInner {
    pub original: CharSpan,
    pub modified: CharSpan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OracleDiff {
    pub changes: Vec<OracleChange>,
    pub moves: Vec<CharSpan>,
    pub hit_timeout: bool,
}

/// Parses output of the form:
///
/// ```text
///   Changes: 1 line mapping(s)
///     [0] Lines 2-2 -> Lines 2-3 (1 inner change)
///          Inner: L2:C1-L2:C3 -> L2:C1-L3:C4
///
///   Moves: 0 move(s)
/// ```
///
/// `diff_tool` prints the end line **inclusive**; ranges are converted back to
/// end-exclusive here so both sides speak the same language.
pub fn parse(output: &str) -> Result<OracleDiff> {
    let mut changes: Vec<OracleChange> = Vec::new();
    let mut moves = Vec::new();
    let mut hit_timeout = false;
    let mut in_moves = false;

    for line in output.lines() {
        let trimmed = line.trim();

        if let Some(rest) = trimmed.strip_prefix("Hit timeout:") {
            hit_timeout = rest.trim() == "yes";
            continue;
        }
        if trimmed.starts_with("Moves:") {
            in_moves = true;
            continue;
        }
        if trimmed.starts_with("Changes:") {
            in_moves = false;
            continue;
        }

        if let Some(rest) = trimmed.strip_prefix("Inner: ") {
            let (original, modified) = parse_inner(rest)
                .with_context(|| format!("parsing inner change from {trimmed:?}"))?;
            let change = changes
                .last_mut()
                .context("an Inner line appeared before any change")?;
            change.inner.push(OracleInner { original, modified });
            continue;
        }

        if trimmed.starts_with('[') {
            let (original, modified) = parse_ranges(trimmed)
                .with_context(|| format!("parsing ranges from {trimmed:?}"))?;
            if in_moves {
                moves.push((original.0, original.1, modified.0, modified.1));
            } else {
                changes.push(OracleChange {
                    original,
                    modified,
                    inner: Vec::new(),
                });
            }
        }
    }

    Ok(OracleDiff {
        changes,
        moves,
        hit_timeout,
    })
}

/// `[0] Lines 2-2 -> Lines 2-3 (1 inner change)` → `((2, 3), (2, 4))`
fn parse_ranges(line: &str) -> Result<(LineSpan, LineSpan)> {
    let mut parts = line.split("Lines ").skip(1);
    let original = parts.next().context("missing the original range")?;
    let modified = parts.next().context("missing the modified range")?;
    Ok((inclusive_pair(original)?, inclusive_pair(modified)?))
}

/// `2-3 -> ...` → `(2, 4)`, converting the inclusive end to exclusive.
fn inclusive_pair(text: &str) -> Result<LineSpan> {
    let field = text
        .split_whitespace()
        .next()
        .context("empty line range field")?;
    let (start, end) = field
        .split_once('-')
        .with_context(|| format!("expected START-END, got {field:?}"))?;
    let start: u32 = start.parse().context("start line is not a number")?;
    let end: i64 = end.parse().context("end line is not a number")?;
    Ok((start, u32::try_from(end + 1).unwrap_or(start)))
}

/// `L2:C1-L2:C3 -> L2:C1-L3:C4`
fn parse_inner(text: &str) -> Result<(CharSpan, CharSpan)> {
    let (original, modified) = text
        .split_once(" -> ")
        .with_context(|| format!("expected `A -> B`, got {text:?}"))?;
    Ok((char_range(original)?, char_range(modified)?))
}

/// `L2:C1-L3:C4` → `(2, 1, 3, 4)`
fn char_range(text: &str) -> Result<CharSpan> {
    let (start, end) = text
        .trim()
        .split_once('-')
        .with_context(|| format!("expected START-END, got {text:?}"))?;
    let (start_line, start_col) = position(start)?;
    let (end_line, end_col) = position(end)?;
    Ok((start_line, start_col, end_line, end_col))
}

/// `L2:C1` → `(2, 1)`
fn position(text: &str) -> Result<(u32, u32)> {
    let text = text.trim();
    let Some(rest) = text.strip_prefix('L') else {
        bail!("expected a position beginning with L, got {text:?}");
    };
    let (line, col) = rest
        .split_once(":C")
        .with_context(|| format!("expected L<line>:C<col>, got {text:?}"))?;
    Ok((line.parse()?, col.parse()?))
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "\
Diff Results:
Number of changes: 1
Hit timeout: no

  Changes: 1 line mapping(s)
    [0] Lines 2-2 -> Lines 2-3 (1 inner change)
         Inner: L2:C1-L2:C3 -> L2:C1-L3:C4

  Moves: 0 move(s)
";

    #[test]
    fn converts_inclusive_ends_to_exclusive() {
        let parsed = parse(SAMPLE).unwrap();
        assert_eq!(parsed.changes.len(), 1);
        // Printed as 2-2 and 2-3, meaning lines [2,3) and [2,4).
        assert_eq!(parsed.changes[0].original, (2, 3));
        assert_eq!(parsed.changes[0].modified, (2, 4));
        assert!(!parsed.hit_timeout);
        assert!(parsed.moves.is_empty());
    }

    #[test]
    fn reads_inner_positions() {
        let parsed = parse(SAMPLE).unwrap();
        assert_eq!(parsed.changes[0].inner.len(), 1);
        assert_eq!(parsed.changes[0].inner[0].original, (2, 1, 2, 3));
        assert_eq!(parsed.changes[0].inner[0].modified, (2, 1, 3, 4));
    }

    #[test]
    fn reads_moves_separately_from_changes() {
        let text = "\
  Changes: 0 line mapping(s)

  Moves: 1 move(s)
    [0] Lines 1-3 -> Lines 7-9
";
        let parsed = parse(text).unwrap();
        assert!(parsed.changes.is_empty());
        assert_eq!(parsed.moves, vec![(1, 4, 7, 10)]);
    }

    #[test]
    fn notices_a_timeout() {
        let parsed = parse("Hit timeout: yes\n  Changes: 0 line mapping(s)\n").unwrap();
        assert!(parsed.hit_timeout);
    }
}
