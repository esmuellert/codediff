# vcs

Asks a version control system what changed, and reads the two sides of a file.

## One folder per capability

```text
lib.rs        re-exports only
path.rs       RelPath      } shared vocabulary, above every capability
repo.rs       Repo         }
error.rs

diff/         trait Diff       files() · before(f) · after(f)      the reviewer's words
staging/      trait Staging    stage, unstage                      later
history/      trait History    commits, merge_base                 later

git/          rev_parse · cat_file · status · worktree             git's own words
```

Each trait lives with the types in its signatures, so adding a capability means adding a
folder rather than growing a file. A crate named for a whole domain is otherwise an
invitation to put anything in it.

`Diff` names no git concept — no index, no `HEAD`, no blob, no object id — because a system
need not have any of them. jj has no staging area at all. What "before" means is decided
when a backend is constructed, not by the trait.

It is called `Diff` because that is what git and jj both call it, and because `Change` was
already taken: the diff engine reports **line**-level changes, and two meanings of the word
in one pipeline is one too many. `vcs::Diff` is per file; `vscode_diff::LinesDiff` is per
line.

Underneath, `git/` keeps every git word it needs: `XY` codes, `Oid`, the index. Its modules
are named for the commands they run, so the file tree says which command before you open
anything. `git::to_file_diff` is the single place the two vocabularies meet, and the only
thing a second backend would write its own version of.

Capabilities not every system has get their own trait, so a backend lacking one fails to
compile rather than returning "unsupported" at runtime. Only `Diff` exists today.

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
