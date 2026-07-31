# codediff

A standalone, read-only terminal diff reviewer built for reviewing LLM-agent code changes.

- **Read-only.** Not an editor. codediff never writes to your files.
- **Standalone.** No Neovim, no editor host. A single self-contained binary.
- **VSCode-quality diffs.** The C diff engine from
  [codediff.nvim](https://github.com/esmuellert/codediff.nvim), compiled from source
  and linked statically.
- **Agent-focused.** Built for the workflow where an agent edits while you review.

**Status: S1 and S2 complete.** The vendored C engine, the FFI layer and the safe Rust
wrapper build, link, are covered by tests, and agree with upstream's own `diff_tool` on
every fixture. There is no review interface yet — see
[docs/plan/04-milestones.md](docs/plan/04-milestones.md).

## Building

Requires a C compiler. The Rust toolchain is pinned by `rust-toolchain.toml`, so
[rustup](https://rustup.rs) fetches the right one automatically.

```sh
cargo build --release
./target/release/codediff doctor
```

`doctor` reports how the binary was built. Printing the engine version requires a
successful call through the C ABI, so it doubles as proof the engine is linked:

```
codediff 0.1.0

build
  diff engine   libvscode-diff 2.60.0 (static, call succeeded)
  openmp        disabled, no libgomp dependency
  target        aarch64-unknown-linux-gnu
  profile       release
  rustc         rustc 1.97.1
```

Environment checks — git, terminal, watcher, configuration — arrive with the subsystems
they test.

## Development

```sh
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all

cargo xtask verify-c            # vendored C matches vendor/UPSTREAM.lock
cargo xtask verify-oracle       # our binding agrees with upstream's diff_tool
cargo xtask lint-arch           # forbidden crate edges, purity, unsafe policy
cargo xtask lint-size           # file line cap, excluding tests
```

`verify-oracle` builds upstream's own `diff_tool` from the vendored C and compares its
results against ours over upstream's fixtures. It is a differential test: it catches
marshalling mistakes — an off-by-one in a range, a misread field, the wrong line-splitting
rule — that unit tests cannot.

`xtask` is not a build system — `cargo build` compiles everything, including the C.
It holds the chores cargo has no opinion about, and in particular the rules from
`docs/plan` that would otherwise be prose nobody enforces.

### Inspecting a diff

```sh
codediff debug diff old.txt new.txt
```

```
original  old.txt (8 lines)
modified  new.txt (8 lines)
engine    libvscode-diff 2.60.0

2 change(s), 1 move(s)

  [0] inserted   original 1..1 (empty)  modified 1..5
        inner  L1:C1-L1:C1  ->  L1:C1-L5:C1
  [1] deleted    original 4..8  modified 8..8 (empty)

  moves
  [0] original 4..8  ->  modified 1..5
```

### The vendored C engine

`vendor/libvscode-diff/` is a copy of an upstream tag, with its provenance and a
content hash recorded in `vendor/UPSTREAM.lock`. It is never edited in place; a local
change makes `cargo xtask verify-c` fail. To move to a new upstream release:

```sh
cargo xtask sync-c --tag v2.61.0
```

## Architecture

Ten crates with a strictly acyclic dependency graph, four of them pure — no IO, no
state, no clock — and state, time and concurrency deliberately concentrated in one.
Crates are created by the milestone that needs them; `cargo xtask lint-arch` enforces
the forbidden edges as they appear.

See [docs/plan/02-architecture.md](docs/plan/02-architecture.md).

## Documentation

| document | contents |
|---|---|
| [Overview](docs/plan/01-overview.md) | goal, MVP scope boundary, effort estimate |
| [Architecture](docs/plan/02-architecture.md) | crates, dependency graph, data flow, invariants |
| [Verification](docs/plan/03-verification.md) | how each milestone is proven, by hand and by test |
| [Milestones](docs/plan/04-milestones.md) | S1–S17 with exact acceptance criteria |
| [Decisions](docs/plan/05-decisions.md) | decision log with rationale and rejected options |

## License

MIT — see [LICENSE](LICENSE).

The C engine is a port of VSCode's diff algorithm and bundles utf8proc; both are MIT
and their notices are carried in [ATTRIBUTION.md](ATTRIBUTION.md).
