# codediff

A standalone, read-only terminal diff reviewer built for reviewing LLM-agent code changes.

- **Read-only.** Not an editor. codediff never writes to your files.
- **Standalone.** No Neovim, no editor host. Works over SSH, as a `git difftool`, in CI.
- **VSCode-quality diffs.** Reuses the C diff engine from
  [codediff.nvim](https://github.com/esmuellert/vscode-diff.nvim), statically linked.
- **Agent-focused.** Built for the workflow where an agent edits while you review.

Status: **pre-MVP, in design.** See [`docs/plan/`](docs/plan/README.md).

## Documentation

| document | contents |
|---|---|
| [Overview](docs/plan/01-overview.md) | goal, MVP scope boundary, effort estimate |
| [Architecture](docs/plan/02-architecture.md) | crates, dependency graph, data flow, invariants |
| [Verification](docs/plan/03-verification.md) | how each milestone is proven, by hand and by test |
| [Milestones](docs/plan/04-milestones.md) | S1–S17 with exact acceptance criteria |
| [Decisions](docs/plan/05-decisions.md) | decision log with rationale and rejected options |

## License

MIT. The bundled C diff engine is derived from VSCode's diffing algorithm and carries
its own attribution — see `ATTRIBUTION.md` (to be added with the vendored source).
