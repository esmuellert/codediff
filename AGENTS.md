# AGENTS.md

Rules for making changes in this repository.

## Writing

**Match the comment style around you. Keep inline comments short.**
A comment three times longer than its neighbours draws the eye to the wrong place.

**Name things after what they hold (`files`, `diff`, `line`, `commit`), use `Option` and `Result` directly instead of wrapper enums, and reserve `use_*` for real hooks taking `&mut Scope`.**
Domain nouns and standard types say what is there without making the reader learn new framework words.

## Running

**Locally, pass `-j` to every cargo command, set to half the machine's cores.**
Cargo takes all of them by default, which leaves the machine unusable to whoever is sitting at it.

## Verifying

**Do not state a number or a cause unless you measured it.**
Say what you measured. A guess that sounds precise is worse than no number.

**Break the code on purpose and watch a new test fail. Only then trust it.**
A test that has never failed proves nothing. Breaking the code also finds design faults that reading it does not.

## Changing

**Do not commit, push or open a PR unless explicitly asked.**
Write the file and show it. The user decides when it ships.

**PR titles must use the conventional-commit prefixes defined in `cliff.toml` — git-cliff reads them to build the changelog.**

**When you move code, only move it.**
Do not rename, reword or reshape it in the same step. A reader cannot tell a move from an edit when both are in one diff.
