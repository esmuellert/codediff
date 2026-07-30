# codediff — plan

Design and delivery plan for `codediff`, a standalone read-only Rust TUI diff reviewer.

## Read in this order

1. **[Overview](01-overview.md)** — what we are building, the MVP scope boundary, effort.
2. **[Architecture](02-architecture.md)** — crate layout, dependency graph, data flow, the
   invariants that keep it honest.
3. **[Verification](03-verification.md)** — the tooling that makes every milestone provable
   by a human in a few commands.
4. **[Milestones](04-milestones.md)** — S1 through S17, each with an exact acceptance check.
5. **[Decisions](05-decisions.md)** — the decision log, including options considered and
   rejected, so they are not relitigated later.

## Delivery model

Work proceeds one milestone at a time:

> implement → deliver → human runs the acceptance check → pass → next

A milestone is not done when the code compiles or when tests pass. It is done when the
acceptance check in [Milestones](04-milestones.md) has been run by a human and passed.

## Keeping these documents true

These are living documents, not a historical record.

- When a decision changes, edit [Decisions](05-decisions.md) and note the supersession.
- When a milestone's acceptance criteria change, edit it before starting the work.
- When the architecture changes, edit [Architecture](02-architecture.md) in the same
  change that alters the code.
