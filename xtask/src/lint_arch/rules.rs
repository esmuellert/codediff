//! The rules themselves, and nothing that applies them.
//!
//! A file of tables, so that "what is forbidden here" can be read and argued
//! with in one screen, without the walking and parsing that enforces it. Every
//! entry carries the reason it exists: a rule whose justification has to be
//! looked up elsewhere is a rule that will eventually be deleted by someone
//! who could not find it.

/// Edges that must never exist, with the reason reported on failure.
pub const FORBIDDEN_EDGES: &[(&str, &str, &str)] = &[
    ("ui", "vcs", "a renderer must not be able to reach git"),
    (
        "ui",
        "vscode-diff",
        "rendering consumes model types, it does not compute diffs",
    ),
    (
        "ui",
        "vscode-diff-sys",
        "rendering must never touch the FFI layer",
    ),
    (
        "align",
        "vcs",
        "the aligned model is pure and must not perform IO",
    ),
    (
        "explorer",
        "vcs",
        "the explorer model is pure; obtaining entries belongs to vcs",
    ),
    ("line-index", "vcs", "text measurement must not perform IO"),
    ("syntax", "vcs", "syntactic analysis must not perform IO"),
];

/// Edges forbidden in what ships, but allowed in tests.
///
/// A dev-dependency does not propagate to consumers, so a crate may use the
/// engine as a test oracle while its library still builds without one. That is
/// exactly what `align` does: its tests feed real engine output through the
/// aligner, and its library names only the types.
pub const FORBIDDEN_SHIPPED_EDGES: &[(&str, &str, &str)] = &[
    (
        "align",
        "vscode-diff",
        "pairing is handed a diff, it does not compute one — depending on the \
         engine would drag a C toolchain into a pure crate",
    ),
    (
        "align",
        "vscode-diff-sys",
        "the aligned model must never touch the FFI layer",
    ),
    (
        "diff-types",
        "vscode-diff-sys",
        "what a diff *is* must be nameable without a C toolchain",
    ),
    (
        "file-types",
        "vcs",
        "every layer names the shared file vocabulary, so it can name none of \
         them — an edge here would be a cycle waiting to happen",
    ),
    (
        "file-types",
        "align",
        "a file does not know how two of them are paired up",
    ),
];

/// Crates that must not perform IO, so that they stay trivially testable.
pub const PURE_CRATES: &[&str] = &[
    "line-index",
    "syntax",
    "align",
    "explorer",
    "vscode-diff",
    "diff-types",
    "file-types",
];

pub const IO_MARKERS: &[&str] = &["std::fs", "std::process", "std::net", "std::env::var"];

/// Directories that must not read a clock, with the property that depends on
/// it.
///
/// Not the same rule as [`PURE_CRATES`]: `ui` legitimately performs IO —
/// it owns a terminal. What must not happen is a *clock* reaching the key
/// resolver, because being a pure function of its own state and one key is
/// what lets a test be a string of keys. A timeout would make that
/// non-deterministic, and would do so silently: the tests would still pass,
/// until one day they would not.
///
/// If ambiguous bindings are ever wanted, the answer is to inject the clock as
/// a parameter and delete this rule, not to reach for one here.
pub const CLOCK_FREE_DIRS: &[(&str, &str)] = &[(
    "crates/ui/src/input",
    "the key resolver is a pure function of its own state and one key",
)];

pub const CLOCK_MARKERS: &[&str] = &["std::time", "Instant", "SystemTime", "Duration"];

/// Crates exempt from `unsafe_code = "forbid"`, with the policy they use instead.
pub const UNSAFE_EXEMPT: &[&str] = &["vscode-diff-sys", "vscode-diff"];

/// A syntax engine may only be named inside this directory, so that swapping
/// engines touches nothing else. See docs/plan/05-decisions.md D17.
pub const ENGINE_CRATES: &[&str] = &["syntect", "tree_sitter"];
pub const ENGINE_DIR: &str = "crates/syntax/src/engine";
