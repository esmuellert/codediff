# Comment Sanitizer

A protocol for removing LLM-generated comment bloat from a codebase.

## Methods

1. **Find by ratio.** Sort files by comment-to-code percentage. Work top-down — the worst files first. Track progress in a table so nothing is missed.

2. **Rewrite long blocks.** Any `///` or `//!` block over ~8 lines: rewrite it in your own words stating only what a reader needs. If the rewrite loses nothing important and is half the length, apply it.

3. **Strip mechanical patterns.** Bold markdown in comments, "deliberately", "the whole point/reason" — these are LLM tells that can be found with grep and fixed with a regex or one-line edit.

4. **Fix jargon first-lines.** The opening `//!` line of a module should say what the file contains in plain words. If it poses a riddle or uses a formula ("What X can do, and the keys that ask for it"), rewrite it.

5. **Kill defensive/hedge comments.** Comments that explain what something is NOT, justify placement ("here rather than X because"), or tell history ("used to be", "it once was") — delete or collapse to one clause.

## Decision rule

Keep if it answers: "What does a caller need to know that the signature doesn't say?"

Cut if it explains why it's here and not somewhere else, tells a story about what it used to be, defends itself against an alternative, restates the type signature, uses emphasis to sound authoritative, or compares to another project to justify a choice.
