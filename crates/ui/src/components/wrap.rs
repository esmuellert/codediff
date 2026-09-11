use std::ops::Range;

use align::ViewLineType;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct WrappedViewLine {
    pub(super) original: Vec<TerminalLine>,
    pub(super) modified: Vec<TerminalLine>,
    pub(super) diff_type: ViewLineType,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalLine {
    SourceCode { source_line: u32, bytes: Range<u32> },
    Filler,
}
