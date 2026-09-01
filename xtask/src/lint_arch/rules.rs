//! Architecture rules and their failure messages.

/// Edges that must never exist, with the reason reported on failure.
pub const FORBIDDEN_EDGES: &[(&str, &str, &str)] = &[
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
        "loom",
        "ui",
        "a rendering framework must not depend on its application",
    ),
    (
        "loom",
        "pipeline",
        "the framework paints cells; application pipelines stay above it",
    ),
    (
        "loom",
        "align",
        "the framework lays out nodes without knowing the diff model",
    ),
    (
        "loom",
        "syntax",
        "styles arrive as props; the framework does not compute them",
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
    "crates/loom/src/event",
    "event routing is a pure function of tree state and one event",
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
    "crates/loom/src/run.rs",
    "crates/pipeline/src/diff/worker.rs",
    "crates/pipeline/src/files/worker.rs",
    "crates/syntax/src/worker/mod.rs",
    "crates/ui/src/components/explorer/mod.rs",
    "crates/ui/src/lib.rs",
    "crates/watcher/src/watch.rs",
];
pub const THREAD_MARKERS: &[&str] = &["thread::spawn", "thread::Builder"];

/// Hot-path directories that must not block.
pub const NON_BLOCKING_DIRS: &[&str] = &[
    "crates/loom/src/event",
    "crates/loom/src/hook",
    "crates/loom/src/layout",
    "crates/loom/src/paint",
    "crates/ui/src/components",
    "crates/ui/src/hooks",
];

/// Hot-path files checked like [`NON_BLOCKING_DIRS`].
pub const NON_BLOCKING_FILES: &[&str] = &[
    "crates/loom/src/component.rs",
    "crates/loom/src/current.rs",
    "crates/loom/src/frame.rs",
    "crates/loom/src/node.rs",
    "crates/loom/src/reconcile.rs",
    "crates/loom/src/runtime.rs",
    "crates/loom/src/scope.rs",
    "crates/loom/src/tree.rs",
];

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
    // Engine vocabulary stays inside `syntax`.
    (
        "crates/ui/src",
        "syntax::engine",
        "the engines' own words live in `syntax`; `ui` asks for a palette and a role",
    ),
];
