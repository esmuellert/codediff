use std::ops::Range;

use align::{Alignment, ViewLine, ViewLineContent, ViewLineType};
use file_types::{DiffType, DiffVersion};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WrappedViewLine {
    pub(crate) original: Vec<TerminalLine>,
    pub(crate) modified: Vec<TerminalLine>,
    pub(crate) diff_type: ViewLineType,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalLine {
    SourceCode { source_line: u32, bytes: Range<u32> },
    Filler,
}

impl WrappedViewLine {
    pub(crate) fn from_view_line(
        view_line: ViewLine,
        original_source: Option<&str>,
        modified_source: Option<&str>,
    ) -> Self {
        Self {
            original: vec![TerminalLine::from_view_line_content(
                view_line.original,
                original_source,
            )],
            modified: vec![TerminalLine::from_view_line_content(
                view_line.modified,
                modified_source,
            )],
            diff_type: view_line.kind,
        }
    }

    pub(crate) fn selected_terminal_line(&self) -> Option<TerminalLine> {
        match self.modified.first() {
            Some(TerminalLine::SourceCode { .. }) => self.modified.first().cloned(),
            Some(TerminalLine::Filler) | None => self.original.first().cloned(),
        }
    }
}

impl TerminalLine {
    pub(crate) fn from_view_line_content(content: ViewLineContent, source: Option<&str>) -> Self {
        match content {
            ViewLineContent::SourceLine(source_line) => Self::SourceCode {
                source_line,
                bytes: 0..source.unwrap_or("").len() as u32,
            },
            ViewLineContent::Filler => Self::Filler,
        }
    }

    pub(crate) fn source_line(&self) -> Option<u32> {
        match self {
            Self::SourceCode { source_line, .. } => Some(*source_line),
            Self::Filler => None,
        }
    }
}

pub(crate) fn unwrapped_view_lines(
    alignment: &Alignment,
    diff_type: DiffType,
) -> Vec<WrappedViewLine> {
    alignment
        .view_lines_from(diff_type, 0)
        .map(|view_line| {
            let original = view_line
                .original
                .line()
                .and_then(|line| alignment.line(DiffVersion::Original, line));
            let modified = view_line
                .modified
                .line()
                .and_then(|line| alignment.line(DiffVersion::Modified, line));
            WrappedViewLine::from_view_line(view_line, original, modified)
        })
        .collect()
}

pub(crate) fn find_terminal_line_index(
    lines: &[WrappedViewLine],
    target: &TerminalLine,
) -> Option<u32> {
    lines
        .iter()
        .position(|line| line.selected_terminal_line().as_ref() == Some(target))
        .map(|index| index as u32)
}
