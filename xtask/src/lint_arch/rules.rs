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

/// Words that may not appear in a type we declare, and what to use instead.
///
/// These are *meta-names*: they classify without saying anything. `RowKind`
/// told a reader that a row has variants, which the `enum` already said; the
/// word that carries meaning is the noun beside it. The house suffix is
/// `Type` — `ChangeType`, `BufferType`, `ViewLineType` — chosen once so there
/// is not a second word for one idea, which is the failure D28 removed.
///
/// Only names *we declare* are checked. `std::io::ErrorKind` and crossterm's
/// `KeyEventKind` are other people's vocabulary arriving through a `use`, and
/// renaming those is not ours to do.
pub const BANNED_TYPE_WORDS: &[(&str, &str)] = &[
    (
        "Kind",
        "Type — the house suffix, as in ChangeType and BufferType",
    ),
    ("Data", "say what the data is"),
    ("Info", "say what the information is"),
    ("Manager", "a verb, or the thing being managed"),
    ("Helper", "the job it does"),
    ("Util", "the job it does"),
    ("Handler", "the event it answers"),
];

/// Directories that may not name a module of their own crate, and why.
///
/// The boundary between a brick and a composition. `ui/src/render` draws onto
/// a cell grid — rectangles, line numbers, one line of text — and is handed
/// everything it needs; `ui/src/draw` is what knows that a side-by-side diff
/// is two of those columns with a divider between them. If a brick could name
/// a buffer it would stop being reusable by a buffer type that does not exist
/// yet, and would stop being testable without a model.
///
/// Checked as text rather than by the compiler because Rust has no way to say
/// "this module may not import that one" within a crate.
/// The one file allowed to start a thread, and why nowhere else may.
///
/// Concurrency is the easiest thing in this program to get wrong and the
/// hardest to test, so there is exactly one place it exists: the syntax
/// worker, which colours text off the drawing thread. Everything else is
/// single-threaded and can be reasoned about as such.
///
/// A second `spawn` anywhere would mean two things sharing the view, and the
/// question "which thread owns this?" would stop having an obvious answer. If
/// another background job is ever wanted, it belongs beside this one or behind
/// the same channel — not in a new corner. See D41.
pub const THREAD_FILE: &str = "crates/ui/src/syntax/mod.rs";
pub const THREAD_MARKERS: &[&str] = &["thread::spawn", "thread::Builder"];

pub const BLIND_DIRS: &[(&str, &str, &str)] = &[
    (
        "crates/ui/src/render",
        "crate::view",
        "a brick is handed what it draws; `ui/src/draw` is what knows the model",
    ),
    // The interface used to build the scope and capture tables itself and hand
    // them in, which meant it held both engines' vocabulary — `@type.builtin`,
    // `punctuation.definition.string` — outside the directory that is supposed
    // to be the only place either engine is known. The words moved to
    // `syntax::engine`; this stops them coming back. See D43.
    (
        "crates/ui/src",
        "syntax::engine",
        "the engines' own words live in `syntax`; `ui` asks for a palette and a role",
    ),
];
