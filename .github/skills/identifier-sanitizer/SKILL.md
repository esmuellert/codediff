---
name: identifier-sanitizer
description: Fix vague, jargon, or LLM-generated identifiers in a codebase. Use when asked to sanitize names, clean up naming, or run the identifier sanitizer.
---

# Identifier Sanitizer

A protocol for fixing vague, jargon, or LLM-generated identifiers in a codebase.

## Methods

1. **Test each name with one question.** Can a reader guess what it means without opening the file? If not, rename it. A name should say what it does to what — `poll` → `poll_responses`, `send` → `request_colours`, `of` → `from_files`.

2. **Getters say what they return.** A getter for a field named `x` is `get_x`, not `x()` when `x` is vague. A bool getter reads as a question: `is_busy`, `is_one_sided`.

3. **Fix the field when you fix the getter.** If `done` becomes `get_lines_coloured`, the field becomes `lines_coloured` too. One name for one thing.

4. **Rename the comments with the code.** Every rename includes checking doc comments, inline comments, and diagrams for the old name. Apply the comment-sanitizer decision rule to each touched doc block while you're there.

## Decision rule

**Rename** if the identifier:
- Comes from implementation details the caller doesn't see ("picture" from Unicode spec, "reading" from a parse state)
- Describes the effect rather than the action ("visible" vs "sanitize")
- Packs two questions into one return with no way to name both ("only" meaning "is it one-sided, and which side?")
- Is a single generic word that could mean anything without its type (`get`, `take`, `run`, `of`, `have`)

**Keep** if:
- The type already says it (`File::path()`, `Alignment::spans()`, `Hunk::id()`)
- It's a standard language convention (`new`, `len`, `is_empty`, `from`, `into`, `default`)
- It's the domain term (`compute` for the diff engine, `parse` for a parser)
