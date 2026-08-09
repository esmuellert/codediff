//! The rules themselves, and nothing that applies them.
//!
//! A file of tables, so that "what is forbidden here" can be read and argued
//! with in one screen, without the walking and parsing that enforces it. Every
//! entry carries the reason it exists: a rule whose justification has to be
//! looked up elsewhere is a rule that will eventually be deleted by someone
//! who could not find it.

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

/// Words that may not be a name on their own, and what to say instead.
///
/// Unlike [`BANNED_TYPE_WORDS`], which forbids a word *inside* a name, these
/// are forbidden as the *whole* name — of anything we declare or bind: a type,
/// a function, a field, a parameter, a local. `answer` is refused; `answers`,
/// `request_diff` and `SyntaxResponse` are not.
///
/// Every one of them describes the *shape of the interaction* rather than the
/// thing being handled. They read as if they said something, which is what
/// makes them worse than a meta-name: `RowKind` at least admits it is telling
/// you nothing. Each of these was reached for, at least once, instead of a
/// word this codebase already had — `Comparison` where `Diff` existed,
/// `Answer` where `SyntaxResponse` existed, `paired` where the type was
/// already called `Diff`. That is how one idea comes to have two words, which
/// is the failure D28 removed and D61 removed again.
///
/// Checked against the code with string literals stripped: "paired line by
/// line" is prose, and prose is not a name.
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

/// The files allowed to start a thread, and why nowhere else may.
///
/// Concurrency is the easiest thing in this program to get wrong and the
/// hardest to test, so it exists in as few places as can be counted on one
/// hand: one worker per kind of slow work, each a queue in and a queue out.
/// Everything else is single-threaded and can be reasoned about as such.
///
/// A `spawn` outside this list would mean something sharing the view without
/// a queue between them, and the question "which thread owns this?" would stop
/// having an obvious answer. Another background job belongs beside one of
/// these or behind the same channel — not in a new corner. See D41 and D59.
///
/// | file | what it does off the drawing thread |
/// |---|---|
/// | `syntax/src/service/mod.rs` | colours text |
/// | `pipeline/src/file/service.rs` | reads two versions and pairs them |
pub const THREAD_FILES: &[&str] = &[
    "crates/syntax/src/service/mod.rs",
    "crates/pipeline/src/file/service.rs",
    "crates/pipeline/src/list/worker.rs",
    "crates/ui/src/app/threads.rs",
    "crates/ui/src/app/mod.rs",
    "crates/watcher/src/watch.rs",
];
pub const THREAD_MARKERS: &[&str] = &["thread::spawn", "thread::Builder"];

/// Directories reached on every key and every frame, which must not block.
///
/// **The rule is about the thread, not about the crate.** An earlier version
/// forbade `crates/ui/src` from naming `vcs::`, on the grounds that a renderer
/// must not reach git. That rule was already defeated when it was written:
/// `ui` depends on `pipeline`, so `pipeline::list::run` compiles anywhere in
/// `ui`, opens a repository and blocks for as long as git takes — 29 ms on
/// this repository, 296 ms measured on five thousand changed files. Banning
/// the word `vcs::` stopped the cheapest call in the program, `git rev-parse`
/// once at startup, and stopped nothing else.
///
/// So the question is not which crate a call comes from. It is **when it
/// runs**. A call made once, before the terminal is opened, may block: there
/// is nothing to stay responsive with, which is why the file list is read
/// synchronously. A call made while the reader is holding a key may not.
///
/// These directories are only ever reached from inside the loop, so nothing in
/// them may perform IO or wait. `app/mod.rs` holds the loop itself — its
/// `recv()` is the single permitted block — and is not checked. The event
/// handlers in `keys.rs` and `mouse.rs` are checked because they run on
/// every key and every frame.
pub const NON_BLOCKING_DIRS: &[&str] = &[
    "crates/ui/src/input",
    "crates/ui/src/draw",
    "crates/ui/src/render",
    "crates/ui/src/view",
];

/// Files reached on every key and every frame, checked as [`NON_BLOCKING_DIRS`]
/// are.
///
/// The event handlers that run inside the loop. They must never block because
/// they are called between the channel wait and the next draw.
pub const NON_BLOCKING_FILES: &[&str] =
    &["crates/ui/src/app/keys.rs", "crates/ui/src/app/mouse.rs"];

/// What blocking looks like, in the directories above.
///
/// `IO_MARKERS` plus the two ways to wait for something already running.
/// `try_recv` is deliberately absent: it is how the loop collects an answer
/// without waiting, and it is the whole point of the workers.
pub const BLOCKING_MARKERS: &[&str] = &[
    "std::fs",
    "std::process",
    "std::net",
    "vcs::",
    "vscode_diff::",
    ".recv()",
    ".join()",
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
