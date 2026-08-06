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

## B7 — a path with a newline in it cannot be read

**Owner: `vcs`.** `crates/vcs/src/git/cat_file.rs`.

Refused with an error rather than corrupting the stream, which was the bug
before. Reading it needs `git cat-file --batch -Z`, whose NUL framing would set
a floor of git 2.42 — a trade not made anywhere else in this crate, so it is
recorded rather than taken.

---

## B8 — three terminal tests still describe the pre-pathspec screen

**Owner: the terminal tests.** `crates/codediff/tests/terminal.rs`.

`codediff <path>` is a pathspec on the list now (D58), so it draws the split —
list on the left with focus, file on the right — where it used to draw one
buffer. Three tests still assert the old screen and fail:

| test | what it assumes |
|---|---|
| `a_one_sided_file_is_drawn_in_one_pane` | no `│` anywhere; the split always draws one |
| `a_change_key_with_nowhere_to_go_says_so_on_a_real_terminal` | `]c` reaches the diff, not the list |
| `the_layout_key_is_delivered_by_a_real_terminal` | the status line is the diff's, not the list's |

The last two need a `Tab` before their keys. The first needs a different
assertion: with a split there is always one divider, so "no second column" can
no longer be shown by its absence.

They were hidden because `cargo test --workspace` stops at the first failing
target and `--test pipeline` fails earlier. **Run the suite with
`--no-fail-fast`**, or targets after the first failure never run at all.
