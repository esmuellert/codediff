//! Tuning for a single diff computation.

/// Options passed to [`crate::compute`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Options {
    /// Treat lines that differ only in leading or trailing whitespace as equal.
    pub ignore_trim_whitespace: bool,

    /// Give up refining after this long and return a coarser result with
    /// [`crate::LinesDiff::hit_timeout`] set. Zero means no limit.
    pub max_computation_time_ms: u32,

    /// Detect blocks that moved rather than being deleted and re-added.
    /// Off by default because it costs additional work.
    pub compute_moves: bool,

    /// Extend character-level changes out to subword boundaries, which tends to
    /// produce more readable highlighting on identifier edits.
    pub extend_to_subwords: bool,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            ignore_trim_whitespace: false,
            // Matches the upstream Neovim plugin's default. Large enough that
            // ordinary files never hit it, small enough that a pathological
            // file cannot stall the UI.
            max_computation_time_ms: 5_000,
            compute_moves: false,
            extend_to_subwords: false,
        }
    }
}

impl Options {
    /// Enables move detection.
    pub fn with_moves(mut self) -> Self {
        self.compute_moves = true;
        self
    }

    /// Ignores leading and trailing whitespace differences.
    pub fn ignoring_trim_whitespace(mut self) -> Self {
        self.ignore_trim_whitespace = true;
        self
    }

    /// Sets the computation budget in milliseconds; zero means no limit.
    pub fn with_time_budget_ms(mut self, ms: u32) -> Self {
        self.max_computation_time_ms = ms;
        self
    }
}

impl From<Options> for vscode_diff_sys::DiffOptions {
    fn from(options: Options) -> Self {
        Self {
            ignore_trim_whitespace: options.ignore_trim_whitespace,
            max_computation_time_ms: options.max_computation_time_ms.min(i32::MAX as u32) as i32,
            compute_moves: options.compute_moves,
            extend_to_subwords: options.extend_to_subwords,
        }
    }
}
