# vcs

Asks a version control system what changed, and reads the two sides of a file.

## What is here, and what is not

```text
lib.rs        re-exports only
repo.rs       Repo         where the repository is, and where it keeps its state
error.rs      Error        how running a version control system fails

git/          rev_parse · cat_file · status · worktree             git's own words
```

**Every type this crate produces lives in `file-types`** — `ChangedFile`, `File`,
`RepoPath`, `FileContent` — and `cargo xtask lint-arch` forbids that crate from naming this
one, so no git concept can reach a reviewer. This crate holds the verbs; `file-types` holds
the nouns.

`Repo` stays here because it is a property of the *repository*, not of a file: `control_dir`
is where git keeps its own state, which the file watcher needs and no file has. The root is
recoverable from any `RepoPath`, which carries both spellings.

**There is no trait.** There was one, with a single implementor, no generic use, and every
call site importing it as `Changes as _` — an inherent `impl` wearing a trait's clothes. It
claimed to enforce neutrality, but that came from the types in its signatures and from the
lint, not from the trait itself. What actually checks a backend is the pipeline that calls
`Git`'s methods: a trait proves four methods exist, while the pipeline proves they are the
methods needed and that their results compose. A second backend earns a trait extracted from
two real implementations. See [D30](../../docs/plan/05-decisions.md#d30).

**`git diff` is never run.** `files()` is `git status`; `read()` is `git cat-file` or
`std::fs`, chosen by the file's own `Rev`. Computing the difference is the engine's job,
two stages later — which is why nothing here is called `Diff`
([D29](../../docs/plan/05-decisions.md#d29)).

**One function reads either side**, because which side it is says nothing about where to
look: `Rev::stored()` gives git's spelling for anything in the object store and nothing for
the file on disk, so the choice is data rather than a branch written into the function. It
was two functions once, and the after side could not be anything but the working tree.

Underneath, `git/` keeps every git word it needs: `XY` codes, the index, the stage numbers.
Its modules are named for the commands they run, so the file tree says which command before
you open anything. `git::to_file_diff` is the single place the two vocabularies meet, and
the only thing a second backend would write its own version of.

## Why it runs `git` rather than linking a library

`gix` and `git2` are real alternatives, and speed is not the reason to avoid them — a
`git status` on a 340-file repository takes about **4.5 ms**.

The reason is that git's own binary already honours the user's config, `.gitignore` rules,
linked worktrees, sparse checkout and clean filters. Those rules decide *which files appear
at all*, so a reimplementation that differs anywhere shows the wrong list.

## Three details that break naive implementations

**A rename record spans two NUL-terminated fields.** `git status --porcelain=v2 -z` writes
`2 R. ... moved.txt\0tomove.txt\0` — splitting the stream on NUL and treating each piece as
a record turns one rename into a record plus a garbage entry.

**`--no-optional-locks` goes before the subcommand.** As a `status` flag it is rejected. It
stops git taking `.git/index.lock` for the optional index refresh, which would both fail a
concurrent `git add` and wake the file watcher that asked for the status.

**Field offsets differ per record type.** `1` carries two hashes, `2` adds a similarity
score, `u` has three stages and so three modes and three hashes. Counting wrong puts a hash
in the path — which is what happened, and what the fixture caught.
