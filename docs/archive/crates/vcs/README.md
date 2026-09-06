# vcs

Asks a version control system what changed, and reads the two sides of a file.

## What is here, and what is not

```text
repository/   the whole surface
├ mod.rs        Repository   open · changes · counts · read
├ diff_type.rs  DiffType     the five ways to compare
├ changes.rs    Changes      files that share a comparison
└ changed_file.rs            git's records, in the reviewer's terms
repo.rs       Repo           where the repository is, and where it keeps its state
error.rs      Error          how running a version control system fails

git/          PRIVATE — one file per command, named as git spells it
├ run           spawn git, capture, fail loudly
├ rev_parse     git rev-parse --show-toplevel | --absolute-git-dir | --verify
├ status        git status --porcelain=v2 -z
├ diff/         git diff --name-status -z  ·  git diff --numstat -z
├ merge_base    git merge-base
├ cat_file      git cat-file --batch | --filters
└ worktree      std::fs — the third thing git compares, and not a command
```

**Two layers, and the test for each.** A file in `git/` **runs one command and parses what
it printed into git's own words** — `XY` codes, status letters, similarity scores. Nothing
there decides anything. `repository/` **turns those into the standard format**, and its files
are named for what a reviewer would call them.

`git` is private, so nothing outside can run a git command, name a status code, or hold a
`--cached`. A second backend is a directory beside it and an arm in `Repository::open` —
not a search for every caller that reached past the layer. See
[D67](../../docs/plan/05-decisions.md#d67).

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
`Repository`'s methods: a trait proves four methods exist, while the pipeline proves they
are the methods needed and that their results compose. A second backend earns a trait extracted from
two real implementations. See [D30](../../docs/plan/05-decisions.md#d30).

**`git diff` never computes a difference.** It is run for `--name-status` and `--numstat`,
which are lists rather than diffs; the difference itself is the engine's job, two stages
later, which is why nothing here is called `Diff`
([D29](../../docs/plan/05-decisions.md#d29)).

**One function reads either side**, because which side it is says nothing about where to
look: `Rev::stored()` gives git's spelling for anything in the object store and nothing for
the file on disk, so the choice is data rather than a branch written into the function. It
was two functions once, and the after side could not be anything but the working tree.

`repository/changed_file.rs` is the single place the two vocabularies meet, and **the file a
second backend forks** — not `Repository`, whose four operations would not change.

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
