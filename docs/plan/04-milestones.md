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

**Build.** Cargo workspace with all ten shipped crates stubbed (plus `xtask`; `fixtures`
arrives at S5) and **the complete dependency graph declared in `Cargo.toml` before any logic
exists** — from this point, an architecture violation is a compile error. Copy `libvscode-diff` into `vendor/` with `UPSTREAM.lock`.
`vscode-diff-sys` compiles it with `cc`, OpenMP off. CI: fmt, `clippy -D warnings`, test,
`lint-size`, `lint-arch`, `verify-c`, and a check that the seven crates other than
`vscode-diff-sys` and `vscode-diff` carry `#![forbid(unsafe_code)]`.

**Check.**
```
cargo build --release
./target/release/codediff doctor
ldd target/release/codediff        # otool -L on macOS
```

**Pass when.**
- [ ] `doctor` prints the engine version (`2.59.0`) and `linkage: static`
- [ ] `ldd` shows **neither `libvscode_diff` nor `libgomp`** — only libc and libm
- [ ] `cargo xtask verify-c` passes
- [ ] `cargo xtask lint-arch` passes

---

### S2 — Safe diff wrapper, oracle parity

**Build.** `vscode-diff`: `compute()` returning owned Rust types. C memory converted eagerly
and freed immediately — no C pointer escapes into application types. All `unsafe` confined
to `vscode-diff-sys` plus the conversion function.

**Check.**
```
cargo xtask verify-oracle
codediff debug diff <a> <b>
```

**Pass when.**
- [ ] `verify-oracle` prints one row per `test_pairs/*` fixture, **every row PASS**, exit 0
- [ ] `debug diff` output is legible and matches the upstream tool by eye on a sample
- [ ] the wrapper is leak-clean under ASAN

---

### S3 — `metrics`, text measurement

**Build.** `ByteOff` / `CharIdx` / `Utf16Col` / `CellCol` newtypes, conversions, display
width, tab expansion, grapheme-safe slicing by cell range.

**Check.**
```
codediff debug measure --file crates/metrics/fixtures/nasty.txt
```
Prints each line, a **cell ruler beneath it**, and a conversion table per grapheme boundary.

**Pass when.**
- [ ] the ruler **visually aligns** under text containing tabs, CJK, emoji ZWJ sequences and
      combining accents
- [ ] printed cell widths match the reference table shipped with the fixture
- [ ] property tests pass: round-trip conversions, monotonicity, never splits a grapheme

---

### S4 — `align`, the aligned model — **KEYSTONE**

**Build.** `AlignedDoc` from a `Diff` plus two texts. `Row`/`Cell`/`RowKind`. Hunks with
content-hash `HunkId`. Inner-change spans resolved to byte ranges via `metrics`. Projections
(side-by-side today, inline and compact later). Navigation primitives. A **plain-text
renderer** — no TUI in this milestone.

**Check.**
```
codediff debug align crates/align/fixtures/pairs/<name>/{original,modified}.txt
```
for all twelve fixture pairs. Output resembles:
```
   1 │ fn main() {                │   1 │ fn main() {
   2 │     let x = 1;             │   2 │     let x = 42;
   3 │     println!("hi");        │   3 │     println!("hello");
     │ ╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱╱ │   4 │     return;
   4 │ }                          │   5 │ }
```

**Pass when.**
- [ ] for each of the twelve pairs, the left column reads as **exactly** the original file
      and the right column as **exactly** the modified file
- [ ] fillers sit precisely where lines were added or removed
- [ ] change markers identify the right rows
- [ ] all six `align` invariants hold under `proptest`
- [ ] golden snapshots committed

---

## Phase B — Git

### S5 — `vcs`, status parsing

**Build.** `VcsBackend` trait, git subprocess implementation. `git status --porcelain=v2 -z
--no-optional-locks`, typed `StatusEntry`, `ContentSource` enum. `xtask fixture-repo`.

**Check.**
```
cargo xtask fixture-repo /tmp/cdfix
cd /tmp/cdfix && codediff debug status
```

**Pass when.**
- [ ] output **matches the fixture manifest exactly** — every path, status code and group
- [ ] renames appear as renames with both paths, not as add + delete
- [ ] the conflicted file is identified as a conflict
- [ ] untracked files and the untracked directory appear correctly
- [ ] paths containing spaces and unicode survive intact

---

### S6 — Blob reading and single-file diff

**Build.** Long-lived `git cat-file --batch` child process for blob reads. Worktree reads.
Wire `vcs` → `vscode-diff` → `align` for one file.

**Check.**
```
cd /tmp/cdfix
codediff debug show HEAD:src/changed.rs
codediff debug diff-file src/changed.rs
git diff src/changed.rs                 # compare
```

**Pass when.**
- [ ] blob content matches `git show HEAD:src/changed.rs` byte for byte
- [ ] the aligned diff has the same added and removed lines as `git diff`
- [ ] `src/crlf.rs` produces **no phantom diff** from line-ending handling
- [ ] `src/nonewline.rs` handles the missing trailing newline correctly
- [ ] binary and deleted files are reported, not crashed on

---

## Phase C — TUI, added layer by layer

### S7 — First pixels

**Build.** Terminal lifecycle with a panic hook that restores the terminal. Layout, two
panes rendering an `AlignedDoc`, line numbers, gutter, line and inner-change highlighting,
status line, theme table. Event loop *shape* installed (channel plus
`update(state, event) -> Vec<Command>`) even with only Key, Resize and Tick. `SpanSet`
compositor with priorities. `syntax` crate with the `Syntax` trait returning empty spans.

**Check.**
```
codediff --file a.rs b.rs
codediff --self-panic
```

**Pass when.**
- [ ] the diff renders side by side, correctly coloured, matching `debug align` row for row
- [ ] `q` exits and the shell prompt is **intact** — cursor visible, no alt-screen residue
- [ ] `--self-panic` panics and **still restores the terminal**
- [ ] resizing during use reflows without corruption
- [ ] `Ctrl-Z` then `fg` works
- [ ] screen snapshots committed

---

### S8 — Scroll, cursor, motions

**Build.** Viewport, cursor, shared `scroll_offset`, thin motion set, key dispatch state
machine with pending sequences and count prefixes.

**Check.**
```
codediff --file /tmp/cdfix/big-a.rs /tmp/cdfix/big-b.rs      # 5000 lines
```

**Pass when.**
- [ ] `j k Ctrl-D Ctrl-U gg G` and counts (`5j`) behave correctly
- [ ] **both panes always show the same logical rows** — verifiable from the row gutter
- [ ] no flicker while scrolling fast; holding `j` stays smooth
- [ ] the cursor line is highlighted and never scrolls off screen

---

### S9 — Hunk navigation

**Build.** `]c` / `[c`, hunk index in the status line, wrap behaviour, landing position.

**Check.** Open a file with many hunks, press `]c` repeatedly past the end.

**Pass when.**
- [ ] `]c` / `[c` land on the next and previous change, never mid-hunk
- [ ] the status line reads `hunk 3/17` and stays accurate
- [ ] wrapping at the last hunk behaves as configured
- [ ] panes never desynchronise after a jump

---

### S10 — Horizontal scroll and long lines

**Build.** Shared horizontal offset, grapheme-safe slicing, inner-change spans remapped
through tab expansion.

**Check.** Open `src/longlines.rs`, `src/tabs.rs` and `src/unicode.rs`; scroll right.

**Pass when.**
- [ ] both panes scroll horizontally together
- [ ] **inner-change highlights stay on the correct characters at every offset**
- [ ] no character is ever split mid-grapheme at the pane edge
- [ ] CJK and emoji do not shift the columns

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

**Build.** `explorer` crate: entries → grouped tree, path collapsing, filter. `display`:
explorer pane, selection, expand and collapse, focus switching. Lazy per-file diff with a
cache, computed concurrently.

**Check.**
```
cd /tmp/cdfix && codediff
```

**Pass when.**
- [ ] the list contains **exactly the manifest files**, in the correct groups
      (Changes / Staged Changes / Merge Changes) with correct status letters
- [ ] `src/both.rs` appears in **both** Changes and Staged Changes
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
- [ ] `git add X` moves X from Changes to Staged Changes, **selection preserved**
- [ ] `git reset` moves it back
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
| 1 | three-state explorer (Changes / Staged / Merge) or simple worktree-vs-HEAD? | S5 | **three-state** — matches the plugin and is what agent review needs |
| 3 | include inline (single-pane) mode in MVP? | S7 | **no** — it is a projection over the same model, roughly two days to add later |

*Question 2 (syntax engine) is settled — see [D11](05-decisions.md#d11--syntax-highlighting-is-in-the-mvp-via-syntect).*
