//! Architecture rules and their failure messages.

/// Edges that must never exist, with the reason reported on failure.
pub const FORBIDDEN_EDGES: &[(&str, &str, &str)] = &[
    (
        "ui",
        "vcs",
        "git is reached through `pipeline`, which owns the thread it runs on",
    ),
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
    ("line-index", "vcs", "text measurement must not perform IO"),
    ("syntax", "vcs", "syntactic analysis must not perform IO"),
];

/// Edges allowed only as development dependencies.
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
    "vscode-diff",
    "diff-types",
    "file-types",
];

pub const IO_MARKERS: &[&str] = &["std::fs", "std::process", "std::net", "std::env::var"];

/// Directories that must remain independent of clocks.
pub const CLOCK_FREE_DIRS: &[(&str, &str)] = &[(
    "crates/ui/src/input",
    "the key resolver is a pure function of its own state and one key",
)];

pub const CLOCK_MARKERS: &[&str] = &["std::time", "Instant", "SystemTime", "Duration"];

/// Crates exempt from `unsafe_code = "forbid"`, with the policy they use instead.
pub const UNSAFE_EXEMPT: &[&str] = &["vscode-diff-sys", "vscode-diff"];

/// Package fields inherited from `[workspace.package]`.
pub const INHERITED_PACKAGE_FIELDS: &[&str] = &[
    "version",
    "edition",
    "rust-version",
    "license",
    "repository",
    "authors",
];

/// Inherited fields that every package must declare.
pub const REQUIRED_PACKAGE_FIELDS: &[&str] = &["version"];

/// A syntax engine may only be named inside this directory, so that swapping
/// engines touches nothing else. See docs/plan/05-decisions.md D17.
pub const ENGINE_CRATES: &[&str] = &["syntect", "tree_sitter"];
pub const ENGINE_DIR: &str = "crates/syntax/src/engine";

/// Whole identifiers that are too vague, with preferred vocabulary.
pub const BANNED_NAMES: &[(&str, &str)] = &[
    (
        "comparison",
        "what is compared — a `Diff`, or the two revisions",
    ),
    ("answer", "what came back; the type is a `Response`"),
    ("ask", "what is asked for, as `request` already does"),
    ("wanted", "what wants it, or what it is"),
    ("want", "what is asked for, or what was expected"),
    ("paired", "a diff is a `Diff`"),
    (
        "touch",
        "what happened to it — a file is `used`, not touched",
    ),
];

/// Vague words forbidden inside declared type names.
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

/// Files allowed to start threads.
pub const THREAD_FILES: &[&str] = &[
    "crates/syntax/src/worker/mod.rs",
    "crates/pipeline/src/file/worker.rs",
    "crates/pipeline/src/list/worker.rs",
    "crates/ui/src/app/threads.rs",
    "crates/ui/src/app/mod.rs",
    "crates/watcher/src/watch.rs",
];
pub const THREAD_MARKERS: &[&str] = &["thread::spawn", "thread::Builder"];

/// Hot-path directories that must not block.
pub const NON_BLOCKING_DIRS: &[&str] = &[
    "crates/ui/src/input",
    "crates/ui/src/draw",
    "crates/ui/src/render",
    "crates/ui/src/view",
];

/// Hot-path files checked like [`NON_BLOCKING_DIRS`].
pub const NON_BLOCKING_FILES: &[&str] =
    &["crates/ui/src/app/keys.rs", "crates/ui/src/app/mouse.rs"];

/// Blocking operations forbidden on hot paths.
pub const BLOCKING_MARKERS: &[&str] = &[
    "std::fs",
    "std::process",
    "std::net",
    "vcs::",
    "vscode_diff::",
    ".recv()",
    ".join()",
];

/// Intra-crate module edges forbidden by architectural boundaries.
pub const BLIND_DIRS: &[(&str, &str, &str)] = &[
    (
        "crates/ui/src/render",
        "crate::view",
        "a brick is handed what it draws; `ui/src/draw` is what knows the model",
    ),
    // Engine vocabulary stays inside `syntax`.
    (
        "crates/ui/src",
        "syntax::engine",
        "the engines' own words live in `syntax`; `ui` asks for a palette and a role",
    ),
];
