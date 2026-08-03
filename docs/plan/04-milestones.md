# 04 — Milestones

Seventeen steps from empty repository to MVP. Each ships something, and each has an exact
check a human can run.

**A milestone is done when a human has run its acceptance check and it passed.** Compiling
and green tests are necessary, not sufficient.

Ordering principle: **de-risk by uncertainty × cost-of-being-wrong**, not by user-visible
value. S1–S6 are headless. If the model built in S4 is correct, everything after it is
painting.

---

## Phase A — Foundation (headless, text output)

### S1 — Skeleton, vendored C, FFI

**Build.** Cargo workspace with the crates S1 actually needs — `vscode-diff-sys`,
`vscode-diff`, `codediff` and `xtask`. Copy `libvscode-diff` into `vendor/` with
`UPSTREAM.lock`. `vscode-diff-sys` compiles it with `cc`, OpenMP off. CI: fmt,
`clippy -D warnings`, test, `lint-size`, `lint-arch`, `verify-c`, and a check that every
crate other than `vscode-diff-sys` and `vscode-diff` carries `#![forbid(unsafe_code)]`.

The remaining crates are created by the milestone that needs them. Stubbing them up
front constrains nothing — there is no code in an empty crate to violate a rule — and
the crate set is still moving. `lint-arch` reports which edge rules are waiting on a
crate, so a rule cannot quietly stay dead.

**Check.**
```
cargo build --release
./target/release/codediff doctor
ldd target/release/codediff        # otool -L on macOS
```

**Pass when.**
- [x] `doctor` prints the engine version (`2.60.0`) and reports static linkage
- [x] `ldd` shows **neither `libvscode_diff` nor `libgomp`** — only libc and libm
- [x] `cargo xtask verify-c` passes
- [x] `cargo xtask lint-arch` passes

---

### S2 — Safe diff wrapper, oracle parity

**Build.** `vscode-diff`: `compute()` returning owned Rust types. C memory converted eagerly
and freed immediately — no C pointer escapes into application types. All `unsafe` confined
to `vscode-diff-sys` plus the conversion function.

**Must handle:** an empty side has to be normalised to `[""]` before calling in. The engine
models an empty file as one empty line, following VSCode's document model; a count of 0
silently returns *no changes*, so an entirely-added file would appear unchanged. Pinned by
`zero_lines_is_outside_the_contract_and_silently_reports_nothing` in `vscode-diff-sys`.

**Check.**
```
cargo xtask verify-oracle
codediff debug diff <a> <b>
```

**Pass when.**
- [x] `verify-oracle` prints one row per `test_pairs/*` fixture, **every row PASS**, exit 0
- [x] `debug diff` output is legible and matches the upstream tool by eye on a sample
- [x] the wrapper is leak-clean under Valgrind (0 errors, 0 bytes lost)

---

### S3 — `line-index`, where each character sits

**Build.** `ByteOff` / `CharIdx` / `Utf16Col` / `CellCol` newtypes, conversions, display
width, tab expansion, grapheme-safe slicing by cell range.

**Check.**
```
codediff debug line crates/line-index/fixtures/nasty.txt
```
Lists, per line, every character whose byte / UTF-16 / column positions disagree, with its
display width. Plain ASCII is skipped — there all three are equal, which is why confusing
them goes unnoticed until a file contains a tab or an emoji.

**Pass when.**
- [x] the reported positions are correct for text containing tabs, CJK, emoji ZWJ sequences
      and combining accents
- [x] printed cell widths match the reference table shipped with the fixture
- [x] property tests pass: round-trip conversions, monotonicity, never splits a grapheme

---

### S4 — `align`, pairing the two files — **KEYSTONE** ✅

**Build.** `Alignment` over a `LinesDiff` plus two texts: `Row`/`Slot`/`RowKind`, hunks with
content-hash `HunkId`, inner-change spans resolved to byte ranges via `line-index`, unchanged
regions, moves by line lookup. A **plain-text renderer** — no TUI in this milestone.

It stores no rows and copies no text; every answer is computed from the diff it borrows.
This is VSCode's model — see D18.

**Check.**
```
codediff debug align vendor/test-pairs/<name>/original.txt \
                     vendor/test-pairs/<name>/modified.txt [-v]
```
for all twelve fixture pairs. Output resembles:
```
    1   -- Header comment            │     1   -- Header comment
    3 -                              │         ╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱   ↓ moved to modified 11
    4 - function setup()             │         ╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱
    9                                │     3
```

**Pass when.**
- [x] for each of the twelve pairs, the left column reads as **exactly** the original file
      and the right column as **exactly** the modified file
- [x] fillers sit precisely where lines were added or removed
- [x] change markers identify the right rows
- [x] all six `align` invariants hold under `proptest`
- [x] golden snapshots committed

---

## Phase B — Git

### S5 — `vcs`, reading the repository ✅

**Build.** `Diff` trait and its git implementation: `open`, `files`, `before`, `after`,
`resolve`. `git --no-optional-locks status --porcelain=v2 -z`, typed `StatusEntry` keeping
both status codes. Blobs through one long-lived `cat-file --batch`. `fixtures` crate and
`xtask fixture-repo`. See [D21](05-decisions.md#d21).

**Check.**
```
cargo xtask fixture-repo /tmp/cdfix
codediff debug status /tmp/cdfix -v
diff <(grep -vE '^#|^$' /tmp/cdfix/MANIFEST.txt) <(codediff debug status /tmp/cdfix | ...)
```

**Pass when.**
- [x] output **matches the fixture manifest exactly** — every path and status code
- [x] renames appear as renames with both paths, not as add + delete
- [x] the conflicted file is identified as a conflict
- [x] untracked files and the untracked directory appear correctly
- [x] paths containing spaces and unicode survive intact

---

### S6 — One file, end to end ✅

**Build.** `vcs` → `vscode-diff` → `align` wired together: one path in, both sides found,
compared and paired. `Content` classifies bytes — text, binary, or absent — since `vcs`
hands back `Vec<u8>` and a repository holds pictures as readily as source.

**Check.**
```
cd /tmp/cdfix
codediff debug show HEAD:modified.txt --raw | cmp - <(git show HEAD:modified.txt)
codediff debug diff-file modified.txt
git diff modified.txt                   # compare
```

**Pass when.**
- [x] blob content matches `git show` byte for byte, binaries included
- [x] the aligned diff has the same added and removed lines as `git diff`
- [x] `crlf.txt` produces **no phantom diff** from line-ending handling
- [x] `no-trailing-newline.txt` handles the missing trailing newline correctly
- [x] binary and deleted files are reported, not crashed on

---

## Phase C — TUI, added layer by layer

### S7 — First pixels

**Build.** Terminal lifecycle with a panic hook that restores the terminal. `Pane` /
`Layout` / `View` per [D19](05-decisions.md#d19), two panes over one container-owned row
index, line numbers, gutter, line and inner-change highlighting, status line, themes
([D22](05-decisions.md#d22)) — four Catppuccin flavours derived from the published
palettes, plus a `basic` pair for terminals without 24-bit colour, selected by `--theme` or
detected from the environment. `syntax` crate with the `Highlighter` trait returning empty
spans.

**Check.**
```
codediff <path>
codediff <path> --theme basic-light
codediff --self-panic
codediff doctor
```

**Pass when.**
- [x] the diff renders side by side, correctly coloured, matching `debug diff-file` row for row
- [x] dragging the split resizes both panes; **no scroll synchronisation code exists**
- [x] `q` exits and the shell prompt is **intact** — cursor visible, no alt-screen residue
- [x] `--self-panic` panics and **still restores the terminal**
- [x] resizing during use reflows without corruption
- [x] `Ctrl-Z` then `fg` works
- [x] screen snapshots committed
- [x] every theme draws the same characters and only the colours differ
- [x] `basic-*` emits no 24-bit colour, so a terminal without it still shows a diff

**Deferred, deliberately.** The event loop is a plain blocking `read` rather than the
channel-and-`Command` shape: there is nothing yet to run off-thread, and installing the
seam before it has a second producer would be guessing at its shape. It arrives with the
watcher at S12, which is its first real caller.

`SpanSet` likewise. There are exactly two layers today — line background and inner change —
and a compositor for two layers is a `match`. It earns its keep when syntax colours,
search matches and review marks all want the same cells, at S11.

**Found while building.** A file that exists on only one side has nothing to compare
against, so it is not compared at all: one pane, no highlighting, labelled `(added)` or
`(deleted)`. VSCode reached the same conclusion from the same bug — see
[D23](05-decisions.md#d23). The decision is *absent*, never *empty*: a tracked file emptied
to zero bytes still gets a real two-pane diff.

---

### S8 — Scroll, cursor, motions

**Build.** Viewport, cursor, shared `scroll_offset`, thin motion set, key dispatch state
machine with pending sequences and count prefixes.

**Check.**
```
codediff <a file with a few thousand changed lines>
```

**Pass when.**
- [x] `j k Ctrl-D Ctrl-U gg G` and counts (`5j`) behave correctly
- [x] **both panes always show the same logical rows** — verifiable from the row gutter
- [x] no flicker while scrolling fast; holding `j` stays smooth
- [x] the cursor line is highlighted and never scrolls off screen

**Measured.** Nothing clears the screen between frames, so ratatui emits only the cells that
changed. 5000 lines at 120×40 costs **490 µs a frame**, 33× under a 60 fps budget; the diff
and alignment behind it cost 27 ms, once, at open.

---

### S9 — Change navigation

**Build.** `]c` / `[c`, change index in the status line, landing position. At the last change
the cursor **stays put and the status line says so** — see *Stopping, by design* below.

**Check.** Open a file with many changes, press `]c` repeatedly past the end.

**Pass when.**
- [x] `]c` / `[c` land on the next and previous change, never mid-change
- [x] the status line reads `change 3/17` and stays accurate
- [x] `]c` at the last change stays there and reports `no next change`
- [x] panes never desynchronise after a jump

**Stopping, by design.** The original wording was "wrapping at the last hunk behaves as
configured", which could not be met here at all: there is no configuration until **S17**, so
the criterion depended on a milestone eight ahead of it. Rather than carry a permanently
unmeetable box, the behaviour is now decided rather than deferred — `]c` stops.

That is also the better default for this tool, not merely the reachable one. Wrapping
silently back to the top destroys the one signal that matters when checking an agent's work:
that you have now seen everything. In an editor you are usually hunting one spot and
cycling helps; in a reviewer you are covering all of them, and "finished" must not look like
"going round again".

The three modes stay available later, and cost little once there is a config file to choose
between them: don't cycle (today), cycle within the file, cycle across files. The third also
needs more than one file open, so **S12** at the earliest.

**Counting blocks, not hunks.** The status line and `]c` both count *runs of changed rows*,
never the engine's hunks. Hunks merge changes a few lines apart, which is right for
collapsing context and wrong for navigation, where it would make two nearby edits one stop.
Both read the same `blocks`, so they cannot disagree — the bug that motivated it.

**Found while building.** `n` / `N` were bound here first, which would have left search — `/`
`n` `N` in [D9](05-decisions.md#d9--a-deliberately-thin-motion-set) — with nothing to bind.
Rebound to `]c` / `[c`, vim's own diff-change motions, restoring the split D9 always
specified. Search itself is in D9 but has no milestone.

---

### S10 — Horizontal scroll and long lines

**Build.** Shared horizontal offset, grapheme-safe slicing, inner-change spans remapped
through tab expansion. This is the **default** answer to long lines; wrapping is opt-in and
arrives in S10a.

**Check.** Open `src/longlines.rs`, `src/tabs.rs` and `src/unicode.rs`; scroll right.

**Pass when.**
- [x] both panes scroll horizontally together
- [x] **inner-change highlights stay on the correct characters at every offset**
- [x] no character is ever split mid-grapheme at the pane edge
- [x] CJK and emoji do not shift the columns

**Found while auditing.** The highlight criterion held, but nothing pinned it: every test of
the marks ran at offset zero on ASCII, where byte, cell and scroll all coincide and any
wrong formula gives the right answer. Two sabotages — looking the style up by cell instead
of byte, and subtracting the scroll from the byte offset as well as from the column — left
the whole suite green. Now covered at a non-zero offset, behind a tab, behind a wide
character, and end to end on a real screen.

---

### S10a — Optional line wrapping

**Build.** Opt-in wrap. `line-index` gains "break this line into rows of width W". `ui`
computes each line's row count at its pane width, pairs ranges by **row** height rather than
line count, and pads the shorter side after the range. Viewport position becomes
`(row, subrow)`; the row index is rebuilt on resize.

Because the split is draggable the two panes differ in width, so identical unchanged text
wraps differently on each side and needs its own checkpoints — VSCode's
`handleAlignmentsOutsideOfDiffs`. See [D19](05-decisions.md#d19).

**Check.**
```
codediff --file src/longlines.rs src/longlines.rs   # then toggle wrap, then drag the split
```

**Pass when.**
- [ ] with wrap off, behaviour is unchanged and no per-line height is computed
- [ ] with wrap on, no line is cut off at the pane edge
- [ ] the two sides stay aligned when they wrap to different heights
- [ ] dragging the split rewraps and stays aligned
- [ ] inner-change highlights stay on the right characters across a wrap boundary

---

### S11 — Syntax highlighting

**Build.** `syntect` (with `two-face`) behind the `Syntax` trait in `crates/syntax`. Composition with diff
and inner-change spans via the `SpanSet` priorities built at S7. Filetype detection.

**Check.** Open the twelve S11 fixtures — Rust, TypeScript, JavaScript, Python, Go, Java, C,
C++, JSON, YAML, Markdown, Bash. Toggle syntax on and off with a key to A/B compare.

**Pass when.**
- [ ] keywords, strings and comments are coloured correctly in all twelve
- [ ] **diff backgrounds remain visible underneath syntax foregrounds**
- [ ] a line that is both changed and contains a string shows both correctly
- [ ] character-level inner-change highlighting still wins where they overlap
- [ ] an unrecognised file type renders as plain text rather than failing
- [ ] no measurable scroll lag on a 5000-line file

---

### S12 — Explorer

**Build.** `explorer` crate: entries → grouped tree, path collapsing, filter. `ui`:
explorer pane, selection, expand and collapse, focus switching. Lazy per-file diff with a
cache, computed concurrently.

**Check.**
```
cd /tmp/cdfix && codediff
```

**Pass when.**
- [ ] the list contains **exactly the manifest files** with correct status letters
- [ ] `src/both.rs`, staged and then edited again, appears **once**, carrying both codes
- [ ] the conflicted file is listed and marked as conflicted, with no merge view offered
- [ ] the rename shows both old and new paths
- [ ] `Tab` switches focus; `Enter` opens the diff; `j`/`k` moves; folds work
- [ ] a file with a very long path truncates without breaking the layout

---

### S13 — Full scenario, end to end

**Build.** Wire `codediff` with no arguments to the complete flow. Handle every awkward
file type in the UI rather than by crashing.

**Check.** Run in `/tmp/cdfix`, then in a **real** dirty repository (`~/codediff.nvim`).
Open every changed file in turn.

**Pass when.**
- [ ] every file in both repositories opens without a crash
- [ ] binary files show a clear "binary file" message, not garbage
- [ ] deleted and untracked files render sensibly (one side empty)
- [ ] the 5000-line file opens in under 200 ms
- [ ] no misalignment anywhere, in any file
- [ ] memory stays flat while browsing all files

---

## Phase D — Live

### S14 — Event loop and async loading

**Build.** Formalise `Event` / `Command` / effect runner. Worker pool. `RequestId`
generations with stale-drop. Loading states. `--debug-events`.

**Check.** Run in a repository with 200+ changed files.

**Pass when.**
- [ ] the explorer appears in **under 300 ms**
- [ ] selecting a file shows a loading indicator, then the diff
- [ ] **holding `j` in the explorer stays smooth while diffs load in the background**
- [ ] selecting rapidly through many files never shows the wrong file's diff
      (proves stale-drop works)
- [ ] `--debug-events` produces a replayable log

---

### S15 — File watcher and targeted refresh

**Build.** `notify` plus `notify-debouncer-full` (~50 ms debounce). Watch non-ignored
worktree **directories** recursively and `.git/` non-recursively. Classify events, filter
lock files **by destination path**, suppress refresh while our own git subprocess is in
flight, targeted refresh, position restoration by `(path, HunkId)`, `Flag::Rescan` handling,
`PollWatcher` fallback, opt-out config. See
[D16](05-decisions.md#d16--watcher-design-informed-by-upstream-production-failures).

**Depends on S17.** "Opt-out config" needs the config file, which is S17. Decide the default
here and leave the switch to S17, as S9 did — a criterion that depends on a later milestone
cannot be met in order.

**Check.** With codediff open, in another terminal: edit the open file; `touch newfile.txt`;
`git commit`. Then leave it completely idle for 60 s with a CPU probe attached.

**Pass when.**
- [ ] a working-tree edit surfaces within **100 ms**
- [ ] `touch newfile.txt` — a **new untracked file** — surfaces within **100 ms**
      (this is the regression upstream shipped in #480; it must not recur)
- [ ] an external `git commit` surfaces within **100 ms**
- [ ] **the cursor stays on the same hunk**; no flicker and no scroll jump
- [ ] `--debug-events` proves **exactly one file was re-diffed**, not the whole tree
- [ ] idle CPU **≤ 5 ms per 10 s**
- [ ] **zero git subprocesses fired while idle** over 60 s — verified with a process counter
- [ ] no self-triggering loop: codediff's own `git status` never wakes its own watcher
- [ ] creating a file inside `target/` or `node_modules/` triggers **nothing**
- [ ] watch count stays proportional to directory count, not file count
- [ ] killing the process leaves **no orphaned threads**

---

### S16 — Index and HEAD watching

**Build.** Classification of `.git` events into the correct refresh kind.

**Check.** With codediff open, in another terminal run `git add`, `git reset`, `git stash`,
and a branch switch.

**Pass when.**
- [ ] `git add X` updates X's status codes in place, **selection preserved**
- [ ] `git reset` puts them back
- [ ] `git stash` empties the list; `git stash pop` restores it
- [ ] a branch switch triggers one full refresh, not a storm
- [ ] no `index.lock` contention is ever caused by codediff

---

### S17 — Help, config, polish — **MVP**

**Build.** Keybinding help overlay, config file at the XDG path, `--help` / `--version`,
error surfaces in the status line, README.

**Check.**
```
codediff --help
codediff --version
cd /tmp/cdfix && codediff        # press ?
```

**Pass when.**
- [ ] `?` shows an accurate, complete keybinding list
- [ ] a config file changing theme and tab width takes effect
- [ ] a git failure surfaces in the status line rather than panicking
- [ ] running outside a git repository gives a clear message
- [ ] `--help` and `--version` are sane
- [ ] **all of S1–S16 still pass**

---

## Summary

| phase | milestones | delivers |
|---|---|---|
| A — foundation | S1–S4 | correct diffs and a correct aligned model, provable in text |
| B — git | S5–S6 | real repository data |
| C — TUI | S7–S13 | the complete single-file and multi-file review experience |
| D — live | S14–S17 | asynchrony, watching, refresh, polish |

**Estimate: 9–12 weeks focused.** Heaviest: S4, S7, S11, S12.

## Open questions

Settle before the milestone noted.

| # | question | needed by | recommendation |
|---|---|---|---|
| 1 | ~~three-state explorer or simple worktree-vs-HEAD?~~ | S5 | **settled: one list, worktree vs HEAD.** Staging cannot be acted on from a read-only tool, and git reports both status codes anyway, so the split is additive later. Conflicted files are listed and marked, never given a merge view — three-way is a different model, not a mode, and VSCode likewise keeps `mergeEditor` separate from `diffEditor` |
| 3 | include inline (single-pane) mode in MVP? | S7 | **no** — it is a projection over the same model, roughly two days to add later |

*Question 2 (syntax engine) is settled — see [D11](05-decisions.md#d11--syntax-highlighting-is-in-the-mvp-via-syntect).*
