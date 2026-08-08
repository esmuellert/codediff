# Known bugs

Found and not yet fixed. Each says what is wrong, how to see it, and **which
layer owns it** — because a bug fixed in the wrong layer is worse than one left
alone: it hides the real fault and makes the layer that received bad input look
correct.

Fixed bugs are not listed here. They are in the decisions, with the test that
would catch them coming back.

---

## B1 — a path is not resolved against where the reader is standing

**Owner: the command line.** `crates/codediff/src/cli.rs`.

```
$ cd crates/ui && codediff src/app.rs
Error: src/app.rs is neither changed nor present; git reports 59 changed file(s)
```

`crates/ui/src/app.rs` is right there. Git's own commands take a path relative
to the current directory; this takes one relative to the repository root, so a
reader in a subdirectory has to spell out the whole path.

**Why it is the command line's.** The pipeline receives a path already in the
form the repository uses, and it is right to trust that. Turning what a reader
typed into that form is what a boundary is for — one place, where a string from
the shell becomes a domain type. Fixing it downstream was tried and reverted:
it left the command line still handing on whatever it was given, and put the
knowledge of "where is the reader standing" three layers below the only place
that can answer it.

**What the fix has to handle**, all three found by driving the program:

- an **absolute** path
- a path with `..` in it
- a **deleted** file, whose path cannot be canonicalised because it is gone —
  and which is exactly a file worth reviewing
- a **tracked symlink**, which must be found under the name git listed rather
  than resolved to what it points at

---

## B2 — a path outside the repository is accepted

**Owner: the command line.** Same boundary as B1.

```
$ codediff debug diff-file /etc/hosts
/etc/hosts
Modified
before   absent
after    476 bytes of text
```

It reports that `/etc/hosts` has been added to your repository. The review
itself refuses, but for the wrong reason — it says the file "has not changed",
which is the unchanged-file refusal catching it by accident rather than
anything noticing the path is not ours. Whatever
resolves a path has to answer "is this inside the repository at all", and say
so when it is not.

---

## B3 — a mode-only change opens a diff that looks unchanged

**Owner: `vcs`.** `crates/vcs/src/git/status.rs`.

```
$ chmod +x run.sh && codediff
Changes (1)          │ 1 echo hi │ 1 echo hi
└ run.sh          M
```

Both sides are identical, because only the mode moved. The porcelain record
carries the old and new modes and the parser drops them, so nothing downstream
can say what changed.

---

## B4 — a submodule cannot be reviewed

**Owner: `vcs`.**

A submodule's content is a commit id, not bytes. `worktree::read` now answers
`None` for a directory, so the row is listed and says it cannot be shown rather
than failing — but the commit-id transition, which is the whole content of the
change, is not shown. Full support means carrying the gitlink mode through the
status parser.

---

## B5 — a git command can hang for ever

**Owner: `vcs`.** `crates/vcs/src/git/run.rs`, `cat_file.rs`.

`Command::output` blocks with no timeout, and the batch reader blocks on the
child's stdout. A clean or smudge filter that hangs — git-lfs fetching over a
slow network is the ordinary case — hangs the review before the terminal even
opens, with nothing on screen to say why.

`GIT_TERMINAL_PROMPT=0` prevents a prompt but not a hang.

---

## B6 — a very large file is copied several times before any limit applies

**Owner: the file pipeline.**

Both sides are read whole, duplicated into `CString`s for the engine, and
copied again into the alignment. A 200 MB file is therefore several hundred
megabytes before the engine's own timeout can fire. There is no size limit
anywhere.

---

## B7 — a path with a newline in it corrupts the read stream

**Owner: wherever a path enters.** `crates/vcs/src/git/status.rs`,
`name_status.rs`, and the pathspec on the command line.

`git cat-file --batch` takes one request per line, so a path holding a newline
is read as two requests, and every answer after it belongs to the wrong file —
silently, which is the bad part.

**Not `cat_file.rs`'s to check.** It was checked there for one commit, and that
was the wrong place: a path is vetted where one enters, not at each of the
places that use it. `read()` takes the path as good, as every other function
handed a `RepoPath` does.

Every other git command here already avoids the problem upstream, by asking for
NUL framing: `status --porcelain=v2 -z`, `diff --name-status -z`,
`diff --numstat -z`. Two tests assert it works — `git_status.rs`'s
`a_path_containing_a_newline_survives`, and `name_status.rs`'s, whose comment
reads *"What `-z` is for."*

So git hands us a path that git will not take back: `status -z` parses
`two\nlines.txt`, the explorer lists it, and pressing enter desynchronises the
stream.

**Two ways to fix it, both upstream of the read.**

*Sanitise where a path enters.* One check, at the boundary, and everything
below stays simple. Such a file cannot then be reviewed, but it is named rather
than silently mixed up with another.

*Take the framing git offers.* `cat-file --batch -Z` frames with NUL and the
problem does not exist. It sets a floor of git 2.42 (September 2023) — Debian
12 ships 2.39 and Ubuntu 22.04 ships 2.34, and `-Z` on an older git fails at
startup rather than degrading. Nothing here declares a minimum git version
today, and `doctor` checks none.

---

## B8 — three terminal tests still describe the pre-pathspec screen ✅ fixed

**Was: the terminal tests.** `crates/codediff/tests/terminal.rs`.

`codediff <path>` is a pathspec on the list (D58), so it draws the split — list
on the left with focus, file on the right — where it used to draw one buffer.
Two of the three tests needed a `Tab` before their keys, so the diff has focus
when `]c` and `t` arrive.

The third could not be fixed by pressing a key: it showed "no second column" by
the *absence* of `│`, and the split always draws one. It counts gutters now — a
file with two sides numbers both, a one-sided file numbers only its own.

A fourth thing surfaced while fixing them: `the_layout_key_is_delivered_by_a_
real_terminal` asserted on `1/4` in the status line, and taking focus rewrites
only the cells that changed — the count goes from the list's `2/2` to the
diff's `1/4` a digit at a time, so the phrase is never on the wire even though
it is on the screen. It asserts on the fourth gutter instead, which is the grid
rather than the redraw strategy.

**The trap this hid behind is still real.** `cargo test --workspace` stops at
the first failing target, so targets after it never run. Use `--no-fail-fast`.

---

## B9 — a wide path loses the position instead of its directory

**Owner: the status line.** `crates/ui/src/draw/status.rs`, `name()`.

It counts `chars()` where a terminal counts columns, so a CJK path measures at
two-thirds of the width it takes and keeps a directory there is no room for.
Two paths that are both eighteen columns wide, at width 36:

```text
abcdef/filename.rs  →  " filename.rs      3 changes   1/100 "
なまえ/ファイル.rs    →  " なまえ/ファイル.rs                 "
```

The ASCII one drops its directory, as it should. The CJK one keeps it and
silently loses `3 changes 1/100` — the position gone rather than the part a
reviewer can most afford to lose.

**The fix is already in place to be called.** `render::fit` is what the file
list narrows a row with, and it counts columns; D65 made it reachable from
here. `name()` becomes a list of `Piece`s and one call. The one thing to get
right is which width to pass: `room` is what is left before the position, but
the row itself is the hard limit, because a name that will not fit beside the
position pushes the position out rather than being cut.
