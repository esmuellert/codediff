# file-types

What a file under review **is** — not how one is read, diffed, or drawn.

## Why it exists

A file passes through four layers on its way to the screen: `vcs` finds it, the pipeline
reads and diffs it, `ui` draws it. Before this crate, each layer declared its own partial
answer to "which file is this", and the answer got worse at every step:

```text
RelPath(String)                             typed
FileDiff { path, previous_path, kind }      typed, structured
"old.rs → new.rs   (added)"                 a String — three facts fused
Status { path: &str }                       called path; is not one
```

The last step is the damage. Once fused, the status line bolds `(added)` as though it were
part of the path, and cannot shorten a long path because nothing can find where the path
ends. The facts were destroyed, not hidden.

VSCode has the same bug, filed as issue #110694 — *"the tab title … is too long:
`very/long/path/file1.js <-> very/long/path/file2.js`"*. The fix works precisely because it
truncates the two paths **while they are still separate values**. `codediff.nvim` went
further and lost the facts entirely: it consumes `status` and `old_path` as control flow
when opening a diff and never stores them, so its diff view genuinely cannot answer "is
this file a rename?".

This crate is the fix for both: one vocabulary, named by every layer, so identity is never
converted and never flattened.

## What is in it

```text
RepoPath      where a file lives — both spellings, one constructor
File          which file this is: a version on each side, either absent
ChangeType    what happened to it — four derived, two only a backend knows
ChangedFile   a File, plus what a backend had to tell us about it
FileContent   what one version holds — text, a binary blob, or nothing
DiffVersion   which of the two: Original or Modified
```

Six types, no dependencies, no build script.

## This is also the backend contract

`ChangedFile` is what a version control backend must produce, and therefore what
everything downstream receives — whether the backend is git, jj, or something not written
yet. Nothing here names a version control concept: **no index, no `HEAD`, no blob and no
object id**, because a system need not have any of them. jj has no staging area at all.
What "before" means is decided when a backend is constructed, not here.

There is deliberately **no trait**. `vcs` had one, with a single implementor, no generic
use, and every call site importing it as `Changes as _` — so it was an inherent `impl`
wearing a trait's clothes. The contract it claimed to enforce was never its own: it came
from the types in its signatures, all of which are here, and from `cargo xtask lint-arch`
forbidding this crate from naming `vcs`. A lint is not opt-in; a trait is.

What checks a backend has met the contract is **the pipeline that calls it**, which is the
stricter test: a trait proves four methods exist, while the pipeline proves they are the
methods actually needed and that their results compose. A second backend earns a trait
extracted from two real implementations. See [D30](../../docs/plan/05-decisions.md#d30).

## What is deliberately *not* in it

| | belongs to | because |
|---|---|---|
| `ChangeType`, `similarity` | `vcs` | `Untracked` and `Conflicted` are git's answers, not a file's |
| `Alignment`, `Row`, `Hunk` | `align` | facts about the *relationship* between two files |
| `Column`, `Frame` | `ui` | rectangles. Naming one `Column` was a mistake about names, not about layers |
| `SideBySide`, `SingleFile` | `ui` | ways of *showing* a file; each holds a [`File`] |
| a `Lines` newtype | nowhere | two users, no behaviour — a wrapper around `Vec<String>` earns nothing |

The line, stated once:

> **A file** — what it is called, what is in it, which version. → here
> **A pairing** — how two files line up. → `align`
> **A presentation** — how one is shown. → `ui`
> **A repository** — what git says about it. → `vcs`

## Absence is `Option`, never `""`

`codediff.nvim` carries both path forms in one table and uses `""` for "not applicable"
(`lua/codediff/core/path.lua:53-56`). Its `relative = ""` means three different things —
no file, the file *is* the root, or the file lives outside the root — and its `is_empty`
requires *both* fields empty, so a file outside the root reads as present with no relative
path.

Here a missing version is `None`, a missing path cannot be constructed, and `RepoPath`'s
fields are private so no third state can be invented.
